use std::{arch::global_asm, time::Duration};

// use futures::executor::block_on;
use tokio::runtime::Runtime;
use tokio_cron_scheduler::{Job, JobScheduler};
use tower::make::future;
use tracing::{error, info};

use anyhow::Result;

use crate::{chain_scan, common::scheduler_util::fetch_task, db, global_var};
// use crate::common::types::*;
// use crate::logic::dexprocess::cronjob;

// pub async fn start(clickhouse_pool: &Pool, redis_pool: &RedisPool) -> anyhow::Result<()> {
//     let scheduler = JobScheduler::new().await.unwrap();
//     let redis_clone_three = redis_pool.clone();
//     // TODO: in prod need to change the interval
//     let redis_schder = Job::new_async("0 0 0/12 * * * *", move |_, _| {
//         let redis_clone_three = redis_clone_three.clone();
//         Box::pin(async move {
//             info!("Run get_all_cmc_prices_into_redis 120 seconds -------------------");
//             if let Err(e) = cronjob::get_all_cmc_token_info_into_redis(redis_clone_three).await {
//                 warn!("cron task cronjob execute error----->{e:?}");
//             }
//         })
//     })
//     .unwrap();

//     scheduler
//         .add(redis_schder)
//         .await
//         .expect("Should be able to add a job");

//     scheduler.start().await?;

//     Ok(())
// }

pub async fn start() -> Result<()> {
    tokio::spawn(async {
        loop {
            let fetch_task = tokio::spawn(async {
                loop {
                    {
                        // let mutex = global_var::get_fetch_mutex();
                        // let mut num = mutex.lock().await;
                        info!("start fetch");
                        fetch_task().await;
                        // *num = 1;
                        info!("end fetch");
                    }
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            });
            tokio::select! {
                _=fetch_task => {
                    info!("fetch task exist");
                }
            }

            info!("fetch task will start it later");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // let scheduler = JobScheduler::new().await?;
    // // secs run it every 50 .
    // let fetch_account_job = Job::new_repeated(Duration::from_secs(1), move |_, _| {
    //     info!("start fetch");
    //     block_on(future_fn());
    //     info!("end fetch")
    // })?;

    // scheduler
    //     .add(fetch_account_job)
    //     .await
    //     .expect("job should be added");

    // scheduler.start().await?;

    Ok(())
}
