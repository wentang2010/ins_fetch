use axum::BoxError;

use http::StatusCode;

use serde::{Deserialize, Serialize};

use fred::pool::RedisPool;

use rustc_hex::ToHex;

use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use std::sync::Arc;
use std::{fmt, str::FromStr};
use thiserror::Error;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;


const MAX_BASE58_LEN: usize = 44;
const ETH_BASE58_LEN: usize = 44;

#[derive(Clone)]
pub struct AppContext {
    // pub pool: Pool,
    // pub redis_pool: RedisPool,
    // pub chain_http_rpc_url: HashMap<String, String>,
    // pub ws_db: WSDB,
}

impl AppContext {
    pub fn new(// pool: Pool,
        // redis_pool: RedisPool,
        // chain_http_rpc_url: HashMap<String, String>,
        // ws_db: WSDB,
    ) -> Self {
        Self {
            // pool,
            // redis_pool,
            // chain_http_rpc_url,
            // ws_db,
        }
    }
}

pub async fn handle_timeout_error(err: BoxError) -> (StatusCode, String) {
    if err.is::<tower::timeout::error::Elapsed>() {
        (
            StatusCode::REQUEST_TIMEOUT,
            "Request took too long".to_string(),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled internal error: {}", err),
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Res<T: Serialize> {
    pub data: Option<T>,
    pub code: u16,
    pub err_msg: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SocketRes<T: Serialize> {
    pub data: Option<T>,
    pub code: u16,
    pub err_msg: Option<String>,
}

impl<T: Serialize> Res<T> {
    pub fn with_data(t: T) -> Self {
        Self {
            data: Some(t),
            code: 0,
            err_msg: None,
        }
    }

    pub fn with_err(code: u16, err_msg: &str) -> Self {
        Self {
            data: None,
            code: code,
            err_msg: Some(err_msg.to_string()),
        }
    }

    pub fn with_none() -> Self {
        Self {
            data: None,
            code: 0,
            err_msg: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        // let v = Key::new_anyhash([10u8, 11, 16].as_slice());
        // println!("{}", v.to_string());
    }
}
