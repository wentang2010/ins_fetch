use crate::common::types::ChainConfigs;
use crate::logic::weblogic::tokens;
use crate::{config::application::Application, logic::clickhouseimp};
use anyhow::Result;
use clickhouse_rs::{row, types::Block, Pool};

pub async fn init_main() -> Result<()> {
    let application = Application::new().unwrap();
    // let chain_configs = ChainConfigs::new().unwrap();
    let clickhouse_url = application.clickhouse.clickhouse_database_url.as_str();
    println!("the clickhouse url is ------> {}", clickhouse_url);
    println!("db is {}", application.clickhouse.db_name);
    let clickhouse_pool = Pool::new(clickhouse_url);
    init_db(&clickhouse_pool).await?;
    init_db_data(&clickhouse_pool).await?;
    Ok(())
}

pub async fn init_db(pool: &Pool) -> Result<()> {
    let result = clickhouseimp::setup_init(pool).await;
    // assert_eq!(result.is_ok(), true);
    if let Err(e) = result {
        panic!("clickhouse setup err: {:?}", e);
    } else {
        println!("clickhouse setup result: {:?}", result);
    }

    // let result = tokens::tokenicon_init(pool).await;
    // if let Err(e) = result {
    //     panic!("tokenicon init err: {:?}", e);
    // } else {
    //     println!("tokenicon init result: {:?}", result);
    // }

    Ok(())
}

pub async fn init_db_data(pool: &Pool) -> Result<()> {
    let result = clickhouseimp::insert_data(pool).await;
    // assert_eq!(result.is_ok(), true);
    if let Err(e) = result {
        panic!("clickhouse init_db_data err: {:?}", e);
    } else {
        println!("clickhouse init_db_data result: {:?}", result);
    }

    Ok(())
}
