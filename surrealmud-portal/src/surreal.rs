use std::{
    sync::Arc,
    net::IpAddr
};

use tokio::{
    sync::mpsc,
    sync::oneshot,
};

use surrealmud_shared::TotalConf;

use crate::{
    telnet::conn::Msg2TelnetProtocol
};

use surrealdb::{Error, Surreal};
use surrealdb::opt::auth::Root;
use surrealdb::engine::remote::ws::{Ws, Wss, Client};

pub enum Msg2Db {
    GetMSSP(mpsc::Sender<Msg2TelnetProtocol>),
    CheckSite(oneshot::Sender<bool>, Vec<String>, IpAddr)
}

pub struct SurrealManager {
    conf: Arc<TotalConf>,
    mssp: Vec<(String, String)>,
    rx_db: mpsc::Receiver<Msg2Db>,
    db: Surreal<Client>

}

impl SurrealManager {
    pub fn new(conf: Arc<TotalConf>, rx_db: mpsc::Receiver<Msg2Db>, db: Surreal<Client>) -> Self {
        SurrealManager {
            conf,
            rx_db,
            db,
            mssp: Default::default()
        }
    }

    pub async fn run(&mut self) {

    }
}