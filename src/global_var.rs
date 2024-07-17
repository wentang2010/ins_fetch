use std::sync::Once;

use fred::pool::RedisPool;
use futures_util::Future;
use once_cell::sync::OnceCell;
use tokio::sync::Mutex;

use crate::config::application::Application;
static mut application: Option<Application> = None;
static FETCH_MUTEX: OnceCell<Mutex<u8>> = OnceCell::new();
static INIT: Once = Once::new();
static INIT1: Once = Once::new();
static REDIS_POOL: OnceCell<RedisPool> = OnceCell::new();
// static DB_DATA: OnceCell<DB> = OnceCell::new();

// static mut CHAIN_EVT_CB: Vec<Box<dyn fn() -> impl Future<Output = ()>> = Vec::new();

pub fn init_app_conf(conf: Application) {
    INIT.call_once(|| unsafe {
        application = Some(conf);
    });
}

// pub fn set_db(db: DB) {
//     DB_DATA.set(db).unwrap();
// }

pub fn get_fetch_mutex() -> &'static Mutex<u8> {
    FETCH_MUTEX.get_or_init(|| Mutex::new(0))
}

pub fn init_redis_pool(value: RedisPool) {
    REDIS_POOL.set(value).unwrap();
}

pub fn get_redis_pool() -> &'static RedisPool {
    REDIS_POOL.get().expect("please init it")
}

pub fn get_app_conf() -> &'static Application {
    unsafe { application.as_ref().unwrap() }
}

// pub fn get_db() -> &'static DB {
//     DB_DATA.get().expect("please init it")
// }
