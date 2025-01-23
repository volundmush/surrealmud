use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc
};

use futures::future::join_all;

use tokio;
use tokio::sync::mpsc::{Sender, Receiver, channel};

use tracing::{error, info, Level};
use tracing_subscriber;

use surrealmud_shared::TotalConf;

use surrealmud_portal::{
    surreal::{Msg2Db, DbManager},
    telnet::listen::TelnetListener
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // This check will also ensure we're in the right directory.
    let conf = Arc::new(TotalConf::set("devel")?);

    // TODO: Make this more configurable and save to the logs directory.
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("surrealmud-portal starting up...");

    let mut v = Vec::new();

    info!("Starting up SurrealDB to {}, tls: {}", conf.surreal.address, conf.surreal.tls);
    let mut db = DbManager::new(conf.clone()).await?;

    db.setup().await?;

    let tx_db = db.tx_db.clone();
    v.push(tokio::spawn(async move {db.run().await;}));

    info!("Starting up telnet acceptor on {}...", conf.portal.telnet);
    let mut telnet_acceptor = TelnetListener::new(conf.clone(), tx_db.clone()).await?;

    let tx_telnet = telnet_acceptor.tx_telnet.clone();
    v.push(tokio::spawn(async move {telnet_acceptor.run().await;}));

    info!("Starting all tasks...");
    join_all(v).await;

    info!("surrealmud-portal shutting down.");
    Ok(())
}