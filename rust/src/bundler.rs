use anyhow::{anyhow, Result};
use ethers::prelude::*;
use ethers::types::{
    transaction::{eip2718::TypedTransaction, eip2930::AccessList},
    Address, Eip1559TransactionRequest, U256,
};
use ethers::{
    abi,
    middleware::MiddlewareBuilder,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
};
use ethers_flashbots::*;
use std::{str::FromStr, sync::Arc};
use url::Url;

use crate::constants::Env;

abigen!(
    ArbBot,
    r#"[
        function recoverToken(address token) external;
        function approveRouter(address router, address[] memory tokens, bool force) external;
    ]"#,
);

#[derive(Debug, Clone)]
pub struct PathParam {
    pub router: Address,
    pub token_in: Address,
    pub token_out: Address,
}

impl PathParam {
    pub fn make_params(&self) -> Vec<abi::Token> {
        vec![
            abi::Token::Address(self.router.into()),
            abi::Token::Address(self.token_in.into()),
            abi::Token::Address(self.token_out.into()),
        ]
    }
}

#[derive(Debug, Clone)]
pub enum Flashloan {
    NotUsed = 0,
    Balancer = 1,
    UniswapV2 = 2,
}

type SignerProvider = SignerMiddleware<Provider<Http>, LocalWallet>;

pub struct Bundler {
    pub env: Env,
    pub sender: LocalWallet,
    pub bot: ArbBot<SignerProvider>,
    pub provider: SignerProvider,
    pub flashbots: SignerMiddleware<FlashbotsMiddleware<SignerProvider, LocalWallet>, LocalWallet>,
}

impl Bundler {
    pub fn new<'a>() -> Self {
        let env = Env::new();

        let sender = env
            .private_key
            .parse::<LocalWallet>()
            .unwrap()
            .with_chain_id(env.chain_id.as_u64());
        let signer = env
            .signing_key
            .parse::<LocalWallet>()
            .unwrap()
            .with_chain_id(env.chain_id.as_u64());

        let provider = Provider::<Http>::try_from(&env.https_url)
            .unwrap()
            .with_signer(sender.clone());

        let flashbots = SignerMiddleware::new(
            FlashbotsMiddleware::new(
                provider.clone(),
                Url::parse("https://relay.flashbots.net").unwrap(),
                signer,
            ),
            sender.clone(),
        );

        let client = Arc::new(provider.clone());
        let bot = ArbBot::new(env.bot_address.parse::<Address>().unwrap(), client.clone());

        Self {
            env,
            sender,
            bot,
            provider: provider,
            flashbots: flashbots,
        }
    }

    pub async fn _common_fields(&self) -> Result<(H160, U256, U64)> {
        let nonce = self
            .provider
            .get_transaction_count(self.sender.address(), None)
            .await?;
        Ok((self.sender.address(), U256::from(nonce), self.env.chain_id))
    }

    pub async fn sign_tx(&self, tx: Eip1559TransactionRequest) -> Result<Bytes> {
        let typed = TypedTransaction::Eip1559(tx);
        let signature = self.sender.sign_transaction(&typed).await?;
        let signed = typed.rlp_signed(&signature);
        Ok(signed)
    }

    pub fn to_bundle<T: Into<BundleTransaction>>(
        &self,
        signed_txs: Vec<T>,
        block_number: U64,
    ) -> BundleRequest {
        let mut bundle = BundleRequest::new();

        for tx in signed_txs {
            let bundle_tx: BundleTransaction = tx.into();
            bundle = bundle.push_transaction(bundle_tx);
        }

        bundle
            .set_block(block_number + 1)
            .set_simulation_block(block_number)
            .set_simulation_timestamp(0)
    }

    pub async fn send_bundle(&self, bundle: BundleRequest) -> Result<TxHash> {
        let simulated = self.flashbots.inner().simulate_bundle(&bundle).await?;

        for tx in &simulated.transactions {
            if let Some(e) = &tx.error {
                return Err(anyhow!("Simulation error: {:?}", e));
            }
            if let Some(r) = &tx.revert {
                return Err(anyhow!("Simulation revert: {:?}", r));
            }
        }

        let pending_bundle = self.flashbots.inner().send_bundle(&bundle).await?;
        let bundle_hash = pending_bundle.await?;
        Ok(bundle_hash)
    }

    pub async fn send_tx(&self, tx: Eip1559TransactionRequest) -> Result<TxHash> {
        let pending_tx = self.provider.send_transaction(tx, None).await?;
        let receipt = pending_tx.await?.ok_or_else(|| anyhow!("Tx dropped"))?;
        Ok(receipt.transaction_hash)
    }

    pub async fn transfer_in_tx(
        &self,
        amount_in: U256,
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
    ) -> Result<Eip1559TransactionRequest> {
        let common = self._common_fields().await?;
        let to = NameOrAddress::Address(H160::from_str(&self.env.bot_address).unwrap());
        Ok(Eip1559TransactionRequest {
            to: Some(to),
            from: Some(common.0),
            data: Some(Bytes(bytes::Bytes::new())),
            value: Some(amount_in),
            chain_id: Some(common.2),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            max_fee_per_gas: Some(max_fee_per_gas),
            gas: Some(U256::from(60000)),
            nonce: Some(common.1),
            access_list: AccessList::default(),
        })
    }

    pub async fn transfer_out_tx(
        &self,
        token: &str,
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
    ) -> Result<Eip1559TransactionRequest> {
        let token_address = Address::from_str(token).unwrap();
        let calldata = self.bot.encode("recoverToken", (token_address,))?;

        let common = self._common_fields().await?;
        let to = NameOrAddress::Address(H160::from_str(&self.env.bot_address).unwrap());
        Ok(Eip1559TransactionRequest {
            to: Some(to),
            from: Some(common.0),
            data: Some(calldata),
            value: Some(U256::zero()),
            chain_id: Some(common.2),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            max_fee_per_gas: Some(max_fee_per_gas),
            gas: Some(U256::from(50000)),
            nonce: Some(common.1),
            access_list: AccessList::default(),
        })
    }

    pub async fn approve_tx(
        &self,
        router: &str,
        tokens: Vec<&str>,
        force: bool,
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
    ) -> Result<Eip1559TransactionRequest> {
        let router_address = Address::from_str(router).unwrap();
        let token_addresses: Vec<Address> = tokens
            .iter()
            .map(|token| Address::from_str(token).unwrap())
            .collect();
        let calldata = self
            .bot
            .encode("approveRouter", (router_address, token_addresses, force))?;

        let token_cnt = tokens.len();
        let common = self._common_fields().await?;
        let to = NameOrAddress::Address(H160::from_str(&self.env.bot_address).unwrap());
        Ok(Eip1559TransactionRequest {
            to: Some(to),
            from: Some(common.0),
            data: Some(calldata),
            value: Some(U256::zero()),
            chain_id: Some(common.2),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            max_fee_per_gas: Some(max_fee_per_gas),
            gas: Some(U256::from(55000) * U256::from(token_cnt)),
            nonce: Some(common.1),
            access_list: AccessList::default(),
        })
    }

    pub async fn order_tx(
        &self,
        paths: Vec<PathParam>,
        amount_in: U256,
        flashloan: Flashloan,
        loan_from: Address,
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
    ) -> Result<Eip1559TransactionRequest> {
        let nhop = paths.len();

        let mut params = Vec::new();
        params.extend(vec![
            abi::Token::Uint(amount_in),
            abi::Token::Uint(U256::from(flashloan as u64)),
            abi::Token::Address(loan_from),
        ]);

        for i in 0..nhop {
            params.extend(paths[i].make_params());
        }

        let encoded = abi::encode(&params);
        let calldata = Bytes::from(encoded);

        let common = self._common_fields().await?;
        let to = NameOrAddress::Address(H160::from_str(&self.env.bot_address).unwrap());
        Ok(Eip1559TransactionRequest {
            to: Some(to),
            from: Some(common.0),
            data: Some(calldata),
            value: Some(U256::zero()),
            chain_id: Some(common.2),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            max_fee_per_gas: Some(max_fee_per_gas),
            gas: Some(U256::from(600000)),
            nonce: Some(common.1),
            access_list: AccessList::default(),
        })
    }
}

#[cfg(test)]
mod bundler_tests {
    use super::*;
    use crate::constants::{GWEI, WEI};

    #[tokio::test]
    async fn bundler_test() {
        let bundler = Bundler::new();

        let tx = bundler
            .transfer_in_tx(
                U256::from(5) * *WEI,
                U256::from(50) * *GWEI,
                U256::from(200) * *GWEI,
            )
            .await
            .unwrap();
        // let tx_hash = bundler.send_tx(tx).await?;
        // println!("{:?}", tx_hash);

        let tx = bundler
            .transfer_out_tx(
                "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270",
                U256::from(50) * *GWEI,
                U256::from(200) * *GWEI,
            )
            .await
            .unwrap();
        // let tx_hash = bundler.send_tx(tx).await?;
        // println!("{:?}", tx_hash);

        let tx = bundler
            .approve_tx(
                "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",
                vec!["0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270"],
                true,
                U256::from(50) * *GWEI,
                U256::from(200) * *GWEI,
            )
            .await
            .unwrap();
        // let tx_hash = bundler.send_tx(tx).await?;
        // println!("{:?}", tx_hash);

        let paths = vec![PathParam {
            router: Address::from_str("0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506").unwrap(),
            token_in: Address::from_str("0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270").unwrap(),
            token_out: Address::from_str("0xc2132D05D31c914a87C6611C10748AEb04B58e8F").unwrap(),
        }];
        let tx = bundler
            .order_tx(
                paths,
                U256::from(1) * *WEI,
                Flashloan::Balancer,
                Address::from_str("0xBA12222222228d8Ba445958a75a0704d566BF2C8").unwrap(),
                U256::from(100) * *GWEI,
                U256::from(300) * *GWEI,
            )
            .await
            .unwrap();
        // let tx_hash = bundler.send_tx(tx).await?;
        // println!("{:?}", tx_hash);
    }
}
