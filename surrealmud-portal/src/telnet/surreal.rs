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

pub enum Msg2Surreal {
    Command(String),
    Capabilities(ProtocolCapabilities)
}

pub struct SurrealManager {
    conf: Arc<TotalConf>,
    rx_surreal: mpsc::Receiver<Msg2Surreal>,
    tx_telnet: mpsc::Sender<Msg2TelnetProtocol>,
    db: Surreal<Client>
}

impl SurrealManager {
    pub fn new(conf: Arc<TotalConf>, rx_surreal: mpsc::Receiver<Msg2Surreal>, tx_telnet: mpsc::Sender<Msg2TelnetProtocol>, db: Surreal<Client>) -> Self {
        Self {
            conf,
            rx_surreal,
            tx_telnet,
            db
        }
    }




    pub async fn run(&mut self) {

    }


}