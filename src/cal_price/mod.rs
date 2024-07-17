use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use ethers::{types::U256, utils::format_units};
use fred::pool::RedisPool;
use futures_util::future::{BoxFuture, FutureExt};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::{
    common::{
        structs::SwapPairEventInfo,
        types::{LogParsedData, UniEChain},
        util,
    },
    config::chains_token_config::{CTokenConfig, ChainTokenConfigs},
    const_var::{CMC_USDC_ADDR, CMC_USDT_ADDR, REDIS_PREFIX},
    file::add_to_log,
};
use anyhow::{anyhow, Ok, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairInfo {
    // pub pair_type: u8, // 0 base pair, 1: calculated pair
    pub token_0: String,
    pub token_1: String,
    pub symbol_0: String,
    pub symbol_1: String,
    pub deciaml_0: u8,
    pub deciaml_1: u8,
    pub amount_0: String,
    pub amount_1: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenUSDTInfo {
    pub token: String,
    pub symbol: String,
    pub decimal: u8,
    pub price: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenUSDInfo {
    pub token: String,
    pub symbol: String,
    pub decimal: u8,
    pub price: f64,
}

pub fn format_amount_to_f64(amount: &str, decimal: u8) -> f64 {
    let r0 = U256::from_dec_str(amount).unwrap();

    let r0_f64: f64 = format_units(r0, decimal as u32).unwrap().parse().unwrap();
    r0_f64
}

static PRICE_INST: OnceCell<PriceStore> = OnceCell::new();
static SIM_PRICE_INST: OnceCell<PriceStore> = OnceCell::new();

pub struct PriceStore {
    type_s: String,
}

pub fn static_inst() -> &'static PriceStore {
    PRICE_INST.get_or_init(|| PriceStore::new(String::new()))
}

pub fn simulate() -> &'static PriceStore {
    SIM_PRICE_INST.get_or_init(|| PriceStore::new("simulate".to_string()))
}

impl PriceStore {
    fn new(type_s: String) -> PriceStore {
        PriceStore { type_s: type_s }
    }

    pub fn gen_pair_key(
        &self,
        chain_id: UniEChain,
        token_symbol_0: &str, // this can be symbol or token
        token_symbol_1: &str, // this can be symbol or token
        key_type: u8,         //  0 base pair, 1: calculated pair, 2: smooth usd pair
    ) -> String {
        self.gen_pair_key_inner(
            chain_id,
            token_symbol_0,
            token_symbol_1,
            key_type,
            self.type_s.as_str(),
        )
    }

    pub fn gen_pair_key_inner(
        &self,
        chain_id: UniEChain,
        token_symbol_0: &str, // this can be symbol or token
        token_symbol_1: &str, // this can be symbol or token
        key_type: u8,
        prefix_str: &str, //  0 base pair, 1: calculated pair, 2: smooth usd pair
    ) -> String {
        if key_type == 0 {
            return format!(
                "{REDIS_PREFIX}:{prefix_str}:pair_info:{}:{}:{}",
                chain_id.to_string().to_uppercase(),
                token_symbol_0.to_uppercase(),
                token_symbol_1.to_uppercase()
            );
        } else if key_type == 1 {
            return format!(
                "{REDIS_PREFIX}:{prefix_str}:pair_info_cal:{}:{}:{}",
                chain_id.to_string().to_uppercase(),
                token_symbol_0.to_uppercase(),
                token_symbol_1.to_uppercase()
            );
        } else if key_type == 2 {
            return format!(
                "{REDIS_PREFIX}:{prefix_str}:smooth_price:{}:{}:{}",
                chain_id.to_string().to_uppercase(),
                token_symbol_0.to_uppercase(),
                token_symbol_1.to_uppercase()
            );
        }

        panic!("not support key type:{key_type}");
    }

    // save into pair pool
    pub async fn save(
        &self,
        chain_id: UniEChain,
        pair: PairInfo,
        redis_pool: &RedisPool,
        // key_type: u8, //  0 base pair, 1: calculated pair
    ) -> Result<()> {
        let amount0_f64 = pair.amount_0.parse::<f64>().unwrap();
        let amount1_f64 = pair.amount_1.parse::<f64>().unwrap();
        if amount0_f64 == 0.0 || amount1_f64 == 0.0 {
            return Err(anyhow!(
                "amount should not be zero in pair. pair: {:?}",
                pair
            ));
        }

        let token_symbol_0: &str = pair.symbol_0.as_str();
        let token_symbol_1: &str = pair.symbol_1.as_str();

        // add_to_log(format!(
        //     "chain_id:{:?} symbol_0/symbol_1: {:?}/{:?}",
        //     chain_id, token_symbol_0, token_symbol_1
        // ))
        // .unwrap();

        let key = self.gen_pair_key(chain_id, token_symbol_0, token_symbol_1, 0);
        util::save_into_redis::<PairInfo>(key, pair, redis_pool).await?;
        Ok(())
    }

    // get from pair pool
    pub async fn get(
        &self,
        chain_id: UniEChain,
        token_symbol_0: &str,
        token_symbol_1: &str,
        redis_pool: &RedisPool,
        // key_type: u8, // 0 base pair, 1: calculated pair
    ) -> Result<Option<PairInfo>> {
        let key = self.gen_pair_key(chain_id, token_symbol_0, token_symbol_1, 0);
        let d = util::get_from_redis::<PairInfo>(key, redis_pool).await?;
        Ok(d)
    }

    // save into pair pool
    pub async fn save_v1(
        &self,
        chain_id: UniEChain,
        pair: PairInfo,
        redis_pool: &RedisPool,
        // key_type: u8, //  0 base pair, 1: calculated pair
    ) -> Result<()> {
        // pair.amount_0 is the smallest unit.
        let amount0_f64 = pair.amount_0.parse::<f64>()?;
        let amount1_f64 = pair.amount_1.parse::<f64>()?;
        if amount0_f64 == 0.0 || amount1_f64 == 0.0 {
            return Err(anyhow!(
                "amount should not be zero in pair. pair: {:?}",
                pair
            ));
        }

        let token_0: &str = pair.token_0.as_str();
        let token_1: &str = pair.token_1.as_str();

        // add_to_log(format!(
        //     "chain_id:{:?} symbol_0/symbol_1: {:?}/{:?}",
        //     chain_id, token_symbol_0, token_symbol_1
        // ))
        // .unwrap();

        let key = self.gen_pair_key(chain_id, token_0, token_1, 0);
        util::save_into_redis::<PairInfo>(key, pair, redis_pool).await?;
        Ok(())
    }

    pub async fn get_v1(
        &self,
        chain_id: UniEChain,
        token_0: &str,
        token_1: &str,
        redis_pool: &RedisPool,
        // key_type: u8, // 0 base pair, 1: calculated pair
    ) -> Result<Option<PairInfo>> {
        let key = self.gen_pair_key(chain_id, token_0, token_1, 0);
        let d = util::get_from_redis::<PairInfo>(key, redis_pool).await?;
        Ok(d)
    }

    pub async fn save_cal_info(
        &self,
        chain_id: UniEChain,
        info: TokenUSDTInfo,
        redis_pool: &RedisPool,
        // key_type: u8, //  0 base pair, 1: calculated pair
    ) -> Result<()> {
        let key = self.gen_pair_key(chain_id, &info.symbol, "USDT", 1);
        let token_addr_key = self.gen_pair_key(chain_id, &info.token, "USDT", 1);
        util::save_into_redis::<TokenUSDTInfo>(key, info.clone(), redis_pool).await?;
        util::save_into_redis::<TokenUSDTInfo>(token_addr_key, info, redis_pool).await?;
        Ok(())
    }

    pub async fn get_cal_info(
        &self,
        chain_id: UniEChain,
        token_symbol: &str,
        redis_pool: &RedisPool,
        // key_type: u8, //  0 base pair, 1: calculated pair
    ) -> Result<Option<TokenUSDTInfo>> {
        let key = self.gen_pair_key(chain_id, token_symbol, "USDT", 1);
        let d = util::get_from_redis::<TokenUSDTInfo>(key, redis_pool).await?;
        Ok(d)
    }

    pub async fn save_usd_smooth_info(
        &self,
        chain_id: UniEChain,
        info: TokenUSDInfo,
        redis_pool: &RedisPool,
        // key_type: u8, //  0 base pair, 1: calculated pair
    ) -> Result<()> {
        let symbol_key = self.gen_pair_key(chain_id, &info.symbol, "USD", 2);
        let addr_key = self.gen_pair_key(chain_id, &info.token, "USD", 2);
        let prev_obj = self
            .get_usd_smooth_info_v1(chain_id, info.token.as_str(), redis_pool)
            .await?;
        let mut will_save_obj = info.clone();
        if let Some(prev_obj) = prev_obj {
            // smooth line price.
            will_save_obj.price = (prev_obj.price * 2.0 + info.price) / 3.0;
        }

        // add_to_log(format!(
        //     "add smooth price. symbol: {symbol_key}, addr:{addr_key}, price: {}, decimal: {}",
        //     will_save_obj.price, info.decimal
        // ))?;

        util::save_into_redis::<TokenUSDInfo>(symbol_key, will_save_obj.clone(), redis_pool)
            .await?;
        util::save_into_redis::<TokenUSDInfo>(addr_key, will_save_obj, redis_pool).await?;

        Ok(())
    }

    pub async fn get_usd_smooth_info(
        &self,
        chain_id: UniEChain,
        token_symbol: &str,
        redis_pool: &RedisPool,
        // key_type: u8, //  0 base pair, 1: calculated pair
    ) -> Result<Option<TokenUSDInfo>> {
        let key = self.gen_pair_key(chain_id, token_symbol, "USD", 2);
        let d = util::get_from_redis::<TokenUSDInfo>(key, redis_pool).await?;
        Ok(d)
    }

    pub async fn get_usd_smooth_info_v1(
        &self,
        chain_id: UniEChain,
        token_address: &str,
        redis_pool: &RedisPool,
        // key_type: u8, //  0 base pair, 1: calculated pair
    ) -> Result<Option<TokenUSDInfo>> {
        let key = self.gen_pair_key(chain_id, token_address, "USD", 2);
        let d = util::get_from_redis::<TokenUSDInfo>(key, redis_pool).await?;
        Ok(d)
    }

    pub async fn save_token_price_main(
        &self,
        chain_id: UniEChain,
        usdt_price: Vec<TokenUSDTInfo>,
        usd_price: Vec<TokenUSDInfo>,
        redis_pool: &RedisPool,
    ) -> Result<()> {
        for v in usdt_price {
            self.save_cal_info(chain_id, v, redis_pool).await?;
        }

        for v in usd_price {
            self.save_usd_smooth_info(chain_id, v, redis_pool).await?;
        }

        Ok(())
    }

    pub async fn get_without_dire(
        &self,
        chain_id: UniEChain,
        token_symbol_a: &str,
        token_symbol_b: &str,
        redis_pool: &RedisPool,
        // key_type: u8,
    ) -> Result<Option<(PairInfo, f64, f64, u8)>> {
        let obj = self
            .get(
                chain_id,
                token_symbol_a,
                token_symbol_b,
                redis_pool,
                // key_type,
            )
            .await?;
        if obj.is_some() {
            let obj = obj.unwrap();
            let amount_0 = format_amount_to_f64(obj.amount_0.as_str(), obj.deciaml_0);
            let amount_1 = format_amount_to_f64(obj.amount_1.as_str(), obj.deciaml_1);
            // let amount_0 = obj.amount_0.clone();
            // let amount_1 = obj.amount_1.clone();
            return Ok(Some((obj, amount_0, amount_1, 0u8)));
        }

        let obj = self
            .get(
                chain_id,
                token_symbol_b,
                token_symbol_a,
                redis_pool,
                // key_type,
            )
            .await?;
        if obj.is_some() {
            let obj = obj.unwrap();
            let amount_0 = format_amount_to_f64(obj.amount_0.as_str(), obj.deciaml_0);
            let amount_1 = format_amount_to_f64(obj.amount_1.as_str(), obj.deciaml_1);
            // let amount_0 = obj.amount_0.clone();
            // let amount_1 = obj.amount_1.clone();
            return Ok(Some((obj, amount_1, amount_0, 1u8)));
        }

        Ok(None)
    }

    pub async fn get_without_dire_v1(
        &self,
        chain_id: UniEChain,
        token_a: &str,
        token_b: &str,
        redis_pool: &RedisPool,
        // key_type: u8,
    ) -> Result<Option<(PairInfo, f64, f64, u8)>> {
        let obj = self
            .get_v1(
                chain_id, token_a, token_b, redis_pool,
                // key_type,
            )
            .await?;
        if obj.is_some() {
            let obj = obj.unwrap();
            let amount_0 = format_amount_to_f64(obj.amount_0.as_str(), obj.deciaml_0);
            let amount_1 = format_amount_to_f64(obj.amount_1.as_str(), obj.deciaml_1);
            // let amount_0 = obj.amount_0.clone();
            // let amount_1 = obj.amount_1.clone();
            return Ok(Some((obj, amount_0, amount_1, 0u8)));
        }

        let obj = self
            .get_v1(
                chain_id, token_b, token_a, redis_pool,
                // key_type,
            )
            .await?;
        if obj.is_some() {
            let obj = obj.unwrap();
            let amount_0 = format_amount_to_f64(obj.amount_0.as_str(), obj.deciaml_0);
            let amount_1 = format_amount_to_f64(obj.amount_1.as_str(), obj.deciaml_1);
            // let amount_0 = obj.amount_0.clone();
            // let amount_1 = obj.amount_1.clone();
            return Ok(Some((obj, amount_1, amount_0, 1u8)));
        }

        Ok(None)
    }

    pub async fn infer_stable_token_price(
        &self,
        chain_id: UniEChain,
        token_addr: String,
        redis_pool: RedisPool,
    ) -> Result<Option<(f64, String, String)>> {
        // @Return price, price_base_symbol, price_base_token.
        let chain_token_cfgs = ChainTokenConfigs::get_cache_map(&redis_pool).await?;
        let chain_cfg = chain_token_cfgs.get(chain_id.to_string().as_str()).unwrap();

        let base_token1 = chain_cfg.usdc.clone(); // usdc price
        if !base_token1.is_empty() {
            let infer_p = self
                .infer_base_price(
                    chain_id,
                    chain_cfg.clone(),
                    token_addr.clone(),
                    redis_pool.clone(),
                    base_token1.clone(),
                    None,
                    None,
                )
                .await?;
            if let Some(price) = infer_p {
                return Ok(Some((price, "USDC".to_string(), base_token1)));
            }
        }

        let base_token0 = chain_cfg.usdt.clone(); // usdt
        if !base_token0.is_empty() {
            let infer_p = self
                .infer_base_price(
                    chain_id,
                    chain_cfg.clone(),
                    token_addr.clone(),
                    redis_pool.clone(),
                    base_token0.clone(),
                    None,
                    None,
                )
                .await?;
            if let Some(price) = infer_p {
                return Ok(Some((price, "USDT".to_string(), base_token0)));
            }
        }

        Ok(None)
    }

    pub fn infer_base_price<'a>(
        &'a self,
        chain_id: UniEChain,
        chain_token_cfg: CTokenConfig,
        token_addr: String,
        redis_pool: RedisPool,
        base_token: String,
        level: Option<u8>, //important! to avoid infinite recursion, each level only can run once, from high to low level.
        sel_set: Option<HashSet<String>>,
    ) -> BoxFuture<'a, Result<Option<f64>>> {
        // token 0 usdt price, token 1 usdt price
        async move {
            let mut sel_symbol_set = HashSet::<String>::new();
            if sel_set.is_some() {
                sel_symbol_set = sel_set.unwrap();
            }

            if sel_symbol_set.len() > 10 {
                // too long path.
                return Ok(None);
            }

            let token_addr = token_addr.to_lowercase();
            let base_token = base_token.to_lowercase();
            if token_addr.eq(base_token.as_str()) {
                return Ok(Some(1.0));
            }

            let base_pair_obj = self
                .get_without_dire_v1(
                    chain_id,
                    token_addr.as_str(),
                    base_token.as_str(),
                    &redis_pool,
                )
                .await?;

            if let Some((base_pair, amount_a_f64, amount_b_f64, dire)) = base_pair_obj {
                return Ok(Some(amount_b_f64 / amount_a_f64)); // price is inverse ratio of amount. price0/price1 = amount1/amount2
            }

            if let Some(level) = level {
                if level < 2 {
                    // can't run level2. just return none.
                    return Ok(None);
                }
            }

            let level2_symbol = chain_token_cfg.cal_price_level2.clone();

            for s in level2_symbol.iter() {
                if sel_symbol_set.contains(&s.to_string()) {
                    continue;
                }
                sel_symbol_set.insert(s.to_string());

                let pair_obj = self
                    .get_without_dire_v1(chain_id, token_addr.as_str(), s, &redis_pool)
                    .await?;
                if let Some((pair, amount_a_f64, amount_b_f64, dire)) = pair_obj {
                    let next_pair_price_opt = self
                        .infer_base_price(
                            chain_id,
                            chain_token_cfg.clone(),
                            s.to_string(),
                            redis_pool.clone(),
                            base_token.clone(),
                            Some(2),
                            Some(sel_symbol_set.clone()),
                        )
                        .await?;
                    if let Some(next_pair_price) = next_pair_price_opt {
                        return Ok(Some(next_pair_price * amount_b_f64 / amount_a_f64));
                    }
                }
                sel_symbol_set.remove(&s.to_string());
            }

            // found no relationsheep for symbol -> level1_symbol - > USDT

            if let Some(level) = level {
                if level < 3 {
                    // can't run level3. just return none.
                    return Ok(None);
                }
            }

            let level3_symbol = chain_token_cfg.cal_price_level3.clone(); // ,"WADA","WMATIC",

            for s in level3_symbol.iter() {
                if sel_symbol_set.contains(&s.to_string()) {
                    continue;
                }
                sel_symbol_set.insert(s.to_string());

                let pair_obj = self
                    .get_without_dire_v1(chain_id, token_addr.as_str(), s, &redis_pool)
                    .await?;
                if let Some((pair, amount_a_f64, amount_b_f64, dire)) = pair_obj {
                    let next_pair_price_opt = self
                        .infer_base_price(
                            chain_id,
                            chain_token_cfg.clone(),
                            s.to_string(),
                            redis_pool.clone(),
                            base_token.clone(),
                            Some(3),
                            Some(sel_symbol_set.clone()),
                        )
                        .await?;
                    if let Some(next_pair_price) = next_pair_price_opt {
                        return Ok(Some(next_pair_price * amount_b_f64 / amount_a_f64));
                    }
                }

                sel_symbol_set.remove(&s.to_string());
            }

            // no price can be infer from pool.

            Ok(None)
        }
        .boxed()
    }

    pub async fn get_usd_price_from_cmc_cache(
        &self,
        chain_id: UniEChain,
        token_address: &str,
        redis_pool: &RedisPool,
    ) -> Result<Option<f64>> {
        let token_info =
            util::get_cache_token_info(redis_pool, chain_id.to_string().as_str(), token_address)
                .await?;
        if let Some(token_info) = token_info {
            return Ok(Some(token_info.price));
        }

        Ok(None)
    }

    // for usdt price we use mainnet price
    pub async fn get_usdt_price_from_cmc_cache(
        &self,
        redis_pool: &RedisPool,
    ) -> Result<Option<f64>> {
        let usdt_addr = CMC_USDT_ADDR;
        self.get_usd_price_from_cmc_cache(UniEChain::EthMainnet, usdt_addr, redis_pool)
            .await
    }

    // for usdc price we use mainnet price
    pub async fn get_usdc_price_from_cmc_cache(
        &self,
        redis_pool: &RedisPool,
    ) -> Result<Option<f64>> {
        let usdt_addr = CMC_USDC_ADDR;
        self.get_usd_price_from_cmc_cache(UniEChain::EthMainnet, usdt_addr, redis_pool)
            .await
    }

    pub async fn get_stable_token_price(
        &self,
        redis_pool: &RedisPool,
        base_token: &str,
    ) -> Result<Option<f64>> {
        if base_token == "USDT" {
            return self.get_usdt_price_from_cmc_cache(redis_pool).await;
        } else if base_token == "USDC" {
            return self.get_usdc_price_from_cmc_cache(redis_pool).await;
        }

        panic!("not supported base token: {}", base_token);
    }

    // other funs
    // save the pair_info into redis and cal the usd price.
    pub async fn to_ws_swap_event_with_action(
        &self,
        log: LogParsedData,
        redis_pool: RedisPool,
    ) -> Result<Option<SwapPairEventInfo>> {
        // let log1 = log.clone();
        let LogParsedData::SwapLog {
            chain_id,
            pool,
            swap_key,
            from,
            to,
            token_0,
            token_1,
            symbol_0,
            symbol_1,
            name_0,
            name_1,
            decimal_0,
            decimal_1,
            is_order_book,
            trade_direction,
            can_get_fee,
            pool_fee,
            amount_0,
            amount_1,
            block_number,
            tx_index,
            tx_hash,
            transaction_at,
        } = log;

        let mut token_0_base_price = 0.0;
        let mut token_1_base_price = 0.0;
        let mut base_symbol_opt: Option<String> = None;
        let mut token_0_usd_p = 0.0;
        let mut token_1_usd_p = 0.0;
        let mut is_inferred_price = false;
        let chain_id_enum = UniEChain::from_str(chain_id.as_str())
            .map_err(|e| anyhow!("not support chain_id:{chain_id:#?}, err:{e:#?}"))?;

        self.save_v1(
            chain_id_enum,
            PairInfo {
                token_0: token_0.clone(),
                token_1: token_1.clone(),
                symbol_0: symbol_0.clone(),
                symbol_1: symbol_1.clone(),
                amount_0: amount_0.to_string(),
                amount_1: amount_1.to_string(),
                deciaml_0: decimal_0,
                deciaml_1: decimal_1,
            },
            &redis_pool,
        )
        .await?;

        let amount_0_f64 = format_amount_to_f64(amount_0.to_string().as_str(), decimal_0);
        let amount_1_f64 = format_amount_to_f64(amount_1.to_string().as_str(), decimal_1);

        let price_0_opt = self
            .infer_stable_token_price(chain_id_enum, token_0.clone(), redis_pool.clone())
            .await?;
        if let Some((price_0, base_symbol, _)) = price_0_opt {
            token_0_base_price = price_0;
            token_1_base_price = price_0 * amount_0_f64 / amount_1_f64;
            base_symbol_opt = Some(base_symbol);
            is_inferred_price = true;
        }

        let price_1_opt = self
            .infer_stable_token_price(chain_id_enum, token_1.clone(), redis_pool.clone())
            .await?;
        if let Some((price_1, base_symbol, _)) = price_1_opt {
            token_1_base_price = price_1;
            token_0_base_price = price_1 * amount_1_f64 / amount_0_f64;
            base_symbol_opt = Some(base_symbol);
            is_inferred_price = true;
        }

        if is_inferred_price {
            token_0_usd_p = token_0_base_price;
            token_1_usd_p = token_1_base_price;

            let base_symbol_str = base_symbol_opt.as_ref().unwrap();

            let stable_token_p_opt = self
                .get_stable_token_price(&redis_pool, base_symbol_str)
                .await?;

            // we adjust it to usd price. if have usdt price. if not we do nothing, else usdt/usd is similar to 1, we allow inaccuarcy
            if let Some(stable_price) = stable_token_p_opt {
                token_0_usd_p = stable_price * token_0_base_price;
                token_1_usd_p = stable_price * token_1_base_price;
            } else {
                warn!("no usdt price found from cmc cache. we use usdt price instead");
            }

            self.save_token_price_main(
                chain_id_enum,
                vec![
                    TokenUSDTInfo {
                        token: token_0.clone(),
                        symbol: symbol_0.clone(),
                        decimal: decimal_0,
                        price: token_0_base_price,
                    },
                    TokenUSDTInfo {
                        token: token_1.clone(),
                        symbol: symbol_1.clone(),
                        decimal: decimal_1,
                        price: token_1_base_price,
                    },
                ],
                vec![
                    // save smooth price.
                    TokenUSDInfo {
                        token: token_0.clone(),
                        symbol: symbol_0.clone(),
                        decimal: decimal_0,
                        price: token_0_usd_p,
                    },
                    TokenUSDInfo {
                        token: token_1.clone(),
                        symbol: symbol_1.clone(),
                        decimal: decimal_1,
                        price: token_1_usd_p,
                    },
                ],
                &redis_pool,
            )
            .await?;
        } else {
            // can't infer it from pool, we give up it.
            // debug!("can't infer price from pool, for log, chain_id:{:?}, swap_key:{}  we give it up. ", chain_id_enum, swap_key.clone());
            debug!("can't infer price from pool, for log, chain_id:{:?}, swap_key:{} symbol_0:{symbol_0}, symbol_1:{symbol_1}, token_0: {token_0}, token_1: {token_1}, we give it up. ", chain_id_enum, swap_key.clone());
            return Ok(None);

            // let price_0_opt =
            //     get_usd_price_from_cmc_cache(chain_id_enum, token_0.as_str(), &redis_pool).await?;
            // let price_1_opt =
            //     get_usd_price_from_cmc_cache(chain_id_enum, token_1.as_str(), &redis_pool).await?;
            // if let (Some(price_0), Some(price_1)) = (price_0_opt, price_1_opt) {
            //     warn!("can't infer price from pool, for log: {log1:?}, we use cmc usd price.");
            //     token_0_usd_p = price_0;
            //     token_1_usd_p = price_1;
            // } else {
            //     warn!("can't infer price from pool, for log: {log1:?}, we give it up.");
            //     return Ok(None);
            // }
        }

        let (time_order_uid, nonce) = util::create_time_order_uid(transaction_at).await;
        return Ok(Some(SwapPairEventInfo {
            chain_id,
            pool,
            swap_key,
            from,
            to,
            token_0,
            token_1,
            symbol_0,
            symbol_1,
            name_0,
            name_1,
            decimal_0,
            decimal_1,
            is_order_book,
            can_get_fee,
            pool_fee,
            trade_direction,
            block_number,
            tx_index,
            tx_hash,
            amount_0: amount_0.to_string(),
            amount_1: amount_1.to_string(),
            token_0_usd_price: token_0_usd_p.to_string(),
            token_1_usd_price: token_1_usd_p.to_string(),
            is_inferred_price,
            nonce,
            transaction_at,
            time_order_uid,
            fe_id: chain_id_enum.to_uni_string(),
        }));

        // return Err(anyhow!("not supported log event, log:{log:?}"));
    }
}

#[cfg(test)]
mod tests {
    use fred::types::Scanner;
    use futures_util::StreamExt;

    use super::*;

    async fn get_all_keys(redis_pool: RedisPool, patten: String) -> Result<Vec<String>> {
        let mut scan_stream = redis_pool.scan(patten, Some(1000), None);

        let mut vec = vec![];
        while let Some(val) = scan_stream.next().await {
            // if val_opt.is_none() {
            //     println!("val_opt is none");
            //     continue;
            // }
            if val.is_err() {
                println!("error");
                return Err(anyhow!("get all keys failed"));
            }

            let mut rs = val.unwrap();

            let rs1 = rs.take_results();

            if let Some(keys) = rs1 {
                for key in keys {
                    if let Some(s) = key.into_string() {
                        vec.push(s);
                    }
                }
            }
            rs.next()?;
        }
        println!("get_all_keys vec len: {}", vec.len());
        Ok(vec)
    }
    #[tokio::test]
    async fn show_redis_data() -> Result<()> {
        let (db_pool, redis_pool, app) = util::get_test_env().await?;
        let patten = format!("{}*:pair_info:SCROLLTEST*", REDIS_PREFIX);

        let keys = get_all_keys(redis_pool.clone(), patten).await?;

        add_to_log("".to_string(), true)?;

        for key in keys.iter() {
            println!("key: {:?}", key.to_string());

            let value: Option<PairInfo> = util::get_from_redis(key.clone(), &redis_pool).await?;
            add_to_log(format!("key: {}", key), false)?;
            add_to_log(format!("val: {:#?}\n\n", value), false)?
        }

        Ok(())
    }
}
