use std::{str::FromStr, sync::Arc, time::Duration};

use crate::{
    common::{
        structs::{ChainEvent, EventName, OneAccountInfo, OneEvent, ProgramAccounts, TxDetail},
        util,
    },
    const_var::{self, PROGRAM_DATA_STR},
    global_var,
    request::{Client, Root},
};
use anyhow::{anyhow, Result};

use base64::prelude::*;
use futures::StreamExt;
use once_cell::sync::OnceCell;
use reqwest::{Client as HttpClient, Url};
use serde_json::{json, Value};
use solana_client::{
    nonblocking::{pubsub_client::PubsubClient, rpc_client::RpcClient},
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    rpc_response::RpcLogsResponse,
};
use solana_sdk::{
    account::Account,
    commitment_config::{CommitmentConfig, CommitmentLevel},
    hash::hash,
    pubkey::Pubkey,
    signature::Signature,
};
use solana_transaction_status::{
    option_serializer::OptionSerializer, EncodedConfirmedBlock,
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction,
    EncodedTransactionWithStatusMeta, UiMessage, UiTransactionEncoding,
};
use tokio::sync::mpsc::{channel, UnboundedSender};
use tracing::{debug, info};

impl Client {
    pub async fn get_program_accounts(&self, account_size: u32) -> Result<ProgramAccounts> {
        let account = &global_var::get_app_conf().account;
        let res = self
            .post::<Value, Value>(
                "",
                json!({
                        "id": account_size,
                        "jsonrpc": "2.0",
                        "method": "getProgramAccounts",
                        "params": [
                            account.program_id.as_str(),
                            {
                                "encoding": "base64",
                                "filters": [
                                  {
                                    "dataSize": account_size
                                  }
                                ]
                            }
                        ]
                }),
            )
            .await
            .map_err(|e| {
                anyhow!(
                    "client get program account with sieze '{}' failed: {:?}",
                    account_size,
                    e
                )
            })
            .unwrap();

        let mut list = vec![];
        if let Some(li) = res["result"].as_array() {
            for one_li in li.iter() {
                let pk_str = one_li["pubkey"].as_str().unwrap();
                let data_str = one_li["account"]["data"]
                    .get(0)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();
                list.push(OneAccountInfo {
                    pubkey: Pubkey::from_str(pk_str)?,
                    data: BASE64_STANDARD.decode(data_str.trim())?,
                });
            }
        }

        Ok(ProgramAccounts { list })
    }

    pub async fn get_solana_block_by_slots_numbers(
        &self,
        slots: Vec<u64>,
        page_size: Option<usize>,
    ) -> anyhow::Result<Vec<Option<EncodedConfirmedBlock>>> {
        if slots.is_empty() {
            return Ok(vec![]);
        }

        let mut p_size = const_var::DEFAULT_SIZE;
        if let Some(size) = page_size {
            p_size = size;
        }

        let mut trx = vec![];
        let paginated_blocks = slots.chunks(p_size);
        for page in paginated_blocks {
            let mut send_data = vec![];
            page.iter().enumerate().for_each(|(index, block)| {
                send_data.push({
                    json!({
                            "id": util::get_unique_id(),
                            "jsonrpc": "2.0",
                            "method": "getBlock",
                            "params": [block]
                    })
                })
            });

            let res = self
                .post::<Vec<Root<EncodedConfirmedBlock>>, Vec<Value>>("", send_data)
                .await
                .map_err(|e| anyhow!("client get block failed: {:?}", e))
                .unwrap();

            // to avoid query too frequently
            for root in res {
                trx.push(root.result);
            }

            tokio::time::sleep(Duration::from_millis(const_var::ONE_BATCH_TIME)).await;
        }

        // println!("trx:{:#?}", trx[0]);

        Ok(trx)
    }

    pub async fn get_solana_transactions(
        &self,
        tx_hashs: Vec<String>,
        page_size: Option<usize>,
    ) -> anyhow::Result<Vec<Option<EncodedConfirmedTransactionWithStatusMeta>>> {
        if tx_hashs.is_empty() {
            return Ok(vec![]);
        }

        let mut p_size = const_var::DEFAULT_SIZE;
        if let Some(size) = page_size {
            p_size = size;
        }

        let mut trx = vec![];
        let paginated_hashes = tx_hashs.chunks(p_size);
        for page in paginated_hashes {
            let mut send_data = vec![];
            page.iter().enumerate().for_each(|(index, tx_hash)| {
                send_data.push({
                    json!({
                            "id": (index + 1).to_string(),
                            "jsonrpc": "2.0",
                            "method": "getTransaction",
                            "params": [tx_hash]
                    })
                })
            });

            let res = self
                .post::<Vec<Root<EncodedConfirmedTransactionWithStatusMeta>>, Vec<Value>>(
                    "", send_data,
                )
                .await?;

            for root in res {
                trx.push(root.result);
            }
        }

        Ok(trx)
    }
}

// static SCAN_INS: OnceCell<SolanaChain> = OnceCell::new();

pub async fn get_ins() -> Result<SolanaChain> {
    let app = global_var::get_app_conf();
    let ins = SolanaChain::new(
        app.chain.url.clone(),
        app.chain.filter_address.clone(),
        vec![EventName::InscriptionCreated, EventName::ReplicaInsCreated],
    )
    .await?;

    Ok(ins)
    // let v = SCAN_INS.get();
    // if v.is_none() {

    //     v.set(ins)?;
    // }

    // SCAN_INS.get()
}

pub fn get_tx_detail(
    tx: &EncodedTransactionWithStatusMeta,
    filter_accounts: &[String],
) -> Option<TxDetail> {
    let mut rs = TxDetail::default();
    let mut is_set = false;
    if let EncodedTransaction::Json(ui_tx) = &tx.transaction {
        if let UiMessage::Raw(ui_msg) = &ui_tx.message {
            rs.accounts.extend(ui_msg.account_keys.iter().cloned());
            is_set = true;
        }

        if ui_tx.signatures.len() > 0 {
            rs.tx = ui_tx.signatures[0].clone();
            is_set = true;
        }
    }

    if !filter_accounts.is_empty() {
        let new_accounts: Vec<String> = filter_accounts.iter().map(|v| v.to_lowercase()).collect();

        let find = rs
            .accounts
            .iter()
            .any(|e| new_accounts.contains(&e.to_lowercase()));

        if !find {
            return None;
        }
    }

    if let Some(meta) = &tx.meta {
        if let OptionSerializer::Some(lgs) = &meta.log_messages {
            for lg in lgs.iter() {
                let lg = lg.trim();
                if !lg.starts_with(PROGRAM_DATA_STR) {
                    continue;
                }

                let data = lg.trim_start_matches(PROGRAM_DATA_STR);
                let d = BASE64_STANDARD.decode(data.trim()).unwrap();

                rs.logs.push(d);
                // is_set = true;
            }
        };
    }

    if !is_set {
        return None;
    }

    Some(rs)
}

pub struct SolanaChain {
    rpc_url: String,
    ws_url: Option<String>,
    pub rpc_client: Arc<RpcClient>,
    pub pubsub_client: Option<Arc<PubsubClient>>,
    pub client: Arc<Client>,
    pub address: Vec<String>,
    pub filter_evts: Vec<EventName>,
}

impl SolanaChain {
    /// . new
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub async fn new(
        rpc_url: String,
        filter_address: Vec<String>,
        filter_evts: Vec<EventName>,
    ) -> anyhow::Result<Self> {
        // solana rpc client
        let rpc_client = RpcClient::new(rpc_url.clone());
        // http client
        let client = Client::new(rpc_url.clone());
        Ok(SolanaChain {
            rpc_url: rpc_url,
            ws_url: None,
            rpc_client: Arc::new(rpc_client),
            pubsub_client: None,
            client: Arc::new(client),
            address: filter_address,
            filter_evts,
        })
    }

    /// Returns the get max block number of this [`SolanaChain`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub async fn get_max_block_number(&self) -> anyhow::Result<u64> {
        let slot = self.rpc_client.get_slot().await?;
        Ok(slot)
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub async fn get_block_by_number(
        &self,
        block_number: u64,
    ) -> anyhow::Result<EncodedConfirmedBlock> {
        let block = self.rpc_client.get_block(block_number).await?;
        Ok(block)
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub async fn get_transaction(
        &self,
        tx_hash: String,
    ) -> anyhow::Result<EncodedConfirmedTransactionWithStatusMeta> {
        let signature = Signature::from_str(&tx_hash).unwrap();
        let transaction = self
            .rpc_client
            .get_transaction(&signature, UiTransactionEncoding::Json)
            .await?;
        Ok(transaction)
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub async fn get_account(&self, pubkey: String) -> anyhow::Result<Account> {
        let pubkey = Pubkey::from_str(&pubkey)?;
        let account = self.rpc_client.get_account(&pubkey).await?;
        Ok(account)
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub async fn get_account_data(&self, pubkey: String) -> anyhow::Result<Vec<u8>> {
        let pubkey = Pubkey::from_str(&pubkey)?;
        let account_data = self.rpc_client.get_account_data(&pubkey).await?;
        Ok(account_data)
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub async fn get_token_account_balance(&self, pubkey: String) -> anyhow::Result<u64> {
        let pubkey = Pubkey::from_str(&pubkey)?;
        let balance = self.rpc_client.get_balance(&pubkey).await?;
        Ok(balance)
    }

    pub async fn scanning_block(
        &self,
        from_block: u64,
        to_block: Option<u64>,
    ) -> anyhow::Result<Vec<Option<EncodedConfirmedBlock>>> {
        let client_t = self.client.clone();
        match self.rpc_client.get_blocks(from_block, to_block).await {
            Ok(slots) => match client_t
                .get_solana_block_by_slots_numbers(slots, None)
                .await
            {
                Ok(res) => Ok(res),
                Err(e) => {
                    log::error!("{:?}", e);
                    Ok(vec![])
                }
            },
            Err(e) => {
                log::error!("{:?}", e);
                Ok(vec![])
            }
        }
    }

    pub async fn scan_with_detail(
        &self,
        from_block: u64,
        to_block: u64,
        // event_filter: Vec<EventName>
    ) -> anyhow::Result<Vec<ChainEvent>> {
        info!("scan from {} to {}", from_block, to_block);
        let mut arr: Vec<ChainEvent> = vec![];

        // let filter_addr = self.address.clone();
        let client_t = self.client.clone();
        // let to_block = self.rpc_client.get_latest_blockhash().await?;
        // let slot = self.rpc_client.get_slot().await?;
        let f_slots = self
            .rpc_client
            .get_blocks(from_block, Some(to_block))
            .await
            .map_err(|e| anyhow!("get_blocks error: {:?}", e))?;
        //  get_blocks(from_block, to_block).await?;

        // return Ok(vec![]);
        // let mut logs = vec![];

        if f_slots.is_empty() {
            debug!(
                "f_slots is empty from_block:{}, to_block:{}",
                from_block, to_block
            );
        }

        info!("use f_slots len: {}", f_slots.len());

        let res = client_t
            .get_solana_block_by_slots_numbers(f_slots, None)
            .await
            .map_err(|e| anyhow!("get_solana_block_by_slots_number error: {:?}", e))?;

        if res.len() > 0 && res[0].is_some() {
            let block = res[0].as_ref().unwrap();
            info!("process block timestamp: {:?}", block.block_time);
        }

        res.into_iter().filter(|f| f.is_some()).for_each(|block| {
            let block = block.unwrap();
            let mut tx_arr: Vec<TxDetail> = vec![];
            block.transactions.iter().for_each(|tx| {
                if let Some(tx_detail) = get_tx_detail(tx, &self.address[..]) {
                    tx_arr.push(tx_detail);
                }
            });

            // if tx_arr.len() == 0 {
            //     return;
            // }

            let filter_evt = &self.filter_evts;

            debug!(
                "tx arr len {} at block height, {:?}",
                tx_arr.len(),
                block.block_height
            );

            tx_arr.iter().for_each(|v| {
                debug!("tx detail: {:?}", v);
                v.logs.iter().for_each(|log| {
                    if let Some(evt_name) = EventName::parse(log) {
                        if filter_evt.len() > 0 && filter_evt.contains(&evt_name)
                            || filter_evt.is_empty()
                        {
                            arr.push(ChainEvent {
                                tx: v.tx.clone(),
                                slot: block.block_height,
                                evt: OneEvent::new(&log[..]),
                                timestamp: block.block_time,
                            })
                        }
                    }
                });
            });
        });

        arr.iter().for_each(|v| {
            debug!("evt name :{:?}", v.evt.name);
        });
        debug!("====\n\n\n");
        // Ok(logs)

        Ok(arr)
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub async fn scanning_block_logs(
        &self,
        from_block: u64,
        to_block_opt: Option<u64>,
    ) -> anyhow::Result<Vec<String>> {
        let sf_addr = self.address.clone();
        let client_t = self.client.clone();
        // let to_block = self.rpc_client.get_latest_blockhash().await?;
        // let slot = self.rpc_client.get_slot().await?;
        let f_slots = self.rpc_client.get_blocks(from_block, to_block_opt).await?;
        //  get_blocks(from_block, to_block).await?;

        // return Ok(vec![]);
        let mut logs = vec![];

        let res = client_t
            .get_solana_block_by_slots_numbers(f_slots, None)
            .await?;

        res.into_iter().filter(|f| f.is_some()).for_each(|block| {
            let filtered_transactions: Vec<_> = block
                .unwrap()
                .transactions
                .into_iter()
                .filter(|tx| -> bool {
                    match &tx.transaction {
                        EncodedTransaction::Json(ui_tx) => match &ui_tx.message {
                            UiMessage::Raw(ui_msg) => sf_addr.iter().any(|elem_b| {
                                ui_msg.account_keys.iter().any(|elem_a| elem_a == elem_b)
                            }),
                            _ => false,
                        },
                        _ => false,
                    }
                })
                .collect();

            filtered_transactions.iter().for_each(|tx| {
                if let Some(meta) = tx.meta.as_ref() {
                    if let solana_transaction_status::option_serializer::OptionSerializer::Some(
                        lgs,
                    ) = &meta.log_messages
                    {
                        logs.extend(lgs.iter().cloned());
                    };
                }
            });
        });

        Ok(logs)
    }

    /*
    pub async fn pubsub_for_address(
        &self,
        wtx: UnboundedSender<RpcLogsResponse>,
    ) -> anyhow::Result<()> {
        for addr in self.address.iter() {
            let ws_url_t = self.ws_url.clone();
            let program_id = addr.clone();
            let wtx_t = wtx.clone();
            tokio::spawn(async move {
                let filter = RpcTransactionLogsFilter::Mentions(vec![program_id.clone()]);
                let config = RpcTransactionLogsConfig {
                    commitment: Some(CommitmentConfig {
                        commitment: CommitmentLevel::Confirmed,
                    }),
                };

                loop {
                    match PubsubClient::new(&ws_url_t).await {
                        Ok(client) => {
                            match client.logs_subscribe(filter.clone(), config.clone()).await {
                                Ok(mut subscription) => loop {
                                    match subscription.0.next().await {
                                        Some(rsp) => {
                                            if let Some(err) = rsp.value.err {
                                                log::error!("subscript err {:?}", err);
                                                continue;
                                            } else {
                                                let tx_hash = &rsp.value.signature;
                                                log::info!("rx.recv>>>{}", &tx_hash);
                                                if let Err(e) = wtx_t.send(rsp.value) {
                                                    log::error!("[{}] send>>>{:?}", program_id, e);
                                                };
                                            }
                                        }
                                        None => {
                                            log::warn!("subscript over");
                                            break;
                                        }
                                    }
                                },
                                Err(err) => {
                                    log::error!("subscription failed: {:?}", err);
                                }
                            }
                        }
                        Err(err) => {
                            log::error!("reconnect failed: {:?}", err);
                        }
                    };
                    std::thread::sleep(Duration::from_secs(3));
                }
            });
        }

        Ok(())
    }
    */
}
