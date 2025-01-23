use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    time::{Duration, Instant},
    vec::Vec,
    sync::LazyLock
};
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc,
    time
};

use tokio_util::codec::Framed;

use tokio_stream::wrappers::IntervalStream;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use futures::{
    sink::SinkExt,
    stream::StreamExt
};

use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use surrealmud_shared::TotalConf;
use crate::{
    telnet::{
        codes as tc,
        codec::{TelnetCodec, TelnetEvent},
    },
    surreal::Msg2Db
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Color {
    NoColor = 0,
    Standard = 1,
    Xterm256 = 2,
    TrueColor = 3
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolCapabilities {
    pub protocol: &'static str,
    pub encryption: bool,
    pub client_name: String,
    pub client_version: String,
    pub host_address: String,
    pub host_port: u16,
    pub host_names: Vec<String>,
    pub encoding: String,
    pub utf8: bool,
    pub ansi_color: Color,
    pub width: u16,
    pub height: u16,
    pub gmcp: bool,
    pub msdp: bool,
    pub mssp: bool,
    pub mccp2: bool,
    pub mccp3: bool,
    pub ttype: bool,
    pub naws: bool,
    pub sga: bool,
    pub linemode: bool,
    pub force_endline: bool,
    pub oob: bool,
    pub tls: bool,
    pub screen_reader: bool,
    pub mouse_tracking: bool,
    pub vt100: bool,
    pub osc_color_palette: bool,
    pub proxy: bool,
    pub mnes: bool
}

impl Default for ProtocolCapabilities {
    fn default() -> Self {
        Self {
            protocol: "telnet",
            width: 78,
            height: 24,
            client_name: "UNKNOWN".to_string(),
            client_version: "UNKNOWN".to_string(),
            host_address: "UNKNOWN".to_string(),
            ..Default::default() // Use defaults for the remaining fields
        }
    }
}

#[derive(Default, Clone)]
pub struct TelnetOptionPerspective {
    pub enabled: bool,
    // Negotiating is true if WE have sent a request.
    pub negotiating: bool
}

#[derive(Default, Clone)]
pub struct TelnetOptionState {
    pub remote: TelnetOptionPerspective,
    pub local: TelnetOptionPerspective,
}

bitflags! {
    pub struct TelnetOption: u8 {
        const ALLOW_LOCAL = 0b0001;
        const ALLOW_REMOTE = 0b0010;
        const START_LOCAL = 0b0100;
        const START_REMOTE = 0b1000;
    }
}

static TELNET_OPTIONS: LazyLock<HashMap<u8, TelnetOption>> = LazyLock::new( || {
    let mut map: HashMap<u8, TelnetOption> = HashMap::new();

    map.insert(tc::SGA, TelnetOption::ALLOW_LOCAL | TelnetOption::START_LOCAL);
    map.insert(tc::NAWS, TelnetOption::ALLOW_REMOTE | TelnetOption::START_REMOTE);
    map.insert(tc::MTTS, TelnetOption::ALLOW_REMOTE | TelnetOption::START_REMOTE);
    map.insert(tc::MSSP, TelnetOption::ALLOW_LOCAL | TelnetOption::START_LOCAL);
    map.insert(tc::MCCP2, TelnetOption::ALLOW_LOCAL | TelnetOption::START_LOCAL);
    map.insert(tc::MCCP3, TelnetOption::ALLOW_LOCAL | TelnetOption::START_LOCAL);
    map.insert(tc::GMCP, TelnetOption::ALLOW_LOCAL | TelnetOption::START_LOCAL);
    map.insert(tc::MSDP, TelnetOption::ALLOW_LOCAL | TelnetOption::START_LOCAL);
    map.insert(tc::LINEMODE, TelnetOption::ALLOW_REMOTE | TelnetOption::START_REMOTE);
    map.insert(tc::TELOPT_EOR, TelnetOption::ALLOW_LOCAL | TelnetOption::START_LOCAL);
    map
});

#[derive(Default, Debug, Clone)]
pub struct TelnetHandshakes {
    pub local: HashSet<u8>,
    pub remote: HashSet<u8>,
    pub ttype: HashSet<u8>
}

impl TelnetHandshakes {
    pub fn len(&self) -> usize {
        self.local.len() + self.remote.len() + self.ttype.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug)]
pub struct TelnetTimers {
    pub last_interval: Instant,
    pub last_keepalive: Instant,
}

impl Default for TelnetTimers {
    fn default() -> Self {
        Self {
            last_interval: Instant::now(),
            last_keepalive: Instant::now()
        }
    }
}

fn ensure_crlf(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut prev_char_is_cr = false;
    let iac = char::from(255);

    for c in input.chars() {
        match c {
            '\r' => {
                prev_char_is_cr = true;
                result.push('\r');
            },
            '\n' => {
                if !prev_char_is_cr {
                    result.push('\r');
                }
                result.push(c);
                prev_char_is_cr = false;
            },
            iac => {
                // This is a telnet IAC character, so we need to escape it.
                result.push(iac);
                result.push(iac);
                prev_char_is_cr = false;
            },
            _ => {
                result.push(c);
                prev_char_is_cr = false;
            }
        }
    }

    result
}

pub enum Msg2TelnetProtocol {
    GameClose,
    GMCP(String, JsonValue),
    Text(String),
    MSSP(Vec<(String, String)>),
}

pub struct TelnetProtocol<T> {
    // This serves as a higher-level actor that abstracts a bunch of the lower-level
    // nitty-gritty so the Session doesn't need to deal with it.
    conf: Arc<TotalConf>,
    op_state: HashMap<u8, TelnetOptionState>,
    config: ProtocolCapabilities,
    handshakes_left: TelnetHandshakes,
    ttype_count: u8,
    ttype_last: Option<String>,
    conn: Framed<T, TelnetCodec>,
    active: bool,
    sent_link: bool,
    running: bool,
    app_buffer: BytesMut,
    time_created: Instant,
    time_activity: Instant,
    timers: TelnetTimers,
    tx_protocol: mpsc::Sender<Msg2TelnetProtocol>,
    rx_protocol: mpsc::Receiver<Msg2TelnetProtocol>,
    tx_db: mpsc::Sender<Msg2Db>
}


impl<T> TelnetProtocol<T> where T: AsyncRead + AsyncWrite + Send + 'static + Unpin + Sync {
    pub fn new(conf: Arc<TotalConf>, conn: T, addr: SocketAddr, hostnames: Vec<String>, tx_db: mpsc::Sender<Msg2Db>, tls: bool) -> Self {

        let (tx_protocol, rx_protocol) = mpsc::channel(10);

        let mut out = Self {
            conf,
            conn: Framed::new(conn, TelnetCodec::new(8192)),
            tx_protocol,
            rx_protocol,
            tx_db,
            running: true,
            app_buffer: BytesMut::with_capacity(1024),
            time_created: Instant::now(),
            time_activity: Instant::now(),
            timers: TelnetTimers::default(),
            active: false,
            sent_link: false,
            ttype_count: 0,
            ttype_last: None,
            op_state: HashMap::new(),
            config: Default::default(),
            handshakes_left: Default::default()
        };
        // Stack overflow before reaching this point.
        out.config.tls = tls;
        out.config.host_address = addr.ip().to_string();
        out.config.host_port = addr.port();
        out.config.host_names = hostnames;
        out
    }

    async fn handle_conn(&mut self, t_msg: Option<Result<TelnetEvent, std::io::Error>>) {
        if let Some(msg) = t_msg {
            self.time_activity = Instant::now();
            match msg {
                Ok(msg) => {
                    let _ = self.process_telnet_event(msg).await;
                },
                Err(e) => {
                    self.running = false;
                }
            }
        } else {
            self.running = false;
        }
    }

    async fn send(&mut self, te: TelnetEvent) -> bool {
        match self.conn.send(te).await {
            Ok(_) => true,
            Err(e) => {
                self.running = false;
                //let _ = self.tx_portal.send(Msg2Portal::ClientDisconnected(self.conn_id, String::from(e.to_string()))).await;
                false
            }
        }
    }

    pub async fn run(&mut self) {

        // Initialize Telnet Op handlers.
        for (code, tel_op) in TELNET_OPTIONS.iter() {

            let mut state = TelnetOptionState::default();
            if(tel_op.contains(TelnetOption::START_LOCAL)) {
                state.local.negotiating = true;
                self.send(TelnetEvent::Negotiate(tc::WILL, *code)).await;
                self.handshakes_left.local.insert(*code);
            }
            if(tel_op.contains(TelnetOption::START_REMOTE)) {
                state.remote.negotiating = true;
                self.send(TelnetEvent::Negotiate(tc::DO, *code)).await;
                self.handshakes_left.remote.insert(*code);
            }
            self.op_state.insert(*code, state);

        }

        let mut interval_timer = IntervalStream::new(time::interval(Duration::from_millis(100)));

        let mut in_negotiation_phase = true;
        let negotiation_deadline = time::Instant::now() + Duration::from_millis(500);

        // The main loop which operates the protocol during and after negotiation.
        while self.running {
            tokio::select! {
            t_msg = self.conn.next() => self.handle_conn(t_msg).await,
            p_msg = self.rx_protocol.recv() => {
                if let Some(msg) = p_msg {
                    let _ = self.process_protocol_message(msg).await;
                }
            },
            i_msg = interval_timer.next() => {
                if let Some(ins) = i_msg {
                    let _ = self.handle_interval_timer(ins.into_std()).await;
                }
            }
            _ = time::sleep_until(negotiation_deadline), if in_negotiation_phase => {
                in_negotiation_phase = false;
            }
        }

            // Check if negotiations are complete or timed out
            if in_negotiation_phase && self.handshakes_left.is_empty() {
                in_negotiation_phase = false;
            }

            // If negotiations have just completed or timed out, send the ClientConnected message
            if !in_negotiation_phase && !self.sent_link {
                // TODO: setup SurrealDB link here...
                self.sent_link = true;
                self.active = true;
                self.process_app_buffer().await;
            }
        }
    }

    async fn handle_interval_timer(&mut self, ins: Instant) {
        // Check if the connection has been utterly idle at a network level for too long.
        if(self.time_activity.elapsed().as_secs() > (60 * 30)) {
            // handle disconnect here.
        }

        self.timers.last_interval = ins;
    }

    async fn process_telnet_event(&mut self, msg: TelnetEvent) {
        match msg {
            TelnetEvent::SubNegotiate(op, data) => self.receive_sub(op, data).await,
            TelnetEvent::Negotiate(comm, op) => self.receive_negotiate(comm, op).await,
            TelnetEvent::Command(byte) => {
                let _ = self.process_telnet_command(byte).await;
            },
            TelnetEvent::Data(data) => {
                self.app_buffer.put(data);
                if self.sent_link {
                    let _ = self.process_app_buffer().await;
                }
            }
        }
    }

    async fn process_telnet_command(&mut self, byte: u8) {
        match byte {
            tc::NOP => {},
            _ => {}
        }
    }

    async fn process_app_buffer(&mut self) {
        loop {
            // Find the position of the LF character
            if let Some(ipos) = self.app_buffer.as_ref().iter().position(|b| b == &b'\n') {

                // Extract the line without CR and LF characters
                let cmd = self.app_buffer.split_to(ipos);


                // Convert the line to a String and handle the command
                if let Ok(s) = String::from_utf8(cmd.to_vec()) {
                    // strip all \r from the string
                    let s = s.replace("\r", "");
                    let _ = self.handle_user_command(s).await;
                }

                // Advance the buffer to consume LF character
                self.app_buffer.advance(1);
            } else {
                // No more complete lines in the buffer, break the loop
                break;
            }
        }
    }

    async fn handle_user_command(&mut self, cmd: String) {
        if cmd.starts_with("//") {
            let _ = self.handle_protocol_command(cmd);
        } else if self.sent_link {
            // We must format the command as a Msg2PortalFromClient::Data, so we must encapsulate this in a MudData.

        }
    }

    async fn handle_protocol_command(&mut self, cmd: String) {
        // TODO: Handle protocol commands
    }

    async fn process_protocol_message(&mut self, msg: Msg2TelnetProtocol) {
        match msg {
            Msg2TelnetProtocol::GameClose => {
                self.running = false;
            },
            Msg2TelnetProtocol::GMCP(v, j) => {
                let mut gmcp_data = Vec::new();
                gmcp_data.push(v);
                gmcp_data.push(j.to_string());
                let gmcp_out = gmcp_data.join(" ");
                let _ = self.send(TelnetEvent::SubNegotiate(tc::GMCP, Bytes::from(gmcp_out))).await;
            },
            Msg2TelnetProtocol::Text(t) => {
                let _ = self.send(TelnetEvent::Data(Bytes::from(ensure_crlf(&t)))).await;
            },
            Msg2TelnetProtocol::MSSP(v) => {
                // this message should come from the DbManager, and it needs to be forwarded to the client.
                let mut mssp_data = Vec::new();
                for (k, v) in v {
                    mssp_data.push(format!("{} {}", k, v));
                }
                let mssp_data = mssp_data.join("\r\n");
                let _ = self.send(TelnetEvent::SubNegotiate(tc::MSSP, Bytes::from(mssp_data))).await;
            }
        }
    }

    async fn receive_negotiate(&mut self, command: u8, op: u8) {
        // This means we received an IAC will/wont/do/dont...
        let mut handshake: u8 = 0;
        let mut enable_local = false;
        let mut disable_local = false;
        let mut enable_remote = false;
        let mut disable_remote = false;
        let mut handshake_remote: u8 = 0;
        let mut handshake_local: u8 = 0;
        let mut respond: u8 = 0;

        if let Some(state) = self.op_state.get_mut(&op) {
            // We DO have a handler for this option... that means we support it!

            match command {
                tc::WILL => {
                    // The remote host has sent a WILL. They either want to Locally-Enable op, or are
                    // doing so at our request.
                    if !state.remote.enabled {
                        if state.remote.negotiating {
                            state.remote.negotiating = false;
                        }
                        else {
                            respond = tc::DO;
                        }
                        handshake = op;
                        handshake_remote = op;
                        enable_remote = true;
                        state.remote.enabled = true;
                    }
                },
                tc::WONT => {
                    // The client has refused an option we wanted to enable. Alternatively, it has
                    // disabled an option that was on.
                    if state.remote.negotiating {
                        handshake = op;
                        handshake_remote = op;
                    }
                    state.remote.negotiating = false;
                    if state.remote.enabled {
                        disable_remote = true;
                        state.remote.enabled = false;
                    }
                },
                tc::DO => {
                    // The client wants the Server to enable Option, or they are acknowledging our
                    // desire to do so.
                    if !state.local.enabled {
                        if state.local.negotiating {
                            state.local.negotiating = false;
                        }
                        else {
                            respond = tc::WILL;
                        }
                        handshake = op;
                        handshake_local = op;
                        enable_local = true;
                        state.local.enabled = true;
                    }
                },
                tc::DONT => {
                    // The client wants the server to disable Option, or are they are refusing our
                    // desire to do so.
                    if state.local.negotiating {
                        handshake = op;
                        handshake_local = op;
                    }
                    state.local.negotiating = false;
                    if state.local.enabled {
                        disable_local = true;
                        state.local.enabled = false
                    }
                },
                _ => {
                    // This cannot actually happen.
                }
            }
        } else {
            // We do not have a handler for this option, whatever it is... do not support.
            respond = match command {
                tc::WILL => tc::DONT,
                tc::DO => tc::WONT,
                _ => 0
            };
        }

        if respond > 0 {
            let _ = self.send(TelnetEvent::Negotiate(respond, op)).await;
        }
        if handshake_local > 0 {
            self.handshakes_left.local.remove(&handshake_local);
        }
        if handshake_remote > 0 {
            self.handshakes_left.remote.remove(&handshake_remote);
        }
        if enable_local {
            let _ = self.enable_local(op).await;
        }
        if disable_local {
            let _ = self.disable_local(op).await;
        }
        if enable_remote {
            let _ = self.enable_remote(op).await;
        }
        if disable_remote {
            let _ = self.disable_remote(op).await;
        }
    }

    async fn enable_remote(&mut self, op: u8) {
        match op {
            tc::NAWS => self.config.naws = true,
            tc::MTTS => {
                self.handshakes_left.ttype.insert(0);
                self.request_ttype().await;
            },
            tc::LINEMODE => self.config.linemode = true,
            _ => {
                // Whatever this option is.. well, whatever.
            }
        }
    }

    async fn disable_remote(&mut self, op: u8) {
        match op {
            tc::NAWS => {
                self.config.naws = false;
                self.config.width = 78;
                self.config.height = 24;
            }
            tc::MTTS => {
                self.config.ttype = false;
                self.handshakes_left.ttype.clear();
            },
            tc::LINEMODE => self.config.linemode = false,
            _ => {
                // Whatever this option is.. well, whatever.
            }
        }
    }

    async fn enable_local(&mut self, op: u8) {
        match op {
            tc::SGA => {
                self.config.sga = true;
            },
            tc::MCCP2 => {
                self.config.mccp2 = true;
                self.send(TelnetEvent::SubNegotiate(tc::MCCP2, Bytes::new())).await;
            },
            tc::MCCP3 => {
                self.config.mccp3 = true;
            }
            _ => {

            }
        }
    }

    async fn disable_local(&mut self, op: u8) {
        match op {
            tc::SGA => {
                self.config.sga = false;
            },
            _ => {

            }
        }
    }

    async fn receive_sub(&mut self, op: u8, mut data: Bytes) {
        if !self.op_state.contains_key(&op) {
            // Only if we can get a handler, do we want to care about this.
            // All other sub-data is ignored.
            return;
        }

        match op {
            tc::NAWS => {
                let _ = self.receive_naws(data).await;
            },
            tc::MTTS => {
                let _ = self.receive_ttype(data).await;
            },
            tc::GMCP => {
                if let Ok(s) = String::from_utf8(data.to_vec()) {
                    if s.contains(" ") {
                        let mut parts = s.splitn(2, " ");
                        if let Some(cmd) = parts.next() {
                            if let Some(data) = parts.next() {

                            }
                        }
                    }
                    else {

                    }
                }
            },
            _ => {}
        }
    }

    async fn request_ttype(&mut self) {
        let mut data = BytesMut::with_capacity(1);
        data.put_u8(1);
        self.send(TelnetEvent::SubNegotiate(tc::MTTS, data.freeze())).await;
    }

    async fn receive_ttype(&mut self, mut data: Bytes) {

        if data.len() < 2 {
            return
        }

        if self.handshakes_left.ttype.is_empty() {
            return;
        }

        if data[0] != 0 {
            return;
        }

        data.advance(1);

        if let Ok(s) = String::from_utf8(data.to_vec()) {
            let upper = s.trim().to_uppercase();

            match self.ttype_count {
                0 => {
                    self.ttype_last = Some(upper.clone());
                    let _ = self.receive_ttype_0(upper.clone()).await;
                    self.ttype_count += 1;
                    self.handshakes_left.ttype.remove(&0);
                    let _ = self.request_ttype().await;
                    return;
                },
                1 | 2 => {
                    if let Some(last) = self.ttype_last.clone() {
                        if last.eq(&upper) {
                            // This client does not support advanced ttype. Ignore further
                            // calls to TTYPE and consider this complete.
                            self.handshakes_left.ttype.clear();
                            self.ttype_last = None;
                        } else {
                            match self.ttype_count {
                                1 => {
                                    let _ = self.receive_ttype_1(upper.clone()).await;
                                    self.ttype_last = Some(upper.clone());
                                },
                                2 => {
                                    let _ = self.receive_ttype_2(upper.clone()).await;
                                    self.ttype_last = None;
                                    self.handshakes_left.ttype.clear();
                                }
                                _ => {}
                            }
                        }
                    }
                    return;
                }
                _ => {
                    unreachable!("TTYPE count is out of bounds.");
                }
            }
        }


    }

    async fn receive_ttype_0(&mut self, data: String) {
        // The first TTYPE receives the name of the client.
        // version might also be in here as a second word.
        if data.contains(" ") {
            let results: Vec<&str> = data.splitn(2, " ").collect();
            if results.len() > 0 {
                self.config.client_name = String::from(results[0]);
            }
            if results.len() > 1 {
                self.config.client_version = String::from(results[1]);
            }
        } else {
            self.config.client_name = data;
        }

        // Now that the name and version (may be UNKNOWN) are set... we can deduce capabilities.
        let mut extra_check = false;
        match self.config.client_name.as_str() {
            "ATLANTIS" | "CMUD"  | "KILDCLIENT" | "MUDLET" | "MUSHCLIENT"  | "PUTTY" | "BEIP" | "POTATO" | "TINYFUGUE" => {
                self.config.ansi_color = Color::Xterm256;
            },
            _ => {
                extra_check = true;
            }
        }
        if extra_check {
            if (self.config.client_name.starts_with("XTERM") || self.config.client_name.ends_with("-256COLOR")) && self.config.ansi_color != Color::TrueColor {
                self.config.ansi_color = Color::Xterm256;
            }
        }
    }

    async fn receive_ttype_1(&mut self, data: String) {
        if (data.starts_with("XTERM") || data.ends_with("-256COLOR")) && self.config.ansi_color != Color::TrueColor  {
            self.config.ansi_color = Color::Xterm256;
        }
        self.handshakes_left.ttype.remove(&1);
    }

    async fn receive_ttype_2(&mut self, data: String) {
        if !data.starts_with("MTTS ") {
            return;
        }
        let results: Vec<&str> = data.splitn(2, " ").collect();
        let value = String::from(results[1]);
        let mtts: usize = value.parse().unwrap_or(0);
        if mtts == 0 {
            return;
        }
        if (1 & mtts) == 1 && (self.config.ansi_color.clone() as i32) < Color::Standard as i32 {
            self.config.ansi_color = Color::Standard;
        }

        if (2 & mtts) == 2 {
            self.config.vt100 = true;
        }
        if (4 & mtts) == 4 {
            self.config.utf8 = true;
        }
        if (8 & mtts) == 8 && (self.config.ansi_color.clone() as i32) < Color::Xterm256 as i32 {
            self.config.ansi_color = Color::Xterm256;
        }
        if (16 & mtts) == 16 {
            self.config.mouse_tracking = true;
        }
        if (32 & mtts) == 32 {
            self.config.osc_color_palette = true;
        }
        if (64 & mtts) == 64 {
            self.config.screen_reader = true;
        }
        if (128 & mtts) == 128 {
            self.config.proxy = true;
        }
        if (256 & mtts) == 256  && (self.config.ansi_color.clone() as i32) < Color::TrueColor as i32 {
            self.config.ansi_color = Color::TrueColor;
        }
        if (512 & mtts) == 512 {
            self.config.mnes = true;
        }
        self.handshakes_left.ttype.remove(&2);
    }

    async fn receive_naws(&mut self, mut data: Bytes) {

        if data.len() >= 4 {
            let old_width = self.config.width;
            let old_height = self.config.height;
            self.config.width = data.get_u16();
            self.config.height = data.get_u16();
            if (self.config.width != old_width) || (self.config.height != old_height) {
                let _ = self.update_capabilities().await;
            }
        }
    }

    async fn update_capabilities(&mut self) {
        if self.sent_link {

        }
    }
}