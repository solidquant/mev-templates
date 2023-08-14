use anyhow::{Ok, Result};
use ethers::{
    abi,
    providers::{Http, Provider},
    types::{H160, H256, U256},
};
use ethers_contract::{Contract, Multicall};
use log::info;
use std::{collections::HashMap, sync::Arc, time::Instant};

use crate::{abi::ABI, pools::Pool};

#[derive(Default, Debug, Clone)]
pub struct Reserve {
    pub reserve0: U256,
    pub reserve1: U256,
}

pub async fn get_uniswap_v2_reserves(
    https_url: String,
    pools: Vec<Pool>,
) -> Result<HashMap<H160, Reserve>> {
    let client = Provider::<Http>::try_from(https_url)?;
    let client = Arc::new(client);

    let abi = ABI::new();
    let mut multicall = Multicall::new(client.clone(), None).await?;

    for pool in &pools {
        let contract = Contract::<Provider<Http>>::new(
            pool.address,
            abi.uniswap_v2_pair.clone(),
            client.clone(),
        );
        let call = contract.method::<_, H256>("getReserves", ())?;
        multicall.add_call(call, false);
    }

    let result = multicall.call_raw().await?;

    let mut reserves = HashMap::new();

    for i in 0..result.len() {
        let pool = &pools[i];
        let reserve = result[i].clone();
        match reserve.unwrap() {
            abi::Token::Tuple(response) => {
                let reserve_data = Reserve {
                    reserve0: response[0].clone().into_uint().unwrap(),
                    reserve1: response[1].clone().into_uint().unwrap(),
                };
                reserves.insert(pool.address.clone(), reserve_data);
            }
            _ => {}
        }
    }

    Ok(reserves)
}

pub async fn batch_get_uniswap_v2_reserves(
    https_url: String,
    pools: Vec<Pool>,
) -> HashMap<H160, Reserve> {
    let start_time = Instant::now();

    let pools_cnt = pools.len();
    let batch = ((pools_cnt / 250) as f32).ceil();
    let pools_per_batch = ((pools_cnt as f32) / batch).ceil() as usize;

    let mut handles = vec![];

    for i in 0..(batch as usize) {
        let start_idx = i * pools_per_batch;
        let end_idx = std::cmp::min(start_idx + pools_per_batch, pools_cnt);
        let handle = tokio::spawn(get_uniswap_v2_reserves(
            https_url.clone(),
            pools[start_idx..end_idx].to_vec(),
        ));
        handles.push(handle);
    }

    let mut reserves: HashMap<H160, Reserve> = HashMap::new();

    for handle in handles {
        let result = handle.await.unwrap();
        reserves.extend(result.unwrap());
    }

    info!(
        "Batch reserves call took: {} seconds",
        start_time.elapsed().as_secs()
    );
    reserves
}
