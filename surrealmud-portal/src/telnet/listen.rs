use std::{
    net::SocketAddr,
    sync::Arc,
    str::FromStr
};
use std::net::ToSocketAddrs;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
};
use tracing::{info, error};
use surrealmud_shared::TotalConf;
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    surreal::{Msg2Db},
    telnet::conn::TelnetProtocol
};

pub enum Msg2Listener {

}

pub struct TelnetListener {
    conf: Arc<TotalConf>,
    listener: TcpListener,
    tx_db: mpsc::Sender<Msg2Db>,
    resolver: TokioAsyncResolver,
    pub tx_telnet: mpsc::Sender<Msg2Listener>,
    rx_telnet: mpsc::Receiver<Msg2Listener>
}

impl TelnetListener {
    pub async fn new(conf: Arc<TotalConf>, tx_db: mpsc::Sender<Msg2Db>) -> Result<Self, Box<dyn std::error::Error>> {
        let addr = SocketAddr::from_str(&conf.portal.telnet)?;

        let listener = TcpListener::bind(addr).await?;
        let resolver = TokioAsyncResolver::tokio_from_system_conf()?;

        let (tx_telnet, rx_telnet) = mpsc::channel(10);

        Ok(TelnetListener {
            conf,
            listener,
            tx_db,
            resolver,
            tx_telnet,
            rx_telnet
        })
    }

    async fn check_site(&self, hostnames: &Vec<String>, ip: std::net::IpAddr) -> bool {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx_db.send(Msg2Db::CheckSite(tx, hostnames.clone(), ip)).await;
        rx.await.unwrap()
    }

    pub async fn run(&mut self) {

        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {

                    let mut hostnames: Vec<String> = vec!();
                    if let Ok(response) = self.resolver.reverse_lookup(addr.ip()).await {
                        hostnames = response.iter().map(|x| x.to_string()).collect();
                    }

                    if(!self.check_site(&hostnames, addr.ip()).await) {
                        info!("(BLOCKED) Connection from: {:?} ({:?})", addr, hostnames);
                        continue;
                    }

                    info!("Connection from: {:?} ({:?})", addr, hostnames);

                    let mut handler = TelnetProtocol::new(self.conf.clone(), stream, addr, hostnames, self.tx_db.clone(), false);
                    tokio::spawn(async move { handler.run().await;});
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }
}