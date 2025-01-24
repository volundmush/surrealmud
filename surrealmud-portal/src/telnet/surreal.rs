use std::{
    sync::Arc,
};

use super::conn::{Msg2TelnetProtocol, ProtocolCapabilities};

use tokio::{
    sync::{mpsc, oneshot}
};

use surrealdb::{Error, Surreal};
use surrealdb::opt::auth::Database;
use surrealdb::engine::remote::ws::{Ws, Wss, Client};

use surrealmud_shared::TotalConf;
use crate::surreal::Msg2Db;

pub enum Msg2Surreal {
    Command(String),
    Capabilities(ProtocolCapabilities)
}

pub struct SurrealManager {
    conf: Arc<TotalConf>,
    rx_surreal: mpsc::Receiver<Msg2Surreal>,
    pub tx_surreal: mpsc::Sender<Msg2Surreal>,
    tx_telnet: mpsc::Sender<Msg2TelnetProtocol>,
    db: Surreal<Client>
}

impl SurrealManager {
    pub async fn new(conf: Arc<TotalConf>, rx_surreal: mpsc::Receiver<Msg2Surreal>, tx_telnet: mpsc::Sender<Msg2TelnetProtocol>, db: Surreal<Client>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut db = if(conf.surreal.tls) {
            Surreal::new::<Wss>(&conf.surreal.address).await?
        } else {
            Surreal::new::<Ws>(&conf.surreal.address).await?
        };

        let (tx_surreal, rx_surreal) = mpsc::channel(10);

        Ok(Self {
            conf,
            rx_surreal,
            tx_surreal,
            tx_telnet,
            db
        })
    }


    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(msg) = self.rx_surreal.recv() => {
                    match msg {
                        Msg2Surreal::Command(cmd) => {

                        },
                        Msg2Surreal::Capabilities(cap) => {

                        }

                    }
                },
                None = self.rx_surreal.recv() => {
                    // the sender has dropped, so we should exit. This only happens if telnet
                    // connection has dropped.
                    break;
                }
            }
        }
    }
}