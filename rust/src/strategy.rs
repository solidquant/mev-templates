use ethers::{
    providers::{Provider, Ws},
    types::{Address, H160, U256},
};
use log::info;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::broadcast::Sender;

use crate::constants::{get_blacklist_tokens, Env, WEI};
use crate::multi::batch_get_uniswap_v2_reserves;
use crate::paths::generate_triangular_paths;
use crate::pools::{load_all_pools_from_v2, Pool};
use crate::simulator::UniswapV2Simulator;
use crate::streams::Event;
use crate::utils::get_touched_pool_reserves;

pub async fn event_handler(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let env = Env::new();

    let factory_addresses = vec!["0xc35DADB65012eC5796536bD9864eD8773aBc74C4"];
    let router_addresses = vec!["0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506"];
    let factory_blocks = vec![11333218u64];

    let pools_vec = load_all_pools_from_v2(env.wss_url.clone(), factory_addresses, factory_blocks)
        .await
        .unwrap();
    info!("Initial pool count: {}", pools_vec.len());

    // Performing USDC triangular arbitrage
    let usdc_address = H160::from_str("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174").unwrap();
    let usdc_decimals = 6;

    let paths = generate_triangular_paths(&pools_vec, usdc_address);

    let blacklist_tokens = get_blacklist_tokens();

    let mut pools = HashMap::new();

    for path in &paths {
        if !path.should_blacklist(&blacklist_tokens) {
            pools.insert(path.pool_1.address.clone(), path.pool_1.clone());
            pools.insert(path.pool_2.address.clone(), path.pool_2.clone());
            pools.insert(path.pool_3.address.clone(), path.pool_3.clone());
        }
    }
    info!("New pool count: {:?}", pools.len());

    let pools_vec: Vec<Pool> = pools.values().cloned().collect();
    let mut reserves =
        batch_get_uniswap_v2_reserves(env.https_url.clone(), pools_vec.clone()).await;

    let mut event_receiver = event_sender.subscribe();

    loop {
        match event_receiver.recv().await {
            Ok(event) => match event {
                Event::Block(block) => {
                    info!("{:?}", block);
                    let touched_reserves =
                        match get_touched_pool_reserves(provider.clone(), block.block_number).await
                        {
                            Ok(response) => response,
                            Err(e) => {
                                info!("Error from get_touched_pool_reserves: {:?}", e);
                                HashMap::new()
                            }
                        };
                    let mut touched_pools = Vec::new();
                    for (address, reserve) in touched_reserves.into_iter() {
                        if reserves.contains_key(&address) {
                            reserves.insert(address, reserve);
                            touched_pools.push(address);
                        }
                    }
                    info!("{:?}", touched_pools);

                    let mut spreads = HashMap::new();
                    for (idx, path) in (&paths).iter().enumerate() {
                        let touched_path = touched_pools
                            .iter()
                            .map(|pool| path.has_pool(&pool) as i32)
                            .sum::<i32>()
                            >= 1;

                        if touched_path {
                            let one_token_in = U256::from(1);
                            let simulated = path.simulate_v2_path(one_token_in, &reserves);

                            match simulated {
                                Some(price_quote) => {
                                    let one_usdc_in = one_token_in * U256::from(usdc_decimals);
                                    let _out = price_quote.as_u128() as i128;
                                    let _in = one_usdc_in.as_u128() as i128;
                                    let spread = _out - _in;

                                    if spread > 0 {
                                        spreads.insert(idx, spread);
                                    }
                                }
                                None => {}
                            }
                        }
                    }

                    let wmatic_usdc_address =
                        Address::from_str("0xcd353F79d9FADe311fC3119B841e1f456b54e858").unwrap();
                    let pool = pools.get(&wmatic_usdc_address).unwrap();
                    let reserve = reserves.get(&wmatic_usdc_address).unwrap();
                    let wmatic_price = UniswapV2Simulator::reserves_to_price(
                        reserve.reserve0,
                        reserve.reserve1,
                        pool.decimals0,
                        pool.decimals1,
                        true,
                    );

                    let base_fee = block.next_base_fee;
                    let estimated_gas_usage = U256::from(550000);
                    let gas_cost_in_wei = base_fee * estimated_gas_usage;
                    let gas_cost_in_wmatic =
                        (gas_cost_in_wei.as_u64() as f64) / ((*WEI).as_u64() as f64);
                    let gas_cost_in_usdc = wmatic_price * gas_cost_in_wmatic;
                    let gas_cost_in_usdc =
                        U256::from((gas_cost_in_usdc * ((10 as f64).powi(usdc_decimals))) as u64);

                    let mut sorted_spreads: Vec<_> = spreads.iter().collect();
                    sorted_spreads.sort_by_key(|x| x.1);
                    sorted_spreads.reverse();

                    for spread in sorted_spreads {
                        let path_idx = spread.0;
                        let path = &paths[*path_idx];
                        let opt = path.optimize_amount_in(U256::from(1000), 10, &reserves);
                        let excess_profit =
                            (opt.1.as_u128() as i128) - (gas_cost_in_usdc.as_u128() as i128);

                        // TODO
                        if excess_profit > 0 {}
                    }
                }
                Event::PendingTx(_) => {
                    // not using pending tx
                }
                Event::Log(_) => {
                    // not using logs
                }
            },
            Err(_) => {}
        }
    }
}
