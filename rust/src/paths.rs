use ethers::types::H160;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use std::time::Instant;

use crate::pools::Pool;

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
    pub fn has_pool(&self, pool: H160) -> bool {
        let is_pool_1 = self.pool_1.address == pool;
        let is_pool_2 = self.pool_2.address == pool;
        let is_pool_3 = self.pool_3.address == pool;
        return is_pool_1 || is_pool_2 || is_pool_3;
    }

    pub fn should_blacklist(&self, blacklist_tokens: &Vec<H160>) -> bool {
        for i in 0..self.nhop {
            let pool = match i {
                0 => Some(&self.pool_1),
                1 => Some(&self.pool_2),
                2 => Some(&self.pool_3),
                _ => None,
            }
            .unwrap();
            return blacklist_tokens.contains(&pool.token0)
                || blacklist_tokens.contains(&pool.token1);
        }
        false
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
            let token_out_1 = if zero_for_one_1 {
                pool_1.token1
            } else {
                pool_1.token0
            };

            for j in 0..pools.len() {
                let pool_2 = &pools[j];
                let can_trade_2 = (pool_2.token0 == token_out_1) || (pool_2.token1 == token_out_1);

                if can_trade_2 {
                    let zero_for_one_2 = pool_2.token0 == token_out_1;
                    let token_out_2 = if zero_for_one_2 {
                        pool_2.token1
                    } else {
                        pool_2.token0
                    };

                    for k in 0..pools.len() {
                        let pool_3 = &pools[k];
                        let can_trade_3 =
                            (pool_3.token0 == token_out_2) || (pool_3.token1 == token_out_2);

                        if can_trade_3 {
                            let zero_for_one_3 =
                                (pool_3.token0 == token_out_2) || (pool_3.token1 == token_out_2);
                            let token_out_3 = if zero_for_one_3 {
                                pool_3.token1
                            } else {
                                pool_3.token0
                            };

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
                                pb.inc(1);
                            }
                        }
                    }
                }
            }
        }
    }

    pb.finish_with_message(format!(
        "Generated {} 3-hop arbitrage paths in {} seconds",
        paths.len(),
        start_time.elapsed().as_secs()
    ));
    paths
}
