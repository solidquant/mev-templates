use ethers::types::H160;
use std::str::FromStr;

pub fn get_env(key: &str) -> String {
    std::env::var(key).unwrap()
}

#[derive(Debug, Clone)]
pub struct Env {
    pub https_url: String,
    pub wss_url: String,
    pub private_key: String,
    pub signing_key: String,
    pub bot_address: String,
}

impl Env {
    pub fn new() -> Self {
        Env {
            https_url: get_env("HTTPS_URL"),
            wss_url: get_env("WSS_URL"),
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
