/*
ethers-provider-flashbots-bundle
is currently dependent on ethers@5.7.2
make sure to check whether you want to use ethers v5, v6
*/
const { ethers, Wallet } = require('ethers');
const { FlashbotsBundleProvider } = require('@flashbots/ethers-provider-bundle');

const { BOT_ABI } = require('./constants');

const ZERO_ADDRESS = '0x0000000000000000000000000000000000000000'

class Path {
    constructor(router, tokenIn, tokenOut) {
        this.router = router;
        this.tokenIn = tokenIn;
        this.tokenOut = tokenOut;
    }

    toList() {
        return [this.router, this.tokenIn, this.tokenOut];
    }
}

const Flashloan = {
    NotUsed: 0,
    Balancer: 1,
    UniswapV2: 2,
};

class Bundler {
    constructor(
        privateKey,
        signingKey,
        httpsUrl,
        botAddress
    ) {
        this.provider = new ethers.JsonRpcProvider(httpsUrl);
        this.sender = new Wallet(privateKey);
        this.signer = new Wallet(signingKey);
        this.bot = new ethers.Contract(botAddress, BOT_ABI, this.provider);
    }

    async setup() {
        this.flashbots = await FlashbotsBundleProvider.create(
            this.provider,
            this.signer,
        );
    }

    _common_fields() {
        let nonce = this.provider.get_transaction;
    }
}