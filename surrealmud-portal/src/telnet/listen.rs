use std::{
    net::SocketAddr,
    sync::Arc
};

use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{Sender, Receiver},
};

use surrealmud_shared::TotalConf;

use crate::{
    surreal::{Msg2Db},
    telnet::conn::TelnetConnection
};

pub struct TelnetListener {
    conf: Arc<TotalConf>,
    listener: TcpListener,
    tx_db: Sender<Msg2Db>
}

impl TelnetListener {
    pub async fn new(conf: Arc<TotalConf>, addr: SocketAddr, tx_db: Sender<Msg2Db>) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;

        Ok(TelnetListener {
            conf,
            listener,
            tx_db
        })
    }

    pub async fn run(&mut self) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    let mut handler = TelnetConnection::new(conf.clone(), addr, self.tx_db.clone());
                    tokio::spawn(async move {
                        match handler.run(stream).await {
                            Ok(()) => {},
                            Err(e) => {
                                println!("Error accepting Telnet Connection: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    println!("Error accepting connection: {}", e);
                }
            }
        }
    }
}