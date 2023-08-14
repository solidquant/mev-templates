use ethers_core::abi::Abi;
use std::fs;

pub struct ABI {
    pub erc20: Abi,
    pub weth: Abi,
    pub uniswap_v2_factory: Abi,
    pub uniswap_v2_pair: Abi,
    pub v2_arb_bot: Abi,
}

impl ABI {
    pub fn new() -> Self {
        let erc20_json = fs::read_to_string("src/abi/ERC20.json").unwrap();
        let weth_json = fs::read_to_string("src/abi/WETH.json").unwrap();
        let uniswap_v2_factory_json = fs::read_to_string("src/abi/UniswapV2Factory.json").unwrap();
        let uniswap_v2_pair_json = fs::read_to_string("src/abi/UniswapV2Pair.json").unwrap();
        let v2_arb_bot_json = fs::read_to_string("src/abi/V2ArbBot.json").unwrap();
        Self {
            erc20: serde_json::from_str(&erc20_json).unwrap(),
            weth: serde_json::from_str(&weth_json).unwrap(),
            uniswap_v2_factory: serde_json::from_str(&uniswap_v2_factory_json).unwrap(),
            uniswap_v2_pair: serde_json::from_str(&uniswap_v2_pair_json).unwrap(),
            v2_arb_bot: serde_json::from_str(&v2_arb_bot_json).unwrap(),
        }
    }
}
