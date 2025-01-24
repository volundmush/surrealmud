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

pub struct DbManager {
    conf: Arc<TotalConf>,
    mssp: Vec<(String, String)>,
    pub tx_db: mpsc::Sender<Msg2Db>,
    rx_db: mpsc::Receiver<Msg2Db>,
    db: Surreal<Client>

}

impl DbManager {
    pub async fn new(conf: Arc<TotalConf>) -> Result<Self, Box<dyn std::error::Error>> {

        let mut db = if(conf.surreal.tls) {
            Surreal::new::<Wss>(&conf.surreal.address).await?
        } else {
            Surreal::new::<Ws>(&conf.surreal.address).await?
        };

        let (tx_db, rx_db) = mpsc::channel::<Msg2Db>(10);

        Ok(Self {
            conf,
            rx_db,
            tx_db,
            db,
            mssp: Default::default()
        })
    }

    pub async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.db.signin(Root {
            username: &self.conf.surreal.username,
            password: &self.conf.surreal.password
        }).await?;

        self.db.use_ns(&self.conf.surreal.namespace).use_db(&self.conf.surreal.database).await?;

        Ok(())
    }

    fn check_site(&self, hostnames: &Vec<String>, ip: &std::net::IpAddr) -> bool {
        // TODO: put in the pull logic later.
        true
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(msg) = self.rx_db.recv() => {
                    match msg {
                        Msg2Db::GetMSSP(tx) => {
                            let _ = tx.send(Msg2TelnetProtocol::MSSP(self.mssp.clone())).await;
                        },
                        Msg2Db::CheckSite(tx, hostnames, ip) => {
                            let _ = tx.send(self.check_site(&hostnames, &ip));
                        }
                    }
                }
            }
        }

    }
}