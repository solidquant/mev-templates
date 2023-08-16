const ethers = require('ethers');
const v2FactoryABI = require('./abi/UniswapV2Factory.json');

const {
    HTTPS_URL,
} = require('./src/constants');
const { loadAllPoolsFromV2 } = require('./src/pools');

async function main() {
    const factoryAddresses = ['0xc35DADB65012eC5796536bD9864eD8773aBc74C4'];
    const factoryBlocks = [11333218];

    await loadAllPoolsFromV2(
        HTTPS_URL, factoryAddresses, factoryBlocks, 50000
    );
}

(async () => {
    await main();
})();