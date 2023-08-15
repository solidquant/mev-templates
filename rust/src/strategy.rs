use ethers::{
    providers::{Provider, Ws},
    types::{H160, U256},
};
use log::info;
use std::{collections::HashMap, path::Path, str::FromStr, sync::Arc};
use tokio::sync::broadcast::{self, Sender};
use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use crate::abi::ABI;
use crate::constants::{get_blacklist_tokens, Env};
use crate::multi::batch_get_uniswap_v2_reserves;
use crate::paths::generate_triangular_paths;
use crate::pools::{load_all_pools_from_v2, Pool};
use crate::simulator::UniswapV2Simulator;
use crate::streams::{stream_new_blocks, stream_pending_transactions, Event, NewBlock};
use crate::utils::{get_touched_pool_reserves, setup_logger};

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

                    info!("{:?}", spreads);
                }
                Event::PendingTx(_) => {
                    // not using pending tx
                }
            },
            Err(_) => {}
        }
    }
}
