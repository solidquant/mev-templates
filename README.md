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

---

## Benchmarks

It's always difficult to pick a programming language just right for the task, and often times, it shouldn't really matter what language you use. We should take many factors into account and decide on the one that you feel most comfortable with.

Among these factors, we should consider how fast each language is - in an environment that's similar to how most people would use the language, not in a super optimized way, because we don't normally hyper-optimize our codes.

For this reason, I've run few benchmarks of the three templates to see how fast each can go.

---

You can find more about this project in my blog post:

[👉 MEV templates written in Python, Javascript, and Rust](https://medium.com/@solidquant/mev-templates-written-in-python-javascript-and-rust-ddd3d324d709)

⚡️ Plus, come join our Discord community to take this journey together. We’re actively reviewing the code used in these blog posts to guarantee safe usage by all our members. Though still in its infancy, we’re slowly growing and collaborating on research/projects in the 💫 MEV space 🏄‍♀️🏄‍♂️:

[👨‍👩‍👦‍👦 Join the Solid Quant Discord Server!](https://discord.com/invite/e6KpjTQP98)

Also, for people that want to reach out to me, they can e-mail me directly at: solidquant@gmail.com