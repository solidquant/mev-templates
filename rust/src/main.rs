use anyhow::{anyhow, Ok, Result};
use ethers::{
    providers::{Provider, Ws},
    types::H160,
};
use log::info;
use std::{path::Path, str::FromStr, sync::Arc};
use tokio::sync::broadcast::{self, Sender};
use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use rust::{
    abi::ABI,
    constants::Env,
    event_handler::event_handler,
    multi::batch_get_uniswap_v2_reserves,
    paths::generate_triangular_paths,
    pools::{load_all_pools_from_v2, Pool},
    streams::{stream_new_blocks, stream_pending_transactions, Event, NewBlock},
    utils::setup_logger,
};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    setup_logger()?;

    let env = Env::new();

    let factory_addresses = vec!["0xc35DADB65012eC5796536bD9864eD8773aBc74C4"];
    let router_addresses = vec!["0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506"];
    let factory_blocks = vec![11333218u64];

    let pools_vec = load_all_pools_from_v2(env.wss_url.clone(), factory_addresses, factory_blocks)
        .await
        .unwrap();
    info!("Initial pool count: {}", pools_vec.len());

    let usdc_address = H160::from_str("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174")?;
    let usdc_decimals = 6;

    let paths = generate_triangular_paths(&pools_vec, usdc_address);

    let reserves = batch_get_uniswap_v2_reserves(env.https_url.clone(), pools_vec.clone()).await;
    info!("{:?}", reserves.len());

    // let ws = Ws::connect(env.wss_url).await?;
    // let provider = Arc::new(Provider::new(ws));

    // let (event_sender, _): (Sender<Event>, _) = broadcast::channel(512);

    // let mut set = JoinSet::new();

    // set.spawn(stream_new_blocks(provider.clone(), event_sender.clone()));
    // // set.spawn(stream_pending_transactions(
    // //     provider.clone(),
    // //     event_sender.clone(),
    // // ));
    // set.spawn(event_handler(event_sender.clone()));

    // while let Some(res) = set.join_next().await {
    //     info!("{:?}", res);
    // }

    Ok(())
}
