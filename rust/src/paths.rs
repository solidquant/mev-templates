use ethers::types::{H160, U256};
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use std::{collections::HashMap, time::Instant};

use crate::bundler::PathParam;
use crate::multi::Reserve;
use crate::pools::Pool;
use crate::simulator::UniswapV2Simulator;

#[derive(Debug, Clone)]
pub struct ArbPath {
    pub nhop: u8,
    pub pool_1: Pool,
    pub pool_2: Pool,
    pub pool_3: Pool,
    pub zero_for_one_1: bool,
    pub zero_for_one_2: bool,
    pub zero_for_one_3: bool,
}

impl ArbPath {
    pub fn has_pool(&self, pool: &H160) -> bool {
        let is_pool_1 = self.pool_1.address == *pool;
        let is_pool_2 = self.pool_2.address == *pool;
        let is_pool_3 = self.pool_3.address == *pool;
        return is_pool_1 || is_pool_2 || is_pool_3;
    }

    pub fn _get_pool(&self, i: u8) -> &Pool {
        match i {
            0 => Some(&self.pool_1),
            1 => Some(&self.pool_2),
            2 => Some(&self.pool_3),
            _ => None,
        }
        .unwrap()
    }

    pub fn _get_zero_for_one(&self, i: u8) -> bool {
        match i {
            0 => Some(self.zero_for_one_1),
            1 => Some(self.zero_for_one_2),
            2 => Some(self.zero_for_one_3),
            _ => None,
        }
        .unwrap()
    }

    pub fn should_blacklist(&self, blacklist_tokens: &Vec<H160>) -> bool {
        for i in 0..self.nhop {
            let pool = self._get_pool(i);
            return blacklist_tokens.contains(&pool.token0)
                || blacklist_tokens.contains(&pool.token1);
        }
        false
    }

    pub fn simulate_v2_path(
        &self,
        amount_in: U256,
        reserves: &HashMap<H160, Reserve>,
    ) -> Option<U256> {
        let token_in_decimals = if self.zero_for_one_1 {
            self.pool_1.decimals0
        } else {
            self.pool_1.decimals1
        };
        let unit = U256::from(10).pow(U256::from(token_in_decimals));
        let mut amount_out = amount_in * unit;

        for i in 0..self.nhop {
            let pool = self._get_pool(i);
            let zero_for_one = self._get_zero_for_one(i);

            let reserve = reserves.get(&pool.address)?;
            let reserve0 = reserve.reserve0;
            let reserve1 = reserve.reserve1;
            let fee = U256::from(pool.fee);

            let reserve_in;
            let reserve_out;
            if zero_for_one {
                reserve_in = reserve0;
                reserve_out = reserve1;
            } else {
                reserve_in = reserve1;
                reserve_out = reserve0;
            }

            amount_out =
                UniswapV2Simulator::get_amount_out(amount_out, reserve_in, reserve_out, fee)?;
        }

        Some(amount_out)
    }

    pub fn optimize_amount_in(
        &self,
        max_amount_in: U256,
        step_size: usize,
        reserves: &HashMap<H160, Reserve>,
    ) -> (U256, U256) {
        let token_in_decimals = if self.zero_for_one_1 {
            self.pool_1.decimals0
        } else {
            self.pool_1.decimals1
        };

        let mut optimized_in = U256::zero();
        let mut profit = 0;

        for amount_in in (0..max_amount_in.as_u64()).step_by(step_size) {
            let amount_in = U256::from(amount_in);
            let unit = U256::from(10).pow(U256::from(token_in_decimals));
            if let Some(amount_out) = self.simulate_v2_path(amount_in, &reserves) {
                let this_profit =
                    (amount_out.as_u128() as i128) - ((amount_in * unit).as_u128() as i128);
                if this_profit >= profit {
                    optimized_in = amount_in;
                    profit = this_profit;
                } else {
                    break;
                }
            }
        }

        (optimized_in, U256::from(profit))
    }

    pub fn to_path_params(&self, routers: &Vec<H160>) -> Vec<PathParam> {
        let mut path_params = Vec::new();
        for i in 0..self.nhop {
            let pool = self._get_pool(i);
            let zero_for_one = self._get_zero_for_one(i);

            let token_in;
            let token_out;
            if zero_for_one {
                token_in = pool.token0;
                token_out = pool.token1;
            } else {
                token_in = pool.token1;
                token_out = pool.token0;
            }

            let param = PathParam {
                router: routers[i as usize],
                token_in: token_in,
                token_out: token_out,
            };
            path_params.push(param);
        }
        path_params
    }
}

pub fn generate_triangular_paths(pools: &Vec<Pool>, token_in: H160) -> Vec<ArbPath> {
    let start_time = Instant::now();

    let token_out = token_in.clone();
    let mut paths = Vec::new();

    let pb = ProgressBar::new(pools.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-"),
    );

    for i in 0..pools.len() {
        let pool_1 = &pools[i];
        let can_trade_1 = (pool_1.token0 == token_in) || (pool_1.token1 == token_in);

        if can_trade_1 {
            let zero_for_one_1 = pool_1.token0 == token_in;
            let (token_in_1, token_out_1) = if zero_for_one_1 {
                (pool_1.token0, pool_1.token1)
            } else {
                (pool_1.token1, pool_1.token0)
            };
            if token_in_1 != token_in {
                continue;
            }

            for j in 0..pools.len() {
                let pool_2 = &pools[j];
                let can_trade_2 = (pool_2.token0 == token_out_1) || (pool_2.token1 == token_out_1);

                if can_trade_2 {
                    let zero_for_one_2 = pool_2.token0 == token_out_1;
                    let (token_in_2, token_out_2) = if zero_for_one_2 {
                        (pool_2.token0, pool_2.token1)
                    } else {
                        (pool_2.token1, pool_2.token0)
                    };
                    if token_out_1 != token_in_2 {
                        continue;
                    }

                    for k in 0..pools.len() {
                        let pool_3 = &pools[k];
                        let can_trade_3 =
                            (pool_3.token0 == token_out_2) || (pool_3.token1 == token_out_2);

                        if can_trade_3 {
                            let zero_for_one_3 =
                                (pool_3.token0 == token_out_2) || (pool_3.token1 == token_out_2);
                            let (token_in_3, token_out_3) = if zero_for_one_3 {
                                (pool_3.token0, pool_3.token1)
                            } else {
                                (pool_3.token1, pool_3.token0)
                            };
                            if token_out_2 != token_in_3 {
                                continue;
                            }

                            if token_out_3 == token_out {
                                let unique_pool_cnt =
                                    vec![pool_1.address, pool_2.address, pool_3.address]
                                        .into_iter()
                                        .unique()
                                        .collect::<Vec<H160>>()
                                        .len();

                                if unique_pool_cnt < 3 {
                                    continue;
                                }

                                let arb_path = ArbPath {
                                    nhop: 3,
                                    pool_1: pool_1.clone(),
                                    pool_2: pool_2.clone(),
                                    pool_3: pool_3.clone(),
                                    zero_for_one_1: zero_for_one_1,
                                    zero_for_one_2: zero_for_one_2,
                                    zero_for_one_3: zero_for_one_3,
                                };

                                paths.push(arb_path);
                            }
                        }
                    }
                }
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message(format!(
        "Generated {} 3-hop arbitrage paths in {} seconds",
        paths.len(),
        start_time.elapsed().as_secs()
    ));
    paths
}
