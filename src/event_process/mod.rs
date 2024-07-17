use fred::pool::RedisPool;
use futures_util::lock::Mutex;
use futures_util::StreamExt;
use once_cell::sync::OnceCell;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::common::structs::ChainEvent;
use crate::{chain_scan, common::util};
use crate::{const_var, db};

use anyhow::{anyhow, Result};
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message},
};

use crate::global_var;

// use crate::const_var::CHANNEL_SIZE;
// use crate::const_var::PRICE_ORACLE_DOMAIN_URL;
// use crate::const_var::PRICE_ORACLE_WS;

pub fn start(// chain_configs: &ChainConfigs,
        // ws_db: WSDB,
) -> Result<()> {
    tokio::spawn(async move {
        // return;
        loop {
            let recv_task = tokio::spawn(async move {
                loop {
                    let db_ins = db::get_db_ins();
                    let scan_ins = chain_scan::solana::get_ins().await.unwrap();
                    let mut start: u64 = util::get_scan_start().unwrap();
                    let latest_slot = scan_ins.rpc_client.get_slot().await.unwrap();
                    if start == 0 {
                        // we should init it.
                        util::save_scan_start(latest_slot);
                        start = latest_slot;
                    }

                    let mut end: u64 = start + const_var::TX_FETCH_STEP;
                    if latest_slot < end {
                        end = latest_slot;
                    }

                    // end = 280104187;

                    if start >= end {
                        tokio::time::sleep(Duration::from_secs(20)).await;
                        continue;
                    }

                    let evts: Result<Vec<ChainEvent>> = scan_ins.scan_with_detail(start, end).await;
                    if evts.is_err() {
                        let err_msg = format!(
                            "scan chain from {} to {} failed: {:?}",
                            start,
                            end,
                            evts.unwrap_err()
                        );
                        error!(err_msg);
                        panic!("abort this thread, {}", err_msg);
                    } else {
                        let evts = evts.unwrap();
                        db_ins.insert_if_not_exist_evts(evts).await.unwrap();
                        util::save_scan_start(end + 1);
                    }

                    tokio::time::sleep(Duration::from_millis(800)).await;
                    debug!("finish one fetch step");
                }
            });

            tokio::select! {
                _=recv_task => {
                    info!("received task exist");
                }
            }

            info!("chain scan exist. will start it later");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use solana_transaction_status::EncodedConfirmedBlock;

    use crate::{
        config::application::Application,
        db::{create_db_ins, get_db_ins, set_db_ins},
        request::Root,
    };

    use super::*;

    #[tokio::test]
    async fn test_http() -> Result<()> {
        util::set_log_conf();
        let application = Application::new().unwrap();
        global_var::init_app_conf(application);
        let db_ins = create_db_ins().await?;
        set_db_ins(db_ins);
        let db_ins = get_db_ins();
        let scan_ins = chain_scan::solana::get_ins().await.unwrap();
        let res = scan_ins
            .client
            .post::<Vec<Root<EncodedConfirmedBlock>>, Vec<Value>>(
                "",
                vec![json!({
                        "id": "0",
                        "jsonrpc": "2.0",
                        "method": "getBlock",
                        "params": [280104188] // 280104186
                })],
            )
            .await
            .map_err(|e| anyhow!("client get block failed: {:?}", e))
            .unwrap();

        println!("res len {:?}", res.len());
        println!("res: {:?}", res);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_program_account() -> Result<()> {
        util::set_log_conf();
        let application = Application::new().unwrap();
        global_var::init_app_conf(application);
        let db_ins = create_db_ins().await?;
        set_db_ins(db_ins);

        let account = &global_var::get_app_conf().account;
        let db_ins = get_db_ins();
        let scan_ins = chain_scan::solana::get_ins().await.unwrap();
        let res = scan_ins
            .client
            .post::<Value, Value>(
                "",
                json!({
                        "id": "1",
                        "jsonrpc": "2.0",
                        "method": "getProgramAccounts",
                        "params": [
                            account.program_id.as_str(),
                            {
                                "encoding": "base64",
                                "filters": [
                                  {
                                    "dataSize": 289
                                  }
                                ]
                            }
                        ]
                }),
            )
            .await
            .map_err(|e| anyhow!("client get program account failed: {:?}", e))
            .unwrap();

        // println!("res len {:?}", res.len());
        println!("res: {:?}", res);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_program_account_v1() -> Result<()> {
        util::set_log_conf();
        let application = Application::new().unwrap();
        global_var::init_app_conf(application);
        let db_ins = create_db_ins().await?;
        set_db_ins(db_ins);

        let account = &global_var::get_app_conf().account;
        let db_ins = get_db_ins();
        let scan_ins = chain_scan::solana::get_ins().await.unwrap();
        let res = scan_ins.client.get_program_accounts(289).await?;

        // println!("res len {:?}", res.len());
        println!("res: {:?}", res);

        Ok(())
    }
}
