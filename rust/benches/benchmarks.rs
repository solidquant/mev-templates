use chrono::prelude::*;
use criterion::{criterion_group, criterion_main, Criterion};
use ethers::{
    providers::{Http, Middleware, Provider, Ws},
    types::{
        transaction::eip2930::AccessList, BlockNumber, Bytes, Eip1559TransactionRequest,
        NameOrAddress, H160, U256,
    },
};
use std::{path::Path, str::FromStr, sync::Arc, time::Instant};
use tokio::runtime::Runtime;
use tokio::sync::broadcast::{self, Sender};
use tokio::task::JoinSet;

use rust::bundler::{Bundler, Flashloan};
use rust::constants::{Env, ZERO_ADDRESS};
use rust::multi::{batch_get_uniswap_v2_reserves, get_uniswap_v2_reserves};
use rust::paths::generate_triangular_paths;
use rust::pools::load_all_pools_from_v2;
use rust::streams::{stream_new_blocks, stream_pending_transactions, Event};
use rust::utils::{calculate_next_block_base_fee, get_touched_pool_reserves};

pub async fn logging_event_handler(_: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let benchmark_file = Path::new("benches/.benchmark.csv");
    let mut writer = csv::Writer::from_path(benchmark_file).unwrap();

    let mut event_receiver = event_sender.subscribe();

    loop {
        match event_receiver.recv().await {
            Ok(event) => match event {
                Event::Block(_) => {}
                Event::PendingTx(tx) => {
                    let now = Local::now().timestamp_micros();
                    writer.serialize((tx.hash, now)).unwrap();
                }
                Event::Log(_) => {}
            },
            Err(_) => {}
        }
    }
}

pub async fn touched_pools_event_handler(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let mut event_receiver = event_sender.subscribe();

    loop {
        match event_receiver.recv().await {
            Ok(event) => match event {
                Event::Block(block) => {
                    let s = Instant::now();
                    match get_touched_pool_reserves(provider.clone(), block.block_number).await {
                        Ok(reserves) => {
                            let took = s.elapsed().as_millis();
                            let now = Instant::now();
                            println!(
                                "[{:?}] Block #{:?} {:?} pools touched | Took: {:?} ms",
                                now,
                                block.block_number,
                                reserves.len(),
                                took
                            );
                        }
                        Err(_) => {}
                    }
                }
                Event::PendingTx(_) => {}
                Event::Log(_) => {}
            },
            Err(_) => {}
        }
    }
}

pub async fn full_course_event_handler(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    // pass
}

pub fn benchmark_function(_: &mut Criterion) {
    /*
    Benchmarking tasks that really matter in MEV bots is the critical part here.
    The first two tasks are to illustrate how benchmarks will be done.

    All benchmarks are done using a local full node running on another machine.
    - Running on the same machine will make the async tasks here go faster.
    - Using node services like Infura/Alchemy will make this go considerably slower.
    */
    dotenv::dotenv().ok();
    let env = Env::new();

    println!("Starting benchmark");

    // 1. Create HTTP provider
    let s = Instant::now();
    let client = Provider::<Http>::try_from(env.https_url.clone()).unwrap();
    let client = Arc::new(client);
    let took = s.elapsed().as_micros();
    println!("1. HTTP provider created | Took: {:?} microsec", took);

    // runtime for async tasks
    let rt = Runtime::new().unwrap();

    // 2: Get block info
    let mut runs = 10;

    // Testing the call synchronously: one by one
    loop {
        let task = async {
            let s = Instant::now();
            let block = client.clone().get_block(BlockNumber::Latest).await.unwrap();
            let took = s.elapsed().as_millis();
            println!(
                "2. New block: #{:?} | Took: {:?} ms",
                block.unwrap().number.unwrap(),
                took
            );
        };
        rt.block_on(task);

        runs -= 1;
        if runs == 0 {
            break;
        }
    }

    // 3. Retrieving cached pools data
    let task = async {
        let factory_addresses = vec!["0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"];
        let factory_blocks = vec![10794229u64];

        let s = Instant::now();
        let pools = load_all_pools_from_v2(env.wss_url.clone(), factory_addresses, factory_blocks)
            .await
            .unwrap();
        let took = s.elapsed().as_millis();
        println!(
            "3. Cached {:?} pools data | Took: {:?} ms",
            pools.len(),
            took
        );
    };
    rt.block_on(task);

    // 4. Generate triangular arbitrage paths
    let task = async {
        let factory_addresses = vec!["0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"];
        let factory_blocks = vec![10794229u64];
        let pools = load_all_pools_from_v2(env.wss_url.clone(), factory_addresses, factory_blocks)
            .await
            .unwrap();
        let usdc_address = H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();

        let s = Instant::now();
        let paths = generate_triangular_paths(&pools, usdc_address);
        let took = s.elapsed().as_millis();
        println!(
            "4. Generated {:?} 3-hop paths | Took: {:?} ms",
            paths.len(),
            took
        );
    };
    rt.block_on(task);

    // 5. Multicall test: calling 250 requests using multicall
    // This is used quite often in real bots.

    // Single multicall
    let task = async {
        let factory_addresses = vec!["0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"];
        let factory_blocks = vec![10794229u64];
        let pools = load_all_pools_from_v2(env.wss_url.clone(), factory_addresses, factory_blocks)
            .await
            .unwrap();

        let s = Instant::now();
        let reserves = get_uniswap_v2_reserves(env.https_url.clone(), pools[0..250].to_vec())
            .await
            .unwrap();
        let took = s.elapsed().as_millis();
        println!(
            "5. Multicall result for {:?} | Took: {:?} ms",
            reserves.len(),
            took
        );
    };
    rt.block_on(task);

    // Batch multicall (thousands of requests asynchronously)
    let task = async {
        let factory_addresses = vec!["0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"];
        let factory_blocks = vec![10794229u64];
        let pools = load_all_pools_from_v2(env.wss_url.clone(), factory_addresses, factory_blocks)
            .await
            .unwrap();

        let s = Instant::now();
        let reserves = batch_get_uniswap_v2_reserves(env.https_url.clone(), pools).await;
        let took = s.elapsed().as_millis();
        println!(
            "5. Bulk multicall result for {:?} | Took: {:?} ms",
            reserves.len(),
            took
        );
    };
    rt.block_on(task);

    /*
    6. Going to start running async streams and record the data on csv files
       to see how long it takes to receive the data on average.

       There may be better ways to pull this off, but the best use case benchmarks are those
       that benchmark the exact same setup that people use.

       Most people running MEV bots using Rust will use ethers-rs Provider<Ws> to stream
       real-time data. And that is what I'm testing, without having to look under the hood.
    */
    // let task = async {
    //     let ws = Ws::connect(env.wss_url.clone()).await.unwrap();
    //     let provider = Arc::new(Provider::new(ws));

    //     let (event_sender, _): (Sender<Event>, _) = broadcast::channel(512);

    //     let mut set = JoinSet::new();

    //     // try running the stream for n seconds
    //     set.spawn(tokio::time::timeout(
    //         std::time::Duration::from_secs(180),
    //         stream_pending_transactions(provider.clone(), event_sender.clone()),
    //     ));

    //     set.spawn(tokio::time::timeout(
    //         std::time::Duration::from_secs(180),
    //         logging_event_handler(provider.clone(), event_sender.clone()),
    //     ));

    //     println!("6. Logging receive time for pending transaction streams. Wait 180 seconds...");
    //     while let Some(res) = set.join_next().await {
    //         println!("Closed: {:?}", res);
    //     }
    // };
    // rt.block_on(task);

    // 7. Retrieving logs from a newly created block
    // let task = async {
    //     let ws = Ws::connect(env.wss_url.clone()).await.unwrap();
    //     let provider = Arc::new(Provider::new(ws));

    //     let (event_sender, _): (Sender<Event>, _) = broadcast::channel(512);

    //     let mut set = JoinSet::new();

    //     // try running the stream for n seconds
    //     set.spawn(tokio::time::timeout(
    //         std::time::Duration::from_secs(60 * 5),
    //         stream_new_blocks(provider.clone(), event_sender.clone()),
    //     ));

    //     set.spawn(tokio::time::timeout(
    //         std::time::Duration::from_secs(60 * 5),
    //         touched_pools_event_handler(provider.clone(), event_sender.clone()),
    //     ));

    //     // test for at least 5 minutes
    //     println!("7. Starting touched pools with new blocks streams. Wait 300 seconds...");
    //     while let Some(res) = set.join_next().await {
    //         println!("Closed: {:?}", res);
    //     }
    // };
    // rt.block_on(task);

    // 8. 3-hop path simulation
    let task = async {
        let factory_addresses = vec!["0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"];
        let factory_blocks = vec![10794229u64];
        let pools = load_all_pools_from_v2(env.wss_url.clone(), factory_addresses, factory_blocks)
            .await
            .unwrap();
        let usdc_address = H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();

        let paths = generate_triangular_paths(&pools, usdc_address);
        let reserves = batch_get_uniswap_v2_reserves(env.https_url.clone(), pools).await;

        let took = paths.iter().map(|path| {
            let s = Instant::now();
            let amount_in = U256::from(1);
            match path.simulate_v2_path(amount_in, &reserves) {
                Some(_) => {}
                None => {}
            };
            return s.elapsed().as_micros() as i32;
        });
        let total_took = took.clone().into_iter().sum::<i32>();
        println!(
            "8. 3-hop path simulation took: {:?} microsecs in total ({:?} simulations)",
            total_took,
            took.len()
        );
    };
    rt.block_on(task);

    // 9. Creating flashbots bundles
    let task = async {
        let factory_addresses = vec!["0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"];
        let factory_blocks = vec![10794229u64];
        let pools = load_all_pools_from_v2(env.wss_url.clone(), factory_addresses, factory_blocks)
            .await
            .unwrap();
        let usdc_address = H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
        let usdc_decimals = 6;

        let paths = generate_triangular_paths(&pools, usdc_address);

        let unit = U256::from(10).pow(U256::from(usdc_decimals));
        let gwei = U256::from(10).pow(U256::from(9));

        let bundler = Bundler::new();
        let block_number = bundler.provider.get_block_number().await.unwrap();

        let s = Instant::now();
        let router_address = "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F";
        let routers = vec![
            H160::from_str(router_address).unwrap(),
            H160::from_str(router_address).unwrap(),
            H160::from_str(router_address).unwrap(),
        ];
        let path = &paths[0];
        let path_params = path.to_path_params(&routers);
        let amount_in = U256::from(1) * unit;
        let flashloan = Flashloan::NotUsed;
        let loan_from = *ZERO_ADDRESS;
        let max_priority_fee_per_gas = U256::from(1) * gwei;
        let max_fee_per_gas = U256::from(50) * gwei;
        let order_tx = bundler
            .order_tx(
                path_params,
                amount_in,
                flashloan,
                loan_from,
                max_priority_fee_per_gas,
                max_fee_per_gas,
            )
            .await
            .unwrap();
        let signed_tx = bundler.sign_tx(order_tx).await.unwrap();
        let bundle = bundler.to_bundle(vec![signed_tx], block_number);
        let took = s.elapsed().as_millis();
        println!("9. Creating Flashbots bundle | Took: {:?} ms", took);
        println!("{:?}", bundle);
    };
    rt.block_on(task);

    // 10. Sending Flashbots bundles
    // Going to test out a real transaction with a wallet that has some ETH: Sending ETH to myself
    let task = async {
        let mut time_took = Vec::new();

        for n in 0..10 {
            let bundler = Bundler::new();
            let block = bundler
                .provider
                .get_block(BlockNumber::Latest)
                .await
                .unwrap()
                .unwrap();
            let next_base_fee = U256::from(calculate_next_block_base_fee(
                block.gas_used,
                block.gas_limit,
                block.base_fee_per_gas.unwrap_or_default(),
            ));
            let max_priority_fee_per_gas = U256::from(1);
            let max_fee_per_gas = next_base_fee + max_priority_fee_per_gas;

            let _s = Instant::now();
            let s = Instant::now();
            let common = bundler._common_fields().await.unwrap();
            let to = NameOrAddress::Address(common.0);
            let amount_in = U256::from(1) * U256::from(10).pow(U256::from(15)); // 0.001
            let tx = Eip1559TransactionRequest {
                to: Some(to),
                from: Some(common.0),
                data: Some(Bytes(bytes::Bytes::new())),
                value: Some(amount_in),
                chain_id: Some(common.2),
                max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
                max_fee_per_gas: Some(max_fee_per_gas),
                gas: Some(U256::from(30000)),
                nonce: Some(common.1),
                access_list: AccessList::default(),
            };
            let signed_tx = bundler.sign_tx(tx).await.unwrap();
            let bundle = bundler.to_bundle(vec![signed_tx], block.number.unwrap());
            let took = s.elapsed().as_millis();
            println!("- Creating bundle took: {:?} ms", took);

            let s = Instant::now();
            let simulated = bundler
                .flashbots
                .inner()
                .simulate_bundle(&bundle)
                .await
                .unwrap();

            for tx in &simulated.transactions {
                if let Some(e) = &tx.error {
                    println!("Simulation error: {e:?}");
                }
                if let Some(r) = &tx.revert {
                    println!("Simulation revert: {r:?}");
                }
            }
            let took = s.elapsed().as_millis();
            println!("- Running simulation took: {:?} ms", took);

            let s = Instant::now();
            let pending_bundle = bundler
                .flashbots
                .inner()
                .send_bundle(&bundle)
                .await
                .unwrap();

            let took = s.elapsed().as_millis();
            let total_took = _s.elapsed().as_millis();
            println!(
                "10. Sending Flashbots bundle ({:?}) | Took: {:?} ms",
                pending_bundle.bundle_hash, took
            );

            time_took.push(total_took as u128);
        }
        println!("{:?}", time_took.iter().copied().sum::<u128>());
    };
    rt.block_on(task);

    // 11. Full course testing
    // ==> Receive new block / get touched pools / simulate paths / create flashbots bundle / send bundle
}

criterion_group!(benches, benchmark_function);
criterion_main!(benches);
