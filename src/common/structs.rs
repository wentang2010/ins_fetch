use std::{collections::HashMap, fmt};

use crate::{chain_scan, config::application::AccountInfo, const_var, global_var};

use super::{opt_pubkey_serde, pubkey_serde, util};
use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use futures_util::sink::Send;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;

// db data

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct AccountBookList {
    pub list: Vec<AccountBook>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct AccountBook {
    pub residue: u64,
    pub amount: u64,
}
impl AccountBook {
    pub const LEN: usize = 8 + 8;
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct MetaplexData {
    /// The name of the asset
    pub name: String,
    /// The symbol for the asset
    pub symbol: String,
    /// URI pointing to JSON representing the asset
    pub uri: String,
    /// Royalty basis points that goes to creators in secondary sales (0-10000)
    pub seller_fee_basis_points: u16,
    /// Array of creators, optional
    pub creators: Option<Vec<Creator>>,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct InsMetaData {
    pub total_supply: u64, // the maximum supply of token.
    // pub token_name: String,   // max char is 12
    // pub token_symbol: String, // max char is 9
    pub start_time: u64,
    pub end_time: u64,
    // these amount will stake into raydium pool.
    pub sol_pool_ratio: u32, // 3 decimal. eg. 123 means 12.3% of minter fee will be transfed into sol pool.

    // ins token ratio
    pub user_pool_ratio: u32,
    pub lp_pool_ratio: u32,
    pub minter_pool_ratio: u32,

    pub supply_token: bool, // if ins project supply token
    pub is_completed: bool,
    pub _reserved: [u8; 20],
    pub _reserved_pk: [u8; 32],
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct JsonUrl {
    /// The picture of the img url.
    pub name: String,
    pub image: String,
    pub image_type: String,
    pub symbol: String,
    pub attributes: Vec<HashMap<String, String>>, // {key: "",value: ""}
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct Creator {
    #[serde(with = "pubkey_serde")]
    pub address: Pubkey,
    pub verified: bool,
    // In percentages, NOT basis points ;) Watch out!
    pub share: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Conf {
    pub start: u64,
}

#[derive(Deserialize, Serialize, BorshDeserialize, Clone, Debug)]
pub enum Category {
    Json,
    Blob,
    Image,
    Txt,
    Audio,
    Video,
    App,
}
impl Category {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Category::Json => &[0u8],
            Category::Blob => &[1u8],
            Category::Image => &[2u8],
            Category::Txt => &[3u8],
            Category::Audio => &[4u8],
            Category::Video => &[5u8],
            Category::App => &[6u8],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgramAccounts {
    pub list: Vec<OneAccountInfo>,
}

#[derive(Debug, Clone)]
pub struct OneAccountInfo {
    pub pubkey: Pubkey,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplicaTx {
    pub key: String, // signature
    pub evt: ChainEvent,
    pub replica_ins: ReplicaIns,
    pub sort_value: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SocialInfoDB {
    #[serde(with = "pubkey_serde")]
    pub key: Pubkey, // inscription account pubkey
    pub website: String,
    pub telegram: String,
    pub twitter: String,
    pub discord: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InscriptionList {
    pub list: Vec<Inscription>,
    pub total: u64,
    pub deployed_count: u64,
    pub query_count: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Inscription {
    #[serde(with = "pubkey_serde")]
    pub key: Pubkey,
    pub data: InscriptionData,
    pub metaplex_data: MetaplexData,
    pub ins_meta_data: Option<InsMetaData>,
    pub uri_data: JsonUrl,
    pub sort_value: u64, // agg value
}

impl Inscription {
    pub async fn from_data(d: &InsData) -> Result<Self> {
        let rpc_client = util::get_rpc_client();
        let data = rpc_client.get_account_data(&d.key).await?;

        // skip 8 bytes delimiter
        let ins_data: InscriptionData = BorshDeserialize::deserialize(&mut &data[8..])?;
        let ins_meta_data = util::get_ins_meta_data(&d.key).await?;
        let (m_data, uri_data) = util::get_metaplex_data(ins_data.master_metadata).await?;
        Ok(Inscription {
            key: d.key,
            data: ins_data,
            metaplex_data: m_data,
            ins_meta_data,
            uri_data: uri_data,
            sort_value: 0,
        })
    }

    pub async fn from_account(one_ac: &OneAccountInfo) -> Result<Self> {
        // let rpc_client = util::get_rpc_client();
        // let data = rpc_client.get_account_data(&d.key).await?;
        // skip 8 bytes delimiter
        let ins_data: InscriptionData = BorshDeserialize::deserialize(&mut &one_ac.data[8..])?;
        let ins_meta_data = util::get_ins_meta_data(&one_ac.pubkey).await?;
        let (m_data, uri_data) = util::get_metaplex_data(ins_data.master_metadata).await?;
        Ok(Inscription {
            key: one_ac.pubkey,
            data: ins_data,
            metaplex_data: m_data,
            ins_meta_data,
            uri_data: uri_data,
            sort_value: 0,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InscriptionMeta {
    // pub cover: String,
    #[serde(with = "pubkey_serde")]
    pub meta_data_key: Pubkey, // will be used in future
    pub used_num: u64,
    pub mint_num: u64, // may be used
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoreData {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, BorshDeserialize, Clone)]
pub struct InscriptionData {
    #[serde(with = "pubkey_serde")]
    pub publisher: Pubkey,
    /// The pubkey of the master token mint (32).
    #[serde(with = "pubkey_serde")]
    pub master_mint: Pubkey,
    /// The pubkey of the MPL master metadata account (32).
    #[serde(with = "pubkey_serde")]
    pub master_metadata: Pubkey,
    /// The pubkey of the master edition mint (32).
    #[serde(with = "pubkey_serde")]
    pub master_edition: Pubkey,
    /// The optional pubkey of the Inscription content (33).
    #[serde(with = "opt_pubkey_serde")]
    pub inscrip_content: Option<Pubkey>,
    /// The optional pubkey of the Inscription compose account (33).
    #[serde(with = "opt_pubkey_serde")]
    pub compose_account: Option<Pubkey>,
    /// The `Category` enum variant to assign the category of Inscription (1).
    pub category: Category,
    /// The optional finite supply of installations available for this Inscription (9).
    pub supply: Option<u64>,
    /// Total amount of install accounts that have been created for this Inscription (8).
    pub total_replicas: u64,
    /// The price-per-install of this Inscription (8).
    pub replica_price: u64,
    /// The unix timestamp of when the account was created (8).
    pub created_ts: i64,
    /// The unix timestamp of the last time the account was updated (8).
    pub deployed_ts: i64,
    /// The share the marketplace takes on all trades (2).
    pub royalty: u16,
    /// The Inscription index of account_book
    /// start from 1.
    pub index: u32,
    /// Flag used to determine whether NFTs can be installed (1).
    pub deployed: bool,
    /// Flag used to determine  it is a inscription Inscription(1).
    pub inscripted: bool,
    // for self instruction
    pub is_combained_type: bool,
    /// Flag used to determine whether it is a combination Inscription (1).
    pub combinated: bool,
    /// Flag to determine whether new installations of the Inscription should be halted (1).
    pub suspended: bool,
    pub bump: [u8; 1],
    /// The optional pubkey of the WhiteList content (33).
    #[serde(with = "opt_pubkey_serde")]
    pub whitelist: Option<Pubkey>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplicaInsList {
    pub list: Vec<ReplicaIns>,
    pub total: u64,
    pub sub_replica_count: u64,
    pub query_count: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplicaInsGroupList {
    pub list: Vec<OneGroupReplicaIns>,
    pub total: u64,
    pub query_count: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplicaIns {
    #[serde(with = "pubkey_serde")]
    pub key: Pubkey,
    pub data: ReplicaInsData,
    pub sort_value: u64,
    // pub meta: ReplicaIns
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OneGroupReplicaIns {
    #[serde(with = "pubkey_serde")]
    pub minter: Pubkey,
    #[serde(with = "pubkey_serde")]
    pub inscription: Pubkey,
    pub count: u64,
    pub sort_value: u64,
    // pub meta: ReplicaIns
}

#[derive(Debug, Serialize, Deserialize, BorshDeserialize, Clone)]
pub struct ReplicaInsData {
    /// The authority who created the installation (32).
    #[serde(with = "pubkey_serde")]
    pub minter: Pubkey,
    // The NFT mint
    #[serde(with = "pubkey_serde")]
    pub mint: Pubkey,
    /// Copy timestamp
    pub timestamp: i64,
    /// The pubkey of the Inscription that was installed (32).
    #[serde(with = "pubkey_serde")]
    pub inscription: Pubkey,

    /// The sequential installation number of the Inscription (8).
    /// Copy timestamp
    pub seed_timestamp: u64,
    /// The sequential installation number of the Inscription (8).
    /// start from 1.
    pub no: u64,
    // the bump seed
    pub bump: [u8; 1],
    // for self instruction
    pub is_combained_type: bool,
    // for self instruction.  de combain the instruction for combained instructions
    pub in_destructing: bool,
    // which one of inscription will be destructed.
    pub in_destruction_index: u16,
    // the number of total inscriptions.
    pub total_inscriptions: u16,
}

impl ReplicaIns {
    pub async fn from_data(d: &CopyInsData) -> Result<Self> {
        let rpc_client = util::get_rpc_client();
        let data = rpc_client.get_account_data(&d.replica_ins).await?;

        let copy_ins_data: ReplicaInsData = BorshDeserialize::deserialize(&mut &data[8..])?;
        Ok(ReplicaIns {
            key: d.replica_ins,
            data: copy_ins_data,
            sort_value: 0,
        })
    }

    pub async fn from_account(one_ak: &OneAccountInfo) -> Result<Self> {
        let copy_ins_data: ReplicaInsData = BorshDeserialize::deserialize(&mut &one_ak.data[8..])?;
        Ok(ReplicaIns {
            key: one_ak.pubkey,
            data: copy_ins_data,
            sort_value: 0,
        })
    }
}
// end db data

// transaction details
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TxDetail {
    pub tx: String,
    pub accounts: Vec<String>,
    pub logs: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ChainEvent {
    pub tx: String,
    pub slot: Option<u64>,
    pub evt: OneEvent,
    pub timestamp: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventName {
    #[default]
    InscriptionCreated,
    ReplicaInsCreated,
}

//event data
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct InsData {
    pub key: Pubkey,
}
// event data
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct CopyInsData {
    pub minter: Pubkey,
    pub receiver: Pubkey,
    pub inscription: Pubkey,
    pub replica_ins: Pubkey,
}

impl InsData {
    pub fn new(d: &[u8]) -> Self {
        let mut d1 = d;
        InsData::deserialize(&mut d1).unwrap()
    }
}

impl CopyInsData {
    pub fn new(d: &[u8]) -> Self {
        let mut d1 = d;
        CopyInsData::deserialize(&mut d1).unwrap()
    }
}

impl EventName {
    pub fn to_string(&self) -> String {
        match self {
            EventName::InscriptionCreated => String::from("InscriptionCreated"),
            EventName::ReplicaInsCreated => String::from("InstallationCreated"),
        }
    }

    pub fn new(d: &[u8]) -> Self {
        Self::parse(d).expect(format!("unknown event, {:?}", d).as_str())

        // error!("unknow event");
        // panic!("unknow event {:?}", d);
    }

    pub fn parse(d: &[u8]) -> Option<Self> {
        if util::is_event(EventName::InscriptionCreated.to_string(), d) {
            return Some(EventName::InscriptionCreated);
        } else if util::is_event(EventName::ReplicaInsCreated.to_string(), d) {
            return Some(EventName::ReplicaInsCreated);
        }

        None
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OneEvent {
    pub name: EventName,
    pub data_raw: Vec<u8>,
}

impl OneEvent {
    pub fn new(d: &[u8]) -> Self {
        Self {
            name: EventName::new(d),
            data_raw: d[8..].to_vec(),
        }
    }
}
