use std::time::Duration;

use tracing::error;

use crate::{chain_scan, db, global_var};

pub async fn fetch_task() {
    // tokio::time::sleep(Duration::from_secs(5)).await;
    // info!("run fetch account job");
    let scan_ins = chain_scan::solana::get_ins().await.unwrap();
    let app = global_var::get_app_conf();
    let fetch_rs = scan_ins
        .client
        .get_program_accounts(app.account.inscription_size)
        .await;
    if fetch_rs.is_err() {
        error!("fetch inscription failed, err: {:?}", fetch_rs.unwrap_err());
    } else {
        let db_ins = db::get_db_ins();
        if let Err(e) = db_ins
            .insert_if_not_exist_inscription(fetch_rs.unwrap().list)
            .await
        {
            error!("insert into inscription failed, err: {:?}", e);
        }
    }

    tokio::time::sleep(Duration::from_secs(3)).await;

    let fetch_rs = scan_ins
        .client
        .get_program_accounts(app.account.replica_ins_size)
        .await;
    if fetch_rs.is_err() {
        error!("fetch replica ins failed, err: {:?}", fetch_rs.unwrap_err());
    } else {
        let db_ins = db::get_db_ins();
        // let pre_count = db_ins.get_replica_ins_count().await.unwrap();
        if let Err(e) = db_ins
            .insert_if_not_exist_replica_ins(fetch_rs.unwrap().list)
            .await
        {
            error!("insert into replica ins failed, err: {:?}", e);
        }

        // let after_count = db_ins.get_replica_ins_count().await.unwrap();
        // if after_count != pre_count {
        //     // println!("sta");
        //     println!("update all used num");
        //     if let Err(e) = db_ins.update_all_ins_used_num(true).await {
        //         error!("error in update_all_ins_used_num, {:?}", e);
        //     }
        // }
    }
}
