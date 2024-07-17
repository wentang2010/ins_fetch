use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

use crate::const_var::ENV_MODE;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cert {
    /// cert
    pub cert: String,
    /// key
    pub key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Log {
    pub dir: String,
    pub log_level: String,
    pub enable_oper_log: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Jwt {
    pub jwt_exp: u32,
    pub jwt_secret: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DBConf {
    pub url: String,
    pub db_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SOA {
    pub webapi: String,
    pub bridge_api_url: String,
    pub hermes_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClickHouse {
    pub clickhouse_database_url: String,
    pub db_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RedisUrl {
    pub redis_connection_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkId {
    pub work_id: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Server {
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainInfo {
    pub url: String,
    pub filter_address: Vec<String>, // program address or account address
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AccountInfo {
    pub program_id: String,
    pub account_book: String, // program address or account address
    pub inscription_size: u32,
    pub replica_ins_size: u32,
    pub ins_meta_size: u32,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Application {
    pub db: DBConf,
    pub redis: RedisUrl,
    pub work_id: WorkId,
    pub server: Server,
    pub chain: ChainInfo,
    pub account: AccountInfo
}

impl Application {
    pub fn new() -> Result<Self, ConfigError> {
        let mut path_name = "./config/dev/application";
        if ENV_MODE == "production" {
            path_name = "./config/pro/application";
        }
        let s = Config::builder()
            .add_source(File::with_name(path_name))
            .add_source(Environment::with_prefix("APP"))
            .build()?;
        s.try_deserialize()
    }
}
