use axum::routing::get_service;
use axum::Json;
use axum::{error_handling::HandleErrorLayer, Router};

use http::StatusCode;

use hyper::service;
use mongodb::bson::doc;
use mongodb::options::FindOptions;
use serde_json::json;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing_subscriber::{prelude::*, EnvFilter};

use std::env::var_os;
use std::net::SocketAddr;

use anyhow::{Error, Result};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceBuilder;
use tracing::{error, info, warn};

use axum::http::Method;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

use futures::stream::TryStreamExt;
use mongodb::{options::ClientOptions, Client};
use serde::{Deserialize, Serialize};

use crate::common::idgen::IdGen;
use crate::common::util;
use crate::common::webcommon::{handle_timeout_error, AppContext};
use crate::config::application::Application;

pub mod businessrouter;
pub mod chain_scan;
pub mod common;
pub mod config;
pub mod const_var;
pub mod db;
pub mod event_process;
pub mod file;
pub mod global_var;
pub mod macro_def;
pub mod request;
pub mod scheduler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize tracing
    util::set_log_conf();

    let application = Application::new().unwrap();
    let work_id: u32 = application.work_id.work_id;
    IdGen::new(work_id).unwrap();
    global_var::init_app_conf(application.clone());

    let db = db::create_db_ins().await?;
    db::set_db_ins(db);

    // event_process::start()?;
    // info!("event_process started");
    scheduler::start().await?;
    info!("scheduler started");

    // event_process::start_pull_history(&clickhouse_pool)?;
    // info!("event_process start pull history started");

    let app_state = AppContext::new(
        // clickhouse_pool.clone(),
        // redis_pool_ref.clone(),
        // chain_http_url_map,
        // ws_db,
    );

    // tokio::spawn(coinmarket::pull_data_from_coinmarket());

    // let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let app = Router::new()
        // .route(
        //     "/api/test/test",
        //     axum::routing::get(|| {
        //         info!("/api/test/test recv ---------------------");
        //         async {
        //             serde_json::to_string(&Res::with_data(json!({"a": 1u8, "b": 2u32}))).unwrap()
        //         }
        //         // async { "welcome to server " }
        //     }),
        // )
        .merge(crate::businessrouter::router::create_route().with_state(app_state))
        .nest_service("/assets", ServeDir::new("assets"))
        //  .layer(CorsLayer::permissive())
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_timeout_error))
                .timeout(Duration::from_secs(18)),
        )
        .layer(
            CorsLayer::new()
                .allow_methods(vec![
                    Method::GET,
                    Method::POST,
                    Method::OPTIONS,
                    Method::HEAD,
                    // Method::CONNECT,
                ])
                .allow_origin(Any)
                .allow_headers(Any),
        )
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([0, 0, 0, 0], application.server.port));
    info!("server started. listening on {:?}", addr);
    let _web_server = axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();

    Ok(())
}

#[cfg(test)]
mod tests {

    use crate::{chain_scan::solana::SolanaChain, common::util};

    use super::*;
    use base64::prelude::*;
    use borsh::{BorshDeserialize, BorshSerialize};
    use solana_sdk::pubkey::Pubkey;

    #[derive(Debug, BorshDeserialize, BorshSerialize)]
    pub struct TestEvent {
        pub index: u64,
        pub owner: Pubkey,
        pub receiver: Pubkey,
    }

    #[tokio::test]
    async fn test() -> Result<()> {
        let sol_chain = SolanaChain::new(
            String::from("http://localhost:8899"),
            vec![String::from("ESgBgi1fKTCLtR8cw2Dhm9Lu11c7EuRwhFQ76AcQk79Y")],
            vec![],
        )
        .await?;

        let block_n = 36501;
        // Some(24190)
        let rs = sol_chain.scanning_block_logs(block_n, Some(36635)).await?;

        println!("res, {:?}", rs);
        Ok(())
    }

    #[tokio::test]
    async fn test1() -> Result<()> {
        let s =  "HDQnaQjSWwk80x981fE9TXoRKkyrd02upvuqAlY7gE/vyZehNXaSOMZIdh4e4rUgMAoW3SOK/LOY1SJY1iVVZcMDBBvbt7xi";
        let d = BASE64_STANDARD.decode(s)?;
        println!("{}", util::is_event("TestEvent", &d[..]));

        let e: TestEvent = TestEvent::deserialize(&mut &d[8..])?;

        println!("evt: {:?}", e);

        Ok(())
    }
}
