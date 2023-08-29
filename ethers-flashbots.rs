use anvil::{Anvil, AnvilApp};
use ethers::prelude::*;
use ethers_flashbots::*;
use url::Url;

#[tokio::main]  
async fn main() -> eyre::Result<()> {

  // Use Anvil local node
  let anvil = Anvil::new().spawn();
  let provider = Provider::<AnvilApp>::try_from(anvil)?;

  // Bundle multiple transactions
  let mut bundle = BundleRequest::new();

  let wallet = // Gnosis Safe multisig wallet

  // Get block number in parallel
  let block_number = provider.get_block_number().await?;

  let txs = vec![
    // Fill 2 transactions in parallel 
    let tx1 = fill_transaction(provider.clone(), wallet.clone(), to, value).await?;
    let tx2 = fill_transaction(provider, wallet, to2, value2).await?;

    // Sign them in parallel
    let sig1 = wallet.sign(tx1).await?;
    let sig2 = wallet.sign(tx2).await?;
  ];

  // Add both to bundle
  bundle.push_transaction(tx1.rlp_signed(&sig1));
  bundle.push_transaction(tx2.rlp_signed(&sig2));
  
  // Set bundle details
  bundle.set_block(block_number + 1);

  // Send bundle
  let miner = FlashbotsRPC::new("https://rpc.flashbots.net");
  miner.send_bundle(&bundle).await?;

  Ok(())
}

async fn fill_transaction(
  provider: Provider<AnvilApp>,
  wallet: Wallet,
  to: Address,
  value: U256
) -> Result<TypedTransaction> {
  let mut tx = TransactionRequest::new()
    .to(to)
    .value(value)
    .gas_price(100 * GWEI); // Use high gas price

  // High slippage tolerance
  provider.fill_transaction(&mut tx, Some(50)).await?;

  Ok(tx)
}
