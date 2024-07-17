use anyhow::anyhow;
use serde_json::Value;
use std::str::FromStr;

use serde::{self, Deserialize, Deserializer, Serialize, Serializer, de};
use solana_sdk::pubkey::Pubkey;

pub fn serialize<S>(pk: &Option<Pubkey>, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(pk) = pk {
        let s = pk.to_string();
        ser.serialize_str(&s)
    } else {
        ser.serialize_none()
    }
}

pub fn deserialize<'de, D>(des: D) -> Result<Option<Pubkey>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(des)?;

    if s.is_none() {
        return Ok(None);
    }

    let pk = Pubkey::from_str(s.as_ref().unwrap())
        .map_err(|e| de::Error::custom(format!("deserialize optional pubkey error, {:?}", e)))?;
    Ok(Some(pk))
}
