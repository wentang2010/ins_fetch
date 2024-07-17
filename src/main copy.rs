use axum::routing::get_service;
use axum::Json;
use axum::{error_handling::HandleErrorLayer, Router};

use http::StatusCode;

use hyper::service;
use serde_json::json;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing_subscriber::{prelude::*, EnvFilter};

use std::net::SocketAddr;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceBuilder;
use tracing::{info, warn};

pub mod bindings;
pub mod businessrouter;
pub mod common;
pub mod config;
pub mod const_var;
pub mod db;
pub mod event_process;
pub mod global_var;
pub mod init;
pub mod logic;
pub mod scheduler;
pub mod system_var;

// pub mod cal_price;
pub mod cached;
mod macro_def;
use crate::common::types::*;

use crate::common::idgen::IdGen;
use crate::common::webcommon::{handle_timeout_error, AppContext, Res};
use crate::const_var::REDIS_PREFIX;
use axum::http::Method;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

use crate::common::{enums::UniEChain, util};

use crate::config::chains_config::ChainConfigs;

#[tokio::main]
async fn maian() -> anyhow::Result<()> {
    // initialize tracing

    std::env::set_var("RUST_LOG", "scroll_project=debug,hyper=info");
    // if std::env::var_os("RUST_LOG").is_none() {
    //     std::env::set_var("RUST_LOG", "market_system=info,hyper=info")
    //     // std::env::set_var("RUST_LOG", "debug")
    // }

    println!("run project {}", REDIS_PREFIX);

    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        println!("Usage: command init, command start, command load_config");
        return Ok(());
    }

    if &args[1] == "load_config" {
        return Ok(());
    }

    if &args[1] == "init" {
        return init::init_main().await;
    }

    if &args[1] != "start" {
        println!("Usage: command init, command start");
        return Ok(());
    }

    // system_var::init_var(&args[2]);

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_file(true)
        .with_line_number(true)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let application = Application::new().unwrap();
    println!(
        "will connect to db: {}, url:{}",
        application.clickhouse.db_name, application.clickhouse.clickhouse_database_url
    );

    let clickhouse_pool = util::get_clickhouse_pool(&application)?;

    global_var::init_app_conf(&application);
    // used send msg to websocket
    let ws_db = Arc::new(RwLock::new(HashMap::new()));

    let work_id: u32 = application.work_id.work_id;
    IdGen::new(work_id).unwrap();
    info!("the idgen is--------> {}", IdGen::next_id());

    let redis_pool = util::create_redis_pool(&application).await?;
    global_var::init_redis_pool(redis_pool);
    let redis_pool_ref = global_var::get_redis_pool();

    util::load_some_data(&clickhouse_pool).await?;
    scheduler::start(&clickhouse_pool, redis_pool_ref).await?;

    let chain_http_url_map = util::get_chain_http_url_map()?;
    event_process::start(
        &clickhouse_pool,
        redis_pool_ref,
        // Arc::clone(&ws_db),
    )?;

    event_process::start_leverage_ws(&clickhouse_pool)?;
    info!("event_process started");

    // event_process::start_pull_history(&clickhouse_pool)?;
    // info!("event_process start pull history started");

    let app_state = AppContext::new(
        clickhouse_pool.clone(),
        redis_pool_ref.clone(),
        chain_http_url_map,
        ws_db,
    );

    // tokio::spawn(coinmarket::pull_data_from_coinmarket());

    // let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let app = Router::new()
        .route("/", axum::routing::get(|| async { "Trade Market API " }))
        .route(
            "/api/test/test",
            axum::routing::get(|| {
                info!("/api/test/test recv ---------------------");
                async {
                    serde_json::to_string(&Res::with_data(json!({"a": 1u8, "b": 2u32}))).unwrap()
                }
                // async { "welcome to server " }
            }),
        )
        .merge(crate::businessrouter::router::create_route().with_state(app_state))
        .nest_service("/assets", ServeDir::new("assets"))
        //  .layer(CorsLayer::permissive())
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_timeout_error))
                .timeout(Duration::from_secs(14)),
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
    use super::*;
    use crate::logic::clickhouseimp;
    use crate::logic::weblogic::tokens;
    use anyhow::Result;
    use clickhouse_rs::{row, types::Block, Pool};
    use ethers::{abi::AbiDecode, prelude::*, types::H256};
    use fred::{pool::RedisPool, prelude::*};
    const rpc_url: &str = "https://rpc-wallet.broearn.com/eth";

    #[tokio::main]
    #[test]
    async fn test_pool() -> Result<()> {
        let pool_meta = util::load_pool_meta("0xac63436b092b944cadea9243f9aff315421d4fee", rpc_url)
            .await?
            .unwrap();
        println!("pool meta :{pool_meta:?}");
        Ok(())
    }

    #[test]
    fn test_hex() {
        let bytes =  hex::decode("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000283542e4bf643250000000000000000000000000000000000000005cc3990df72738cdd0445ed340000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let amount0In = U256::from_big_endian(&bytes[0..32]);
        let amount1In = U256::from_big_endian(&bytes[32..64]);
        let amount0Out = U256::from_big_endian(&bytes[64..96]);
        let amount1Out = U256::from_big_endian(&bytes[96..128]);

        println!("amount0In: {amount0In}, amount1In:{amount1In}, amount0Out:{amount0Out}, amount1Out:{amount1Out}");
    }

    #[tokio::main]
    #[test]
    async fn get_parsed_log() -> Result<()> {
        let application = Application::new().unwrap();
        let chains = ChainConfigs::get_chains();
        // let provider = Provider::<Http>::try_from(rpc_url)?;
        let provider = util::create_provider(&chains[0]).unwrap();
        let db_pool = util::get_clickhouse_pool(&application).unwrap();
        let block_number = provider.get_block_number().await?;
        // let block_number = 17726374;
        println!("block_num:{block_number}");
        let chain_id = provider.get_chainid().await?;
        let block_opt = provider.get_block_with_txs(block_number).await?;

        let filter = Filter::new()
            // .address(addr_arr)
            // .event("Swap(address,uint256,uint256,uint256,uint256,address)") // v2
            .event("Swap(address,address,int256,int256,uint160,uint128,int24)")
            .from_block(block_number - 20);
        let mut stream = provider.get_logs_paginated(&filter, 90).take(5);
        while let Some(res) = stream.next().await {
            let log = res?;
            let event_ctc = ContractEvent(UniEChain::EthGoerli, EventType::UniSwapV3);
            let parsed_log = event_ctc
                .parse_log(&log, &provider, &db_pool)
                .await
                .unwrap();

            println!("Log: {parsed_log:?}\n\n");
        }

        Ok(())
    }

    #[tokio::main]
    #[test]
    async fn test_get_block() -> Result<()> {
        let application = Application::new().unwrap();
        let provider = Provider::<Http>::try_from(rpc_url)?;
        // let provider = util::create_provider(application).unwrap();
        let db_pool = util::get_clickhouse_pool(&application).unwrap();
        // let block_number = provider.get_block_number().await?;
        let block_number = 17726374;
        println!("block_num:{block_number}");
        let chain_id = provider.get_chainid().await?;
        println!("chain_id:{chain_id}");
        let block_opt = provider.get_block_with_txs(block_number).await?;
        let block = block_opt.unwrap();
        println!("block hash: {:02x}", block.hash.unwrap());

        for t in block.transactions {
            let nonce = t.nonce;
            let block_hash = t.block_hash;
            let block_number = t.block_number;
            let transaction_index = t.transaction_index;
            let from = t.from;
            let to = t.to;
            let value = t.value;
            // let is_system_tx = t.is_system_tx;

            let tx_hash =
                "0x7fe80a618665b89ca2409b1e61752d007010cb2e50b67ea3fdeac5904d4f6835".to_string();
            let tx_hash =
                "0x37b90c1ae1ca4bee4860d256e01dc17a1f713000b1fa5a3e0ac645027db6eb6d".to_string();

            if format!("0x{:0x}", t.hash) != tx_hash {
                continue;
            }

            // println!(
            //     "nonce: {nonce}, tx_hash:{:0x},
            // transaction_index:{transaction_index:?},
            // from:{from:0x}, to:{:0x}, value: {value} ",
            //     t.hash,
            //     to.unwrap()
            // );
        }
        // println!("last_block: {last_block}");

        // return Ok(());

        const DAI_ADDRESS: &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";
        const USDC_ADDRESS: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
        const USDT_ADDRESS: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";

        let token_topics = vec![
            H256::from(USDC_ADDRESS.parse::<H160>()?),
            H256::from(USDT_ADDRESS.parse::<H160>()?),
            H256::from(DAI_ADDRESS.parse::<H160>()?),
        ];

        // let filter = Filter::new()
        //     .address("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse::<Address>()?) // v2 factory
        //     // .address("0x1F98431c8aD98523631AE4a59f267346ea31F984".parse::<Address>()?) // v3
        //     // .event("PoolCreated(address,address,uint24,int24,address)")
        //     .event("Swap(address,uint,uint,uint,uint,address)")
        //     // .topic1(token_topics.to_vec())
        //     // .topic2(token_topics.to_vec())
        //     .from_block(0);

        // let erc20_transfer_filter = Filter::new()
        //     .from_block(block_number)
        //     .event("Transfer(address,address,uint256)");

        let block_number = 17712073;
        let addr_arr = vec![
            // "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse::<Address>()?,
            // "0xf164fC0Ec4E93095b804a4795bBe1e041497b92a".parse::<Address>()?,
            // "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse::<Address>()?,
            "0xd6a347c998109ac87ccb323f78960c91bbe911cd".parse::<Address>()?,
        ];
        let filter = Filter::new()
            // .address(addr_arr)
            // .event("Swap(address,uint256,uint256,uint256,uint256,address)") // v2
            .event("Swap(address,address,int256,int256,uint160,uint128,int24)")
            .from_block(block_number);
        let mut stream = provider.get_logs_paginated(&filter, 90).take(5);
        while let Some(res) = stream.next().await {
            let log = res?;
            // let event_ctc = ContractEvent(UniEChain::Eth, EventType::UniSwapV3);
            // let parsed_log = event_ctc.parse_log(&log, &provider, &db_pool).unwrap();
            // if "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822"
            //     .parse::<H256>()?
            //     != log.topics[0]
            // {
            //     continue;
            // }
            // swap event top0 0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822
            println!("Log: {log:?}\n\n");

            // let from = Address::from(log.topics[1]);
            // let to = Address::from(log.topics[2]);
            // let amount0In = U256::from_big_endian(&log.data[0..32]);
            // let amount1In = U256::from_big_endian(&log.data[32..64]);
            // let amount0Out = U256::from_big_endian(&log.data[64..96]);
            // let amount1Out = U256::from_big_endian(&log.data[96..128]);

            // println!("from: {from} to: {to}, amount0In: {amount0In}, amount1In:{amount1In}, amount0Out:{amount0Out}, amount1Out:{amount1Out}");
        }

        // while let Some(res) = stream.next().await {
        //     let log = res?;
        //     println!(
        //         "block: {:?}, tx: {:?}, token: {:?}, from: {:?}, to: {:?}, amount: {:?}",
        //         log.block_number,
        //         log.transaction_hash,
        //         log.address,
        //         log.topics.get(1),
        //         log.topics.get(2),
        //         U256::decode(log.data)
        //     );
        // }

        // while let Some(res) = stream.next().await {
        //     let log = res?;
        //     let token0 = Address::from(log.topics[1]);
        //     let token1 = Address::from(log.topics[2]);
        //     let fee_tier = U256::from_big_endian(&log.topics[3].as_bytes()[29..32]);
        //     let tick_spacing = U256::from_big_endian(&log.data[29..32]);
        //     let pool = Address::from(&log.data[44..64].try_into()?);
        //     println!(
        //         "pool = {pool}, token0 = {token0}, token1 = {token1}, fee = {fee_tier}, spacing = {tick_spacing}"
        //     );
        // }

        Ok(())
    }

    #[tokio::test]
    async fn init_database() {
        let application = Application::new().unwrap();
        let clickhouse_url = application.clickhouse.clickhouse_database_url.as_str();
        let pool = Pool::new(clickhouse_url);

        let result = clickhouseimp::setup_init(&pool).await;
        // assert_eq!(result.is_ok(), true);
        if let Err(e) = result {
            panic!("err: {:?}", e);
        } else {
            println!("result: {:?}", result);
        }
    }

    #[tokio::test]
    async fn init_tokens() {
        let application = Application::new().unwrap();
        let clickhouse_url = application.clickhouse.clickhouse_database_url.as_str();
        let pool = Pool::new(clickhouse_url);

        let result = tokens::tokenicon_init(&pool).await;
        if let Err(e) = result {
            panic!("err: {:?}", e);
        } else {
            println!("result: {:?}", result);
        }
    }

    #[tokio::test]
    async fn test() {
        let application = Application::new().unwrap();
        let redis_url = application.redis.redis_connection_url.as_str();

        println!("the redis_url  is ------> {}", redis_url);
    }

    #[tokio::test]
    async fn test_redis() -> Result<()> {
        let application = Application::new().unwrap();

        let redis_url = application.redis.redis_connection_url.as_str();

        let redis_config = RedisConfig::from_url(redis_url).unwrap();

        // let config = RedisConfig::default();
        let redis_pool = RedisPool::new(redis_config, None, None, 5).unwrap();

        let _ = redis_pool.connect();
        let _ = redis_pool.wait_for_connect().await?;

        for client in redis_pool.clients() {
            println!(
                "{} connected to {:?}",
                client.id(),
                client.active_connections().await?
            );
        }

        if let Err(e) = redis_pool
            .set::<(), _, _>("test", "hello world", None, None, false)
            .await
        {
            panic!("{:?}", e);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_http() -> Result<()> {
        let application = Application::new().unwrap();
        let clickhouse_url = application.clickhouse.clickhouse_database_url.as_str();
        let pool = Pool::new(clickhouse_url);
        let redis_url = application.redis.redis_connection_url.as_str();

        let redis_config = RedisConfig::from_url(redis_url).unwrap();

        // let config = RedisConfig::default();
        let redis_pool = RedisPool::new(redis_config, None, None, 5).unwrap();

        let _ = redis_pool.connect();
        let _ = redis_pool.wait_for_connect().await?;

        for client in redis_pool.clients() {
            println!(
                "{} connected to {:?}",
                client.id(),
                client.active_connections().await?
            );
        }

        match tokens::api_tokens_search(
            pool.clone(),
            redis_pool.clone(),
            "eth".to_string(),
            "desc".to_string(),
            // sort_filed.to_string(),
            0,
            50,
        )
        .await
        {
            Ok(data) => println!("{:?}", data),
            Err(e) => {
                eprintln!("the api_tokens_search query error---->{e:?}");
            }
        }

        Ok(())
    }

    #[test]
    fn test_print() -> Result<()> {
        let f = json!({
            "ETH":{"a":231555084232.08655123456789}
        });

        let f1 = "16.768059947282552123456";
        // println!("f1: {:?}", f1.parse::<f64>().unwrap());
        let addr = "0xd6a347c998109ac87ccb323f78960c91bbe911cd".parse::<Address>()?;
        println!("addr: {:x}", addr);
        println!("addr: {:#x}", addr);
        // let s = "ETH".to_string();
        // println!(
        //     "a :{ }, b: {:?}",
        //     if false { "xxx" } else { "ttt" },
        //     f[&s].as_object().unwrap()
        // );

        // let a = crate::jg!(f, "ETH1", as_object).unwrap();
        // println!("val :{:?}", a);

        Ok(())
    }
}
