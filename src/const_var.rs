//  value has development | production
pub const ENV_MODE: &str = "development"; // TODO: in different environments we need to change this value.
                                          // pub const CHANNEL_SIZE: usize = 1024;
                                          // pub const WS_CHANNEL: usize = 512;
pub const ONE_BATCH_TIME: u64 = 800; // milliseconds
pub const TX_FETCH_STEP: u64 = 30; // the size step of one time query.
pub const DEFAULT_SIZE: usize = 30; // the default batch query of solana
                                    // important don't change this value if you don't know it.
pub const PROGRAM_DATA_STR: &str = "Program data:";

// local net
// pub const ACCOUNT_BOOK: &str = "13MBi6yjdRL87VeraMAaGooyXTHyZiEicXC844eqHBki";

// pub const ICON_SERVICE_DOMAIN_URL: &str = "http://localhost:3605";
// pub const PRICE_ORACLE_DOMAIN_URL: &str = "http://localhost:3600";
// pub const PRICE_ORACLE_WS: &str = "ws://localhost:3600/price_oracle/ws/swap_event";
// pub const REDIS_PREFIX: &str = "scroll_project"; // TODO: for each project we need to change it.
