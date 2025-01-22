use serde::{Deserialize, Serialize};
use config::{Config, File, FileFormat};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct SurrealConf {
    pub address: String,
    pub tls: bool,
    pub namespace: String,
    pub database: String,
    pub username: String,
    pub password: String
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct PortalConf {
    pub telnet: String
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct TotalConf {
    pub surreal: SurrealConf,
    pub portal: PortalConf

}

// A global that will be set *once*

impl TotalConf {
    pub fn set(mode: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Config::builder()
            .add_source(File::new("config.default", FileFormat::Toml).required(true))
            .add_source(File::new(&format!("config.{}", mode), FileFormat::Toml).required(true))
            .add_source(File::new("config.user", FileFormat::Toml).required(false))
            .add_source(File::new(&format!("config.user.{}", mode), FileFormat::Toml).required(false))
            .build()?
            .try_deserialize()?
    }
}

