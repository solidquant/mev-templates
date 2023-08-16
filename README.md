# MEV templates written in py/js/rs

You can find three MEV templates written in Python/Javascript/Rust here.

The three templates follow a similar design pattern and are written with readability in mind. With the components introduced here, you can easily reproduce most of the MEV strategies known to people: sandwich, frontrunning, arbitrage, sniping, so on and so forth.

The templates all include an example DEX flashloan arbitrage strategy to demonstrate how the template can be used. It is a simple demonstration and will need some tweaking to make it work (mostly in regards to order size optimization and gas bidding strategy), though it will work as a real DEX arbitrage bot by doing:

- Retrieving all historical events from the blockchain (PairCreated).

- Create a triangular arbitrage path with all the pools retrieved from above.

- Perform a multicall request of "getReserve" calls to all the pools we're trading (1 ~ 3 second to retrieve >=6000 pools).

- Stream new headers, new pending transactions, events asynchronously.

- Simulate Uniswap V2 3-hop paths offline.

- Sign transactions and create bundles to send to Flashbots (also supports sending transactions to the mempool).

**NOTE**: the template is still under development. The Python version is the only completed project. I'm currently working on the Rust version, after which I'll work on the JS version using ethers.js version 6.