use ethers::{
    prelude::Lazy,
    types::{Address, H160, U256, U64},
};
use std::str::FromStr;

pub static WEI: Lazy<U256> = Lazy::new(|| U256::from(10).pow(U256::from(18)));
pub static GWEI: Lazy<U256> = Lazy::new(|| U256::from(10).pow(U256::from(9)));

pub static ZERO_ADDRESS: Lazy<Address> =
    Lazy::new(|| Address::from_str("0x0000000000000000000000000000000000000000").unwrap());

pub fn get_env(key: &str) -> String {
    std::env::var(key).unwrap()
}

#[derive(Debug, Clone)]
pub struct Env {
    pub https_url: String,
    pub wss_url: String,
    pub chain_id: U64,
    pub private_key: String,
    pub signing_key: String,
    pub bot_address: String,
}

impl Env {
    pub fn new() -> Self {
        Env {
            https_url: get_env("HTTPS_URL"),
            wss_url: get_env("WSS_URL"),
            chain_id: U64::from_str(&get_env("CHAIN_ID")).unwrap(),
            private_key: get_env("PRIVATE_KEY"),
            signing_key: get_env("SIGNING_KEY"),
            bot_address: get_env("BOT_ADDRESS"),
        }
    }
}

pub fn get_blacklist_tokens() -> Vec<H160> {
    vec!["0x9469603F3Efbcf17e4A5868d81C701BDbD222555"]
        .into_iter()
        .map(|addr| H160::from_str(addr).unwrap())
        .collect()
}

// Use later for broadcasting to multiple builders
// static BUILDER_URLS: &[&str] = &[
//     "https://builder0x69.io",
//     "https://rpc.beaverbuild.org",
//     "https://relay.flashbots.net",
//     "https://rsync-builder.xyz",
//     "https://rpc.titanbuilder.xyz",
//     "https://api.blocknative.com/v1/auction",
//     "https://mev.api.blxrbdn.com",
//     "https://eth-builder.com",
//     "https://builder.gmbit.co/rpc",
//     "https://buildai.net",
//     "https://rpc.payload.de",
//     "https://rpc.lightspeedbuilder.info",
//     "https://rpc.nfactorial.xyz",
// ];
