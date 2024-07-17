#[macro_export]
macro_rules! get_mainnet_token_symbol {
    ($redis_pool:expr, $chain_id:expr, $token_address:expr) => {{
        util::get_mainnet_token_display_symbol($redis_pool, $chain_id, $token_address).await
    }};
}

#[macro_export]
macro_rules! jg {
    ($val:ident, $key:expr, $op:tt) => {{
        $val[$key].$op().ok_or(anyhow!(
            "err when getting key {}, op: {}",
            stringify!($key),
            stringify!($op)
        ))
    }};
}

#[macro_export]
macro_rules! Err {
    ($s: literal$(,)? $($v:ident),*) => {{
        Err(anyhow!($s, $($v),*))
    }}
}

#[macro_export]
macro_rules! rg {
    ($row:expr, $name:expr) => {{
        $row.get($name)
            .map_err(|e| anyhow!("when get row {}, err:{:?}", stringify!($name), e))
    }};
}

#[macro_export]
macro_rules! p_e {
    ($val:expr) => {{
        match $val {
            Err(err) => {
                println!("error: {:?}", err);
            }
            _ => {}
        }
    }};
}
