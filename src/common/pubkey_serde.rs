use anyhow::anyhow;
use serde::{de, ser};
use std::str::FromStr;

use serde::{self, Deserialize, Deserializer, Serialize, Serializer};
use solana_sdk::pubkey::Pubkey;

pub fn serialize<S>(pk: &Pubkey, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = pk.to_string();
    ser.serialize_str(&s)
}

pub fn deserialize<'de, D>(des: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(des)?;
    let pk = Pubkey::from_str(&s)
        .map_err(|e| de::Error::custom(format!("deserialize pubkey error, {:?}", e)))?;
    Ok(pk)
}
