use anvil::eth::fees::calculate_next_block_base_fee;
use ethers::{
    providers::{Provider, Ws},
    types::{Transaction, U256, U64},
};
use ethers_providers::Middleware;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio_stream::StreamExt;

#[derive(Default, Debug, Clone)]
pub struct NewBlock {
    pub number: U64,
    pub base_fee_per_gas: U256,
    pub next_base_fee: U256,
    pub timestamp: U256,
    pub gas_used: U256,
    pub gas_limit: U256,
}

#[derive(Debug, Clone)]
pub enum Event {
    Block(NewBlock),
    PendingTx(Transaction),
}

pub async fn stream_new_blocks(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let stream = provider.subscribe_blocks().await.unwrap();
    let mut stream = stream.filter_map(|block| match block.number {
        Some(number) => Some(NewBlock {
            number,
            base_fee_per_gas: block.base_fee_per_gas.unwrap_or_default(),
            next_base_fee: U256::from(calculate_next_block_base_fee(
                block.gas_used.as_u64(),
                block.gas_limit.as_u64(),
                block.base_fee_per_gas.unwrap_or_default().as_u64(),
            )),
            timestamp: block.timestamp,
            gas_used: block.gas_used,
            gas_limit: block.gas_limit,
        }),
        None => None,
    });

    while let Some(block) = stream.next().await {
        match event_sender.send(Event::Block(block)) {
            Ok(_) => {}
            Err(_) => {}
        }
    }
}

pub async fn stream_pending_transactions(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let stream = provider.subscribe_pending_txs().await.unwrap();
    let mut stream = stream.transactions_unordered(512).fuse();

    while let Some(result) = stream.next().await {
        match result {
            Ok(tx) => match event_sender.send(Event::PendingTx(tx)) {
                Ok(_) => {}
                Err(_) => {}
            },
            Err(_) => {}
        };
    }
}
