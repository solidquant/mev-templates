use criterion::{criterion_group, criterion_main, Criterion};
use ethers::{
    providers::{Http, Middleware, Provider, Ws},
    types::{BlockNumber, H160},
};
use ethers_core::k256::sha2::digest::block_buffer::Block;
use futures::prelude::*;
use std::{str::FromStr, sync::Arc, time::Instant};
use tokio::runtime::Runtime;

use rust::constants::Env;
use rust::paths::generate_triangular_paths;
use rust::pools::load_all_pools_from_v2;

pub fn benchmark_function(c: &mut Criterion) {
    /*
    Benchmarking tasks that really matter in MEV bots is the critical part here.
    The first two tasks are to illustrate how benchmarks will be done.
    */
    dotenv::dotenv().ok();
    let env = Env::new();

    println!("Starting benchmark");

    // 1. Create HTTP provider
    let s = Instant::now();
    let client = Provider::<Http>::try_from(env.https_url).unwrap();
    let client = Arc::new(client);
    let took = s.elapsed().as_micros();
    println!("1. HTTP provider creatio | Took: {:?} microsec", took);

    // runtime for async tasks
    let rt = Runtime::new().unwrap();

    // 2: Get block info
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

    // 3. Retrieving cached pools data
    let task = async {
        let factory_addresses = vec!["0xc35DADB65012eC5796536bD9864eD8773aBc74C4"];
        let factory_blocks = vec![11333218u64];

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
        let factory_addresses = vec!["0xc35DADB65012eC5796536bD9864eD8773aBc74C4"];
        let factory_blocks = vec![11333218u64];
        let pools = load_all_pools_from_v2(env.wss_url.clone(), factory_addresses, factory_blocks)
            .await
            .unwrap();
        let usdc_address = H160::from_str("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174").unwrap();

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
}

criterion_group!(benches, benchmark_function);
criterion_main!(benches);
