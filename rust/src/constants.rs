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
            https_url: std::env::var("HTTPS_URL").unwrap(),
            wss_url: std::env::var("WSS_URL").unwrap(),
            private_key: std::env::var("PRIVATE_KEY").unwrap(),
            signing_key: std::env::var("SIGNING_KEY").unwrap(),
            bot_address: std::env::var("BOT_ADDRESS").unwrap(),
        }
    }
}
