use anvil::eth::fees::calculate_next_block_base_fee;
use cfmms::dex::DexVariant;
use ethers::{
    prelude::Lazy,
    providers::{Provider, Ws},
    types::{Address, BlockNumber, H160, U256},
};
use ethers_providers::Middleware;
use log::info;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::broadcast::Sender;

use crate::bundler::{make_path_params, order_tx_calldata, Flashloan};
use crate::constants::{get_blacklist_tokens, Env, WEI, ZERO_ADDRESS};
use crate::evm::EvmSimulator;
use crate::multi::{batch_get_uniswap_v2_reserves, get_uniswap_v3_liquidity};
use crate::paths::{generate_triangular_paths, ArbPath};
use crate::pools::{load_all_pools, Pool};
use crate::simulator::UniswapV2Simulator;
use crate::streams::{Event, NewBlock};
use crate::utils::get_touched_pool_reserves;

pub static ENV: Lazy<Env> = Lazy::new(|| Env::new());

pub fn should_skip_path(skip_paths_map: &HashMap<String, bool>, path: &ArbPath) -> bool {
    let key = format!(
        "{:?}_{:?}_{:?}",
        path.pool_1.address, path.pool_2.address, path.pool_3.address,
    );
    *skip_paths_map.get(&key).unwrap_or(&false)
}

pub async fn event_handler(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    /*
    DEX arbitrage advanced strategy is an attempt to show how revm can be used in arb. path simulations.
    */
    let factories = vec![
        (
            // Crypto.com swap
            "0x9DEB29c9a4c7A88a3C0257393b7f3335338D9A9D",
            DexVariant::UniswapV2,
            10828414u64,
        ),
        (
            // Convergence swap
            "0x4eef5746ED22A2fD368629C1852365bf5dcb79f1",
            DexVariant::UniswapV2,
            12385067u64,
        ),
        (
            // ShibaSwap
            "0x115934131916C8b277DD010Ee02de363c09d037c",
            DexVariant::UniswapV2,
            12771526u64,
        ),
        (
            // Saitaswap
            "0x35113a300ca0D7621374890ABFEAC30E88f214b1",
            DexVariant::UniswapV2,
            15210780u64,
        ),
    ];

    let pools_vec = load_all_pools((*ENV).wss_url.clone(), factories)
        .await
        .unwrap();
    info!("Initial pool count: {}", pools_vec.len());

    let weth_address = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
    let weth_decimals = 18;

    let paths = generate_triangular_paths(&pools_vec, weth_address);

    let mut pools = HashMap::new();

    for path in &paths {
        pools.insert(path.pool_1.address.clone(), path.pool_1.clone());
        pools.insert(path.pool_2.address.clone(), path.pool_2.clone());
        pools.insert(path.pool_3.address.clone(), path.pool_3.clone());
    }
    info!("New pool count: {:?}", pools.len());

    let pools_vec: Vec<Pool> = pools.values().cloned().collect();
    let mut reserves =
        batch_get_uniswap_v2_reserves((*ENV).https_url.clone(), pools_vec.clone()).await;

    // get the spread for all the triangular paths to filter out
    // pools with too much price impact (we won't be able to trade on those pools anyways)
    // let mut skip_paths = HashMap::new();
    // let mut spreads = HashMap::new();

    let one_token_in = U256::from(1);
    let unit = U256::from(10).pow(U256::from(weth_decimals));

    let ws = Ws::connect((*ENV).wss_url.clone()).await.unwrap();
    let provider = Arc::new(Provider::new(ws));

    for (idx, path) in (&paths).iter().enumerate() {
        // let key = format!(
        //     "{:?}_{:?}_{:?}",
        //     path.pool_1.address, path.pool_2.address, path.pool_3.address,
        // );
        // if key != "0x919a0fd38c52342a5848a8897bb3d4a2b5a07947_0x9ef7e917fb41cc02f78a5c99b42f497ed8979350_0xc95389f2883cf6c9099a25f1cf356d881e5a07c6" {
        //     continue;
        // }

        info!("{:?}", path);
        let simulated = path.simulate_v2_path(one_token_in, &reserves);

        match simulated {
            Some(price_quote) => {
                info!("Simulated out: {:?}", price_quote);
                // let one_weth_in = one_token_in * unit;
                // let _out = price_quote.as_u128() as f64;
                // let _in = one_weth_in.as_u128() as f64;
                // let spread = (_out / _in - 1.0) * 100.0;

                // if spread > 10.0 {
                //     info!("{:?} {:?}", path, spread);
                //     spreads.insert(idx, spread);
                // }

                // if spread < 0.95 {
                //     let skip_path = format!(
                //         "{:?}_{:?}_{:?}",
                //         path.pool_1.address, path.pool_2.address, path.pool_3.address,
                //     );
                //     skip_paths.insert(skip_path, true);
                // }
            }
            None => {}
        }

        // run an EVM simulation
        let evm = EvmSimulator::new(&(*ENV));
        let block = provider
            .get_block(BlockNumber::Latest)
            .await
            .unwrap()
            .unwrap();
        let new_block = NewBlock {
            number: block.number.unwrap(),
            base_fee_per_gas: block.base_fee_per_gas.unwrap_or_default(),
            next_base_fee: U256::from(calculate_next_block_base_fee(
                block.gas_used.as_u64(),
                block.gas_limit.as_u64(),
                block.base_fee_per_gas.unwrap_or_default().as_u64(),
            )),
            timestamp: block.timestamp,
            gas_used: block.gas_used,
            gas_limit: block.gas_limit,
        };
        let path_params = make_path_params(path);
        let amount_in = one_token_in * unit;
        let flashloan = Flashloan::NotUsed;
        let loan_from = *ZERO_ADDRESS;
        let calldata = order_tx_calldata(&path_params, amount_in, flashloan, loan_from);
        let amount_out = evm
            .simulate_weth_arbitrage(&new_block, weth_address, amount_in, calldata)
            .await
            .unwrap();
        info!("EVM amount out: {:?}", amount_out);

        break;
    }

    // let mut sorted_spreads: Vec<_> = spreads.iter().collect();
    // sorted_spreads.sort_by(|a, b| (a.1).partial_cmp(b.1).unwrap());
    // sorted_spreads.reverse();

    // info!("{:?}", &sorted_spreads[0..100]);

    let mut event_receiver = event_sender.subscribe();

    loop {
        match event_receiver.recv().await {
            Ok(event) => match event {
                Event::Block(block) => {
                    info!("{:?}", block);
                    let touched_reserves =
                        match get_touched_pool_reserves(provider.clone(), block.number).await {
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
                }
                Event::PendingTx(_) => {}
            },
            Err(_) => {}
        }
    }
}
