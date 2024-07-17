use crate::common::dao::{MetaplexData, SocialInfoDB, StoreData};
use crate::common::idgen::IdGen;

use crate::common::scheduler_util::fetch_task;
use crate::common::webcommon::{AppContext, Res};
use crate::common::{dao, util};

use axum::extract::Path;
use axum::extract::Query;
use axum::routing::post;
use axum::TypedHeader;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};

use dao::{InscriptionList, ReplicaInsGroupList, ReplicaInsList};
use solana_sdk::pubkey::Pubkey;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::RwLock;

use serde::{Deserialize, Serialize};
//allows to extract the IP of connecting user
use anyhow::{anyhow, Result};
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::CloseFrame;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::Arc;

use crate::{db, global_var};

use futures_util::SinkExt;
use futures_util::StreamExt;
use serde_json::json;
use std::str::FromStr;
use std::time::Duration;
use tracing::{error, info, warn};

pub fn create_route() -> Router<AppContext> {
    Router::new()
        // .route("/api/clickhose_init", get(setup_init))
        // .route("/api/tokenicon_init", get(tokenicon_init))
        // .route("/api/cmc_order", get(tokenicon_init))
        //   .route("/api/admin/pull_coin_poolids", get(pull_coin_poolids))
        // icon service
        // .route("/api/trigger_update", get(trigger_update))
        .route("/api", axum::routing::get(|| async { "Inscription API " }))
        .route("/api/all_ins", get(api_get_all_ins))
        .route("/api/replica_ins", get(api_get_replica_ins))
        .route("/api/replica_group_ins", get(api_get_replica_group_ins))
        .route("/api/replica_txs", get(api_replica_txs))
        .route("/api/social_info", get(api_social_info))
        .route("/api/save_social_info", post(api_save_social_info))
        .route(
            "/api/get_simple_metaplex_data",
            get(api_get_simple_metaplex_data),
        )
        .route("/api/save_or_get_data", post(api_save_or_get_data))
}

fn get_page_params(params: &HashMap<String, String>) -> (usize, usize, String) {
    let size = params
        .get("page_size")
        .unwrap_or(&"100".to_string())
        .parse::<usize>()
        .unwrap_or(100);

    let page_index = params
        .get("page_index")
        .unwrap_or(&"0".to_string())
        .parse::<usize>()
        .unwrap_or(0);

    let sort_type = params
        .get("sort_type")
        .unwrap_or(&"default".to_string())
        .to_lowercase();

    (size, page_index, sort_type)
}

async fn trigger_update(State(app_context): State<AppContext>) -> Json<Res<u8>> {
    let mutex = global_var::get_fetch_mutex();
    let mut num = mutex.lock().await;
    info!("trigger start update");
    fetch_task().await;
    info!("trigger end update");
    *num = 2;

    Json(Res::with_none())
}

//api_get_all_ins
async fn api_get_all_ins(
    State(app_context): State<AppContext>,
    // Path((chain_id, from_token, to_token)): Path<(String, String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Res<InscriptionList>> {
    let (size, page_index, sort_type) = get_page_params(&params);
    let range_type = params.get("range_type").cloned();
    let key = params.get("key").cloned();
    let publisher = params.get("publisher").cloned();
    let search_type = params.get("search_type").cloned();
    let search_str = params.get("search_str").cloned();
    let start_time_filter = params.get("start_time_filter").cloned();

    match db::get_db_ins()
        .get_all_ins(
            size,
            page_index,
            sort_type,
            range_type,
            key,
            publisher,
            search_type,
            search_str, // limit,
            start_time_filter,
        )
        .await
    {
        Ok(data) => Json(Res::with_data(data)),
        Err(e) => {
            error!("the api_get_all_ins query error---->{e:?}");
            Json(Res::with_err(1, format!("error {:?}", e).as_str()))
        }
    }
}

//api_get_replica_ins
async fn api_get_replica_ins(
    State(app_context): State<AppContext>,
    // Path((chain_id, from_token, to_token)): Path<(String, String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Res<ReplicaInsList>> {
    let (size, page_index, sort_type) = get_page_params(&params);
    let parent_ins = params.get("parent_ins").cloned();
    let minter = params.get("minter").cloned();
    let key = params.get("key").cloned();

    match db::get_db_ins()
        .get_replica_ins(size, page_index, sort_type, parent_ins, minter, key)
        .await
    {
        Ok(data) => Json(Res::with_data(data)),
        Err(e) => {
            error!("the api_get_replica_ins query error---->{e:?}");
            Json(Res::with_err(1, format!("error {:?}", e).as_str()))
        }
    }
}

//api_get_replica_ins
async fn api_get_replica_group_ins(
    State(app_context): State<AppContext>,
    // Path((chain_id, from_token, to_token)): Path<(String, String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Res<ReplicaInsGroupList>> {
    let (size, page_index, sort_type) = get_page_params(&params);
    let parent_ins = params.get("parent_ins").cloned();
    let minter = params.get("minter").cloned();
    let key = params.get("key").cloned();

    match db::get_db_ins()
        .get_replica_group_ins(size, page_index, sort_type, parent_ins, minter, key)
        .await
    {
        Ok(data) => Json(Res::with_data(data)),
        Err(e) => {
            error!("the api_get_replica_group_ins query error---->{e:?}");
            Json(Res::with_err(1, format!("error {:?}", e).as_str()))
        }
    }
}

async fn api_replica_txs(
    State(app_context): State<AppContext>,
    // Path((chain_id, from_token, to_token)): Path<(String, String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Res<Vec<dao::ReplicaTx>>> {
    let (size, page_index, sort_type) = get_page_params(&params);

    match db::get_db_ins()
        .get_replica_txs(size, page_index, sort_type)
        .await
    {
        Ok(data) => Json(Res::with_data(data)),
        Err(e) => {
            error!("the api_txs query error---->{e:?}");
            Json(Res::with_err(1, format!("error {:?}", e).as_str()))
        }
    }
}

async fn api_social_info(
    State(app_context): State<AppContext>,
    // Path((chain_id, from_token, to_token)): Path<(String, String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Res<dao::SocialInfoDB>> {
    let (size, page_index, sort_type) = get_page_params(&params);
    let key = params.get("key").cloned();

    match db::get_db_ins()
        .get_social_info(size, page_index, sort_type, key)
        .await
    {
        Ok(data) => Json(Res::with_data(data)),
        Err(e) => {
            error!("the api_social query error---->{e:?}");
            Json(Res::with_err(1, format!("error {:?}", e).as_str()))
        }
    }
}

async fn api_save_social_info(
    State(app_context): State<AppContext>,
    // Path((chain_id, from_token, to_token)): Path<(String, String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Res<u8>> {
    let (size, page_index, sort_type) = get_page_params(&params);
    let key = params.get("key").cloned();
    let website = params.get("website").cloned();
    let twitter = params.get("twitter").cloned();
    let telegram = params.get("telegram").cloned();
    let discord = params.get("discord").cloned();

    if key.is_none()
        || website.is_none()
        || twitter.is_none()
        || telegram.is_none()
        || discord.is_none()
    {
        return Json(Res::with_err(1, "some params is empty"));
    }
    let key_str = key.unwrap();

    let pk = Pubkey::from_str(key_str.as_str());
    if pk.is_err() {
        return Json(Res::with_err(1, "parse key failed"));
    }

    match db::get_db_ins()
        .save_social_info(
            size,
            page_index,
            sort_type,
            SocialInfoDB {
                key: pk.unwrap(),
                website: website.unwrap(),
                telegram: telegram.unwrap(),
                twitter: twitter.unwrap(),
                discord: discord.unwrap(),
            },
        )
        .await
    {
        Ok(_) => Json(Res::with_none()),
        Err(e) => {
            error!("the save_api_social error---->{e:?}");
            Json(Res::with_err(1, format!("error {:?}", e).as_str()))
        }
    }
}

async fn api_get_simple_metaplex_data(
    State(app_context): State<AppContext>,
    // Path((chain_id, from_token, to_token)): Path<(String, String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Res<MetaplexData>> {
    let key = params.get("key").cloned().unwrap();
    let data = util::get_simple_metaplex_data(Pubkey::from_str(&key).unwrap()).await;

    match data {
        Ok(data) => Json(Res::with_data(data)),
        Err(e) => {
            error!("the api_get_simple_metaplex_data error---->{e:?}");
            Json(Res::with_err(1, format!("error {:?}", e).as_str()))
        }
    }
}

async fn api_save_or_get_data(
    State(app_context): State<AppContext>,
    Query(params): Query<HashMap<String, String>>,
    Json(data): Json<StoreData>,
) -> Json<Res<StoreData>> {
    let del = params.get("del").cloned();
    let rs = db::get_db_ins().save_or_get_data(data, del.is_some()).await;
    match rs {
        Ok(Some(data)) => Json(Res::with_data(data)),
        Ok(None) => Json(Res::with_none()),
        Err(e) => {
            error!("the api_save_or_get_data error---->{e:?}");
            Json(Res::with_err(1, format!("error {:?}", e).as_str()))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::application::Application;
}
