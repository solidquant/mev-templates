use anyhow::{Ok, Result};
use cfmms::{
    dex::{Dex, DexVariant as CfmmsDexVariant},
    pool::Pool as CfmmsPool,
    sync::sync_pairs,
};
use csv::StringRecord;
use ethers::{
    providers::{Provider, Ws},
    types::H160,
};
use log::info;
use std::{path::Path, str::FromStr, sync::Arc};

#[derive(Debug, Clone)]
pub enum DexVariant {
    UniswapV2,
    UniswapV3,
}

#[derive(Debug, Clone)]
pub struct Pool {
    pub address: H160,
    pub version: DexVariant,
    pub token0: H160,
    pub token1: H160,
    pub decimals0: u8,
    pub decimals1: u8,
    pub fee: u32,
}

impl From<StringRecord> for Pool {
    fn from(record: StringRecord) -> Self {
        let version = if record.get(1).unwrap() == "2" {
            DexVariant::UniswapV2
        } else {
            DexVariant::UniswapV3
        };
        Self {
            address: H160::from_str(record.get(0).unwrap()).unwrap(),
            version,
            token0: H160::from_str(record.get(2).unwrap()).unwrap(),
            token1: H160::from_str(record.get(3).unwrap()).unwrap(),
            decimals0: record.get(4).unwrap().parse().unwrap(),
            decimals1: record.get(5).unwrap().parse().unwrap(),
            fee: record.get(6).unwrap().parse().unwrap(),
        }
    }
}

impl Pool {
    pub fn cache_row(&self) -> (String, i32, String, String, u8, u8, u32) {
        (
            format!("{:?}", self.address),
            match self.version {
                DexVariant::UniswapV2 => 2,
                DexVariant::UniswapV3 => 3,
            },
            format!("{:?}", self.token0),
            format!("{:?}", self.token1),
            self.decimals0,
            self.decimals1,
            self.fee,
        )
    }
}

pub async fn load_all_pools_from_v2(
    wss_url: String,
    factory_addresses: Vec<&str>,
    from_blocks: Vec<u64>,
) -> Result<Vec<Pool>> {
    // Load from cached file if the file exists
    let file_path = Path::new("src/.cached-pools.csv");
    if file_path.exists() {
        let mut reader = csv::Reader::from_path(file_path)?;

        let mut pools_vec: Vec<Pool> = Vec::new();
        for row in reader.records() {
            let row = row.unwrap();
            let pool = Pool::from(row);
            pools_vec.push(pool);
        }
        return Ok(pools_vec);
    }

    let ws = Ws::connect(wss_url).await?;
    let provider = Arc::new(Provider::new(ws));

    let mut dexes_data = Vec::new();

    for i in 0..factory_addresses.len() {
        dexes_data.push((
            factory_addresses[i].clone(),
            CfmmsDexVariant::UniswapV2,
            from_blocks[i],
        ))
    }

    let dexes: Vec<_> = dexes_data
        .into_iter()
        .map(|(address, variant, number)| {
            Dex::new(
                H160::from_str(&address).unwrap(),
                variant,
                number,
                Some(3000),
            )
        })
        .collect();

    let pools_vec: Vec<CfmmsPool> = sync_pairs(dexes.clone(), provider.clone(), None).await?;
    let pools_vec: Vec<Pool> = pools_vec
        .into_iter()
        .map(|pool| match pool {
            CfmmsPool::UniswapV2(pool) => Pool {
                address: pool.address,
                version: DexVariant::UniswapV2,
                token0: pool.token_a,
                token1: pool.token_b,
                decimals0: pool.token_a_decimals,
                decimals1: pool.token_b_decimals,
                fee: pool.fee,
            },
            CfmmsPool::UniswapV3(pool) => Pool {
                address: pool.address,
                version: DexVariant::UniswapV3,
                token0: pool.token_a,
                token1: pool.token_b,
                decimals0: pool.token_a_decimals,
                decimals1: pool.token_b_decimals,
                fee: pool.fee,
            },
        })
        .collect();
    info!("Synced to {} pools", pools_vec.len());

    let mut writer = csv::Writer::from_path(file_path)?;
    writer.write_record(&[
        "address",
        "version",
        "token0",
        "token1",
        "decimals0",
        "decimals1",
        "fee",
    ])?;

    for pool in &pools_vec {
        writer.serialize(pool.cache_row())?;
    }
    writer.flush()?;

    Ok(pools_vec)
}
