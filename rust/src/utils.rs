use anyhow::Result;
use ethers::{
    self,
    abi::{decode, ParamType, Token},
    providers::{Middleware, Provider, Ws},
    types::{Filter, H160, U256, U64},
};
use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;
use rand::Rng;
use std::{collections::HashMap, sync::Arc};

use crate::multi::Reserve;

pub fn setup_logger() -> Result<()> {
    let colors = ColoredLevelConfig {
        trace: Color::Cyan,
        debug: Color::Magenta,
        info: Color::Green,
        warn: Color::Red,
        error: Color::BrightRed,
        ..ColoredLevelConfig::new()
    };

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Local::now().format("[%H:%M:%S]"),
                colors.color(record.level()),
                message
            ))
        })
        .chain(std::io::stdout())
        .level(log::LevelFilter::Error)
        .level_for("rust", LevelFilter::Info)
        .apply()?;

    Ok(())
}

pub fn calculate_next_block_base_fee(
    gas_used: U256,
    gas_limit: U256,
    base_fee_per_gas: U256,
) -> U256 {
    let gas_used = gas_used;

    let mut target_gas_used = gas_limit / 2;
    target_gas_used = if target_gas_used == U256::zero() {
        U256::one()
    } else {
        target_gas_used
    };

    let new_base_fee = {
        if gas_used > target_gas_used {
            base_fee_per_gas
                + ((base_fee_per_gas * (gas_used - target_gas_used)) / target_gas_used)
                    / U256::from(8u64)
        } else {
            base_fee_per_gas
                - ((base_fee_per_gas * (target_gas_used - gas_used)) / target_gas_used)
                    / U256::from(8u64)
        }
    };

    let seed = rand::thread_rng().gen_range(0..9);
    new_base_fee + seed
}

pub async fn get_touched_pool_reserves(
    provider: Arc<Provider<Ws>>,
    block_number: U64,
) -> Result<HashMap<H160, Reserve>> {
    let sync_event = "Sync(uint112,uint112)";
    let event_filter = Filter::new()
        .from_block(block_number)
        .to_block(block_number)
        .event(sync_event);

    let logs = provider.get_logs(&event_filter).await?;

    let mut tx_idx = HashMap::new();
    let mut reserves = HashMap::new();

    for log in &logs {
        let decoded = decode(&[ParamType::Uint(256), ParamType::Uint(256)], &log.data);
        match decoded {
            Ok(data) => {
                let idx = log.transaction_index.unwrap_or_default();
                let prev_tx_idx = tx_idx.get(&log.address);
                let update = (*prev_tx_idx.unwrap_or(&U64::zero())) <= idx;

                if update {
                    let reserve0 = match data[0] {
                        Token::Uint(rs) => rs,
                        _ => U256::zero(),
                    };
                    let reserve1 = match data[1] {
                        Token::Uint(rs) => rs,
                        _ => U256::zero(),
                    };
                    let reserve = Reserve { reserve0, reserve1 };

                    reserves.insert(log.address, reserve);
                    tx_idx.insert(log.address, idx);
                }
            }
            Err(_) => {}
        }
    }

    Ok(reserves)
}
