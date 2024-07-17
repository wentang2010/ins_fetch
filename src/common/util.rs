use std::{
    collections::HashMap,
    future::Future,
    ops::Div,
    pin::Pin,
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Error, Result};
use borsh::BorshDeserialize;
use fred::{pool::RedisPool, types::Expiration};
use futures_util::StreamExt;
use log::error;
use once_cell::sync::OnceCell;
use reqwest::Response;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    hash::hash,
    pubkey::Pubkey,
};
use tokio::sync::mpsc;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::request::Client as HttpClient;
use crate::{const_var, file, global_var};

use super::{
    dao::InsMetaData,
    idgen::IdGen,
    structs::{AccountBook, AccountBookList, JsonUrl, MetaplexData},
};

pub fn is_event(event_name: impl Into<String>, log: &[u8]) -> bool {
    let evt_name: String = event_name.into();
    let discriminator_preimage = format!("event:{}", evt_name);
    // let mut discriminator = [0u8; 8];
    let discriminator = &hash(discriminator_preimage.as_bytes()).to_bytes()[..8];
    &log[..8] == &discriminator[..]
}

pub fn get_unix_secs() -> u64 {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    time
}

pub fn save_scan_start(d: u64) {
    let s = json!({
        "start": d
    });
    let s1 = s.to_string();
    file::write_file("conf", s1.as_str()).unwrap();
}

pub fn get_scan_start() -> Result<u64> {
    let s = file::read_file("conf")?;
    let v: Value = serde_json::from_str(&s)?;
    let start = v["start"]
        .as_u64()
        .expect("in get_file_start, get start failed");

    Ok(start)
}

static CHAIN_RPC_CLIENT: OnceCell<RpcClient> = OnceCell::new();

static HTTP_CLIENT: OnceCell<HttpClient> = OnceCell::new();

pub fn get_rpc_client() -> &'static RpcClient {
    CHAIN_RPC_CLIENT.get_or_init(|| {
        let app = global_var::get_app_conf();
        let rpc_client = RpcClient::new(app.chain.url.clone());
        rpc_client
    })
}

pub fn get_http_client() -> &'static HttpClient {
    HTTP_CLIENT.get_or_init(|| {
        let app = global_var::get_app_conf();
        HttpClient::new(app.chain.url.clone())
    })
}

pub async fn get_account_book_data() -> Result<Vec<u8>> {
    let pk = Pubkey::from_str(global_var::get_app_conf().account.account_book.as_str())?;
    let rpc_client = get_rpc_client();
    Ok(rpc_client.get_account_data(&pk).await?)
}

pub async fn get_simple_metaplex_data(pk: Pubkey) -> Result<MetaplexData> {
    if pk == Pubkey::default() {
        return Ok(MetaplexData::default());
    }
    let rpc_client = get_rpc_client();
    // info!("process metaplex pk: {:?}", pk);
    let data = rpc_client.get_account_data(&pk).await?;

    // skip key enum. two pub  = 65 bytes.
    let mut m_data: MetaplexData = BorshDeserialize::deserialize(&mut &data[65..])?;
    m_data.name = m_data.name.trim_end_matches("\0").to_string();
    m_data.symbol = m_data.symbol.trim_end_matches("\0").to_string();
    m_data.uri = m_data.uri.trim_end_matches("\0").to_string();

    Ok(m_data)
}

pub fn get_program_id() -> Result<Pubkey> {
    let conf = global_var::get_app_conf();
    let pk = Pubkey::from_str(&conf.account.program_id)?;
    Ok(pk)
}

pub async fn get_ins_meta_data(ins_key: &Pubkey) -> Result<Option<InsMetaData>> {
    let rpc_client = get_rpc_client();
    let (ins_meta_key, _) =
        Pubkey::find_program_address(&[b"ins_meta_seed", ins_key.as_ref()], &get_program_id()?);

    let ins_data = rpc_client
        .get_account_with_commitment(
            &ins_meta_key,
            CommitmentConfig {
                commitment: CommitmentLevel::Processed,
            },
        )
        .await?;

    if let Some(account) = ins_data.value {
        let rs_data: Result<InsMetaData, std::io::Error> =
            BorshDeserialize::deserialize(&mut &account.data[8..]);
        if rs_data.is_err() {
            return Ok(None);
        }

        return Ok(Some(rs_data.unwrap()));
    }

    Ok(None)
}

pub async fn get_metaplex_data(pk: Pubkey) -> Result<(MetaplexData, JsonUrl)> {
    if pk == Pubkey::default() {
        return Ok((MetaplexData::default(), JsonUrl::default()));
    }
    let rpc_client = get_rpc_client();
    // info!("process metaplex pk: {:?}", pk);
    let data = rpc_client.get_account_data(&pk).await?;

    // skip key enum. two pub  = 65 bytes.
    let mut m_data: MetaplexData = BorshDeserialize::deserialize(&mut &data[65..])?;
    m_data.name = m_data.name.trim_end_matches("\0").to_string();
    m_data.symbol = m_data.symbol.trim_end_matches("\0").to_string();
    m_data.uri = m_data.uri.trim_end_matches("\0").to_string();

    let http_client = get_http_client();
    let uri_data = http_client.get::<Value>(m_data.uri.as_str()).await?;
    // println!("uri data: {:?}", uri_data);

    let mut json_url = JsonUrl::default();
    json_url.image = uri_data["image"].as_str().unwrap_or_default().to_string();
    let mut attributes = vec![];
    if let Some(arr) = uri_data["attributes"].as_array() {
        for one_attr in arr.iter() {
            let key = one_attr["key"].as_str().unwrap_or_default().to_string();
            let value = one_attr["value"].as_str().unwrap_or_default().to_string();
            let map: HashMap<String, String> =
                HashMap::from([("key".to_string(), key), ("value".to_string(), value)]);
            attributes.push(map);
        }
    }

    json_url.attributes = attributes;

    Ok((m_data, json_url))
}

static mut ab_last_time: OnceCell<u64> = OnceCell::with_value(0);
static mut ab_list: OnceCell<AccountBookList> = OnceCell::new();

pub async fn get_account_book_sync() -> Result<AccountBookList> {
    let account_data = get_account_book_data().await?;
    let list = AccountBookList::deserialize(&mut &account_data[..])?;

    Ok(list)
}

// pub async fn get_account_book_list(refresh: bool) -> Result<&'static AccountBookList> {
//     let now = get_unix_secs();
//     if refresh {
//         unsafe {
//             ab_last_time.take();
//             ab_last_time.set(0).unwrap();
//         }
//     }
//     let prev_time = unsafe { ab_last_time.get().unwrap() };
//     if prev_time + 60 * 10 < now {
//         let account_data = get_account_book_data().await?;
//         let list = AccountBookList::deserialize(&mut &account_data[..])?;
//         unsafe {
//             ab_list.take();
//             ab_list.set(list).unwrap();
//             ab_last_time.take();
//             ab_last_time.set(now).unwrap();
//         }
//     }

//     Ok(unsafe { ab_list.get().unwrap() })
// }

pub fn get_unique_id() -> String {
    let nonce = IdGen::next_id();
    if nonce < 0 {
        //
        error!("nonce should not be negative");
        // TODO: // return error is fine in the future
        panic!("nonce should not be negative");
    }

    let nonce = nonce as u64;
    let time_bytes = get_unix_secs().to_be_bytes();
    let nonce_bytes = nonce.to_be_bytes();

    let time_hex = hex::encode(time_bytes);
    let nonce_hex = hex::encode(nonce_bytes);

    format!("{time_hex}_{nonce_hex}")
}

pub fn create_time_order_uid(unix_timetamp: u64) -> (String, u64) {
    let nonce = IdGen::next_id();
    if nonce < 0 {
        //
        error!("nonce should not be negative");
        // TODO: // return error is fine in the future
        panic!("nonce should not be negative");
    }
    let nonce = nonce as u64;
    let time_bytes = unix_timetamp.to_be_bytes();
    let nonce_bytes = nonce.to_be_bytes();
    let time_hex = hex::encode(time_bytes);
    let nonce_hex = hex::encode(nonce_bytes);
    (format!("{time_hex}_{nonce_hex}"), nonce)
}

pub fn set_log_conf() {
    // if std::env::var_os("RUST_LOG").is_none() {
    //     std::env::set_var("RUST_LOG", "ins_fetch=debug,hyper=info");
    // } else {
    //     let rust_log = std::env::var_os("RUST_LOG").unwrap();
    //     let rust_log_str = rust_log.to_str().map(|s| s.to_string()).unwrap_or_default();
    //     if !rust_log_str.contains("ins_fetch") {
    //         std::env::set_var("RUST_LOG", "ins_fetch=debug,hyper=info");
    //     }
    //     println!("Original RUST_LOG: {:?}", rust_log);
    // }
    std::env::set_var("RUST_LOG", "ins_fetch=info,hyper=info");

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_file(true)
        .with_line_number(true)
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}

// pub async fn save_into_redis<T: Serialize + DeserializeOwned>(
//     key: String,
//     val: T,
//     redis_pool: &RedisPool,
// ) -> Result<()> {
//     let val_str = serde_json::to_string(&val)?;
//     redis_pool
//         .set::<(), _, _>(key, val_str, None, None, false)
//         .await?;
//     Ok(())
// }

// pub async fn save_into_redis_for_cache<T: Serialize + DeserializeOwned>(
//     key: String,
//     val: T,
//     redis_pool: &RedisPool,
// ) -> Result<()> {
//     let val_str = serde_json::to_string(&val)?;
//     let set = redis_pool
//         .set::<(), _, _>(key, val_str, Some(Expiration::EX(60 * 2)), None, false)
//         .await?;
//     Ok(())
// }

// pub async fn get_from_redis<T: Serialize + DeserializeOwned>(
//     key: String,
//     redis_pool: &RedisPool,
// ) -> Result<Option<T>> {
//     let exist: Option<String> = redis_pool
//         .get(key.clone())
//         .await
//         .map_err(|e| anyhow!("get val from redis failed, key: {key}, err:{e:?}"))?;
//     if exist.is_none() {
//         return Ok(None);
//     }
//     let item = exist.unwrap();
//     let data: T = serde_json::from_str(&item).map_err(|e| {
//         anyhow!("get_from_redis serde_json::from_str err:{e:?},  item_str:{item:?}")
//     })?;

//     Ok(Some(data))
// }

#[cfg(test)]
mod tests {
    use crate::config::application::Application;

    use super::*;

    #[tokio::test]
    async fn test_metaplex_data() -> Result<()> {
        set_log_conf();
        let application = Application::new().unwrap();
        global_var::init_app_conf(application);
        // let db_ins = create_db_ins().await?;
        // set_db_ins(db_ins);
        let pk = Pubkey::from_str("BFK2rW6mfaQvwLeaa5nkdfpDyCY4tcz33XKL3mpxws9R").unwrap();
        let (m_data, uri_data) = get_metaplex_data(pk).await?;

        println!("{:?}, uri:{:?}", m_data, uri_data);
        Ok(())
    }
    // use crate::system_var;
}
