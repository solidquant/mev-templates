use ethers::types::U256;

pub struct UniswapV2Simulator;

impl UniswapV2Simulator {
    pub fn reserves_to_price(
        reserve0: U256,
        reserve1: U256,
        decimals0: u8,
        decimals1: u8,
        token0_in: bool,
    ) -> f64 {
        let r0 = reserve0.as_u128() as f64;
        let r1 = reserve1.as_u128() as f64;
        let d0 = decimals0 as i32;
        let d1 = decimals1 as i32;
        let mult = (10.0 as f64).powi(d0 - d1);

        if r1 == 0.0 || r0 == 0.0 {
            return 0.0;
        }

        let price = (r1 / r0) * mult;
        if token0_in {
            price
        } else {
            (1 as f64) / price
        }
    }

    pub fn get_amount_out(
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee: U256,
    ) -> Option<U256> {
        let fee = fee / U256::from(100);
        let amount_in_with_fee = amount_in * (U256::from(1000) - fee);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = (reserve_in * 1000) + amount_in_with_fee;
        numerator.checked_div(denominator)
    }
}
