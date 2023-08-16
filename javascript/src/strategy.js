const ethers = require('ethers');
const v2FactoryABI = require('../abi/UniswapV2Factory.json');

const {
    HTTPS_URL,
} = require('./constants');
const { logger, blacklistTokens } = require('./constants');
const { loadAllPoolsFromV2 } = require('./pools');
const { generateTriangularPaths } = require('./paths');

async function main() {
    const factoryAddresses = ['0xc35DADB65012eC5796536bD9864eD8773aBc74C4'];
    const factoryBlocks = [11333218];

    let pools = await loadAllPoolsFromV2(
        HTTPS_URL, factoryAddresses, factoryBlocks, 50000
    );
    logger.info(`Initial pool count: ${Object.keys(pools).length}`);

    const usdcAddress = '0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174';
    const usdcDecimals = 6;

    let paths = generateTriangularPaths(pools, usdcAddress);

    // Filter pools that were used in arb paths
    pools = {};
    for (let path of paths) {
        if (!path.shouldBlacklist(blacklistTokens)) {
            pools[path.pool1.address] = path.pool1;
            pools[path.pool2.address] = path.pool2;
            pools[path.pool3.address] = path.pool3;
        }
    }
    logger.info(`New pool count: ${Object.keys(pools).length}`);
}

module.exports = {
    main,
};