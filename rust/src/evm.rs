use anyhow::{anyhow, Result};
use colored::Colorize;
use ethers::{
    abi::{self, parse_abi},
    contract::BaseContract,
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Ws},
    signers::{LocalWallet, Signer},
    types::{Address, Bytes, Transaction, H160, U256, U64},
};
use foundry_common::get_http_provider;
use foundry_common::provider::RetryProvider;
use foundry_evm::{
    executor::{
        fork::{BlockchainDb, BlockchainDbMeta, SharedBackend},
        ExecutionResult, Output, TransactTo,
    },
    revm::{
        db::CacheDB,
        primitives::{keccak256, AccountInfo, Address as rAddress, Bytecode, U256 as rU256},
        EVM,
    },
};
use log::{error, info};
use std::{collections::BTreeSet, sync::Arc};

use crate::bundler::{order_tx_calldata, Flashloan, PathParam};
use crate::constants::{Env, BOT_CODE};
use crate::paths::ArbPath;
use crate::streams::NewBlock;

pub struct EvmSimulator {
    pub env: &'static Env,
    pub provider: Arc<RetryProvider>,
}

impl EvmSimulator {
    pub fn new(env: &'static Env) -> Self {
        let provider = get_http_provider(&env.https_url);
        let client = Arc::new(provider);
        Self {
            env,
            provider: client.clone(),
        }
    }

    pub async fn simulate_swap_v2(
        &self,
        new_block: &NewBlock,
        amount_in: U256,
        pair: &str,
        token_in: &str,
        token_out: &str,
    ) {
        let shared_backend = SharedBackend::spawn_backend_thread(
            self.provider.clone(),
            BlockchainDb::new(
                BlockchainDbMeta {
                    cfg_env: Default::default(),
                    block_env: Default::default(),
                    hosts: BTreeSet::from(["".to_string()]),
                },
                None,
            ),
            Some(new_block.number.into()),
        );
    }

    pub async fn simulate_weth_arbitrage(
        &self,
        new_block: &NewBlock,
        weth_address: H160,
        amount_in: U256,
        calldata: Bytes,
    ) -> Result<(U256, u64)> {
        // WETH in and WETH out
        let shared_backend = SharedBackend::spawn_backend_thread(
            self.provider.clone(),
            BlockchainDb::new(
                BlockchainDbMeta {
                    cfg_env: Default::default(),
                    block_env: Default::default(),
                    hosts: BTreeSet::from(["".to_string()]),
                },
                None,
            ),
            Some(new_block.number.into()),
        );

        let mut fork_db = CacheDB::new(shared_backend);
        let wei = rU256::from(10).pow(rU256::from(18));

        // Initialize bot contract
        let bot_address = self.env.bot_address.parse::<rAddress>().unwrap();
        let bot_acc_info =
            AccountInfo::new(rU256::ZERO, 0, Bytecode::new_raw((*BOT_CODE.0).into()));
        fork_db.insert_account_info(bot_address, bot_acc_info);

        // Create owner account with some ETH balance (to pay for gas)
        let owner = self.env.private_key.parse::<LocalWallet>().unwrap();
        let owner_acc_info = AccountInfo::new(
            rU256::from(100).checked_mul(wei).unwrap(),
            0,
            Bytecode::default(),
        );
        fork_db.insert_account_info(owner.address().into(), owner_acc_info);

        // Override the value of balanceOf in WETH contract, storage slot: 3
        // Also, give the bot contract enough WETH balance to play around with: the same as amount_in
        let weth_balance_slot = keccak256(&abi::encode(&[
            abi::Token::Address(bot_address.into()),
            abi::Token::Uint(U256::from(3)),
        ]));
        fork_db.insert_account_storage(
            weth_address.into(),
            weth_balance_slot.into(),
            rU256::from(amount_in.as_u128()),
        )?;

        // Initialize EVM using the forked DB
        let mut evm = EVM::new();
        evm.database(fork_db);

        let builder69 = "0x690B9A9E9aa1C9dB991C7721a92d351Db4FaC990"
            .parse::<rAddress>()
            .unwrap();

        evm.env.block.number = rU256::from(new_block.number.as_u64());
        evm.env.block.timestamp = new_block.timestamp.into();
        evm.env.block.basefee = new_block.base_fee_per_gas.into();
        evm.env.block.coinbase = builder69;

        evm.env.tx.caller = owner.address().into();
        evm.env.tx.transact_to = TransactTo::Call(bot_address);
        evm.env.tx.data = calldata.0;
        evm.env.tx.gas_limit = 700000;
        evm.env.tx.gas_price = new_block.base_fee_per_gas.into();
        evm.env.tx.value = rU256::ZERO;

        let result = match evm.transact_commit() {
            Ok(result) => result,
            Err(e) => return Err(anyhow!("EVM ERROR: {:?}", e)),
        };
        let gas_used = match result {
            ExecutionResult::Success {
                output, gas_used, ..
            } => match output {
                Output::Call(_) => gas_used,
                Output::Create(_, _) => gas_used,
            },
            ExecutionResult::Revert { output, .. } => {
                return Err(anyhow!("EVM REVERT: {:?}", output))
            }
            ExecutionResult::Halt { reason, .. } => return Err(anyhow!("EVM HALT: {:?}", reason)),
        };

        // Get the final balance of WETH
        let weth_abi = BaseContract::from(parse_abi(&[
            "function balanceOf(address owner) external view returns (uint256 balance)",
        ])?);
        let encoded = weth_abi
            .encode(
                "balanceOf",
                (self.env.bot_address.parse::<Address>().unwrap()),
            )
            .unwrap();

        evm.env.tx.caller = owner.address().into();
        evm.env.tx.transact_to = TransactTo::Call(weth_address.into());
        evm.env.tx.data = encoded.0;
        evm.env.tx.value = rU256::ZERO;

        let ref_tx = evm
            .transact_ref()
            .map_err(|e| anyhow!("balanceOf error: {:?}", e))?;
        let result = ref_tx.result;

        let value = match result {
            ExecutionResult::Success {
                output: Output::Call(value),
                ..
            } => value,
            result => return Err(anyhow!("balanceOf error: {:?}", result)),
        };
        let balance: U256 = weth_abi.decode_output("balanceOf", value)?;

        Ok((balance, gas_used))
    }
}
