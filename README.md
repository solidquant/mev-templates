# MEV templates written in Python, Javascript, and Rust

You can find three MEV templates written in Python/Javascript/Rust here.

The three templates follow a similar design pattern and are written with readability in mind. With the components introduced here, you can easily reproduce most of the MEV strategies known to people: sandwich, frontrunning, arbitrage, sniping, so on and so forth.

The templates all include an example **DEX flashloan arbitrage strategy** to demonstrate how the template can be used. It is a simple demonstration and will need some tweaking to make it work (mostly in regards to order size optimization and gas bidding strategy), though it will work as a real DEX arbitrage bot by doing:

- Retrieving all historical events from the blockchain (PairCreated).

- Create a triangular arbitrage path with all the pools retrieved from above.

- Perform a multicall request of "getReserve" calls to all the pools we're trading (1 ~ 3 second to retrieve >=6000 pools).

- Stream new headers, new pending transactions, events asynchronously.

- Simulate Uniswap V2 3-hop paths offline.

- Sign transactions and create bundles to send to Flashbots (also supports sending transactions to the mempool).


## What is this?

![Professor Oak](https://github.com/solidquant/mev-templates/assets/134243834/553560de-3334-4d4b-a447-14aa91ad28de)

> (Professor Oak) *Good. So you are here.*

In this Github repository, you can pick one of the most popular languages to use in your MEV project. By studying this project, you'll get a feel for how MEV strategies are built.

Most strategies share a common code base, and this repository is an attempt to include the basic tooling required for all level of traders to have in their pockets.