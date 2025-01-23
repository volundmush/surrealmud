use std::{
    net::SocketAddr,
    sync::Arc
};

use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{Sender, Receiver},
};

use surrealmud_shared::TotalConf;
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    surreal::{Msg2Db},
    telnet::conn::TelnetProtocol
};

pub struct TelnetListener {
    conf: Arc<TotalConf>,
    listener: TcpListener,
    tx_db: Sender<Msg2Db>,
    resolver: TokioAsyncResolver
}

impl TelnetListener {
    pub async fn new(conf: Arc<TotalConf>, addr: SocketAddr, tx_db: Sender<Msg2Db>) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;
        let resolver = TokioAsyncResolver::tokio_from_system_conf()?;

        Ok(TelnetListener {
            conf,
            listener,
            tx_db,
            resolver
        })
    }

    pub async fn run(&mut self) {

        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {

                    let mut handler = TelnetProtocol::new(self.conf.clone(), stream, addr, vec!(), self.tx_db.clone(), false);
                    tokio::spawn(async move {
                        match handler.run().await {
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