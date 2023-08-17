const { ethers } = require('ethers');
const EventEmitter = require('events');
const v2FactoryABI = require('../abi/UniswapV2Factory.json');

const {
    HTTPS_URL,
    WSS_URL,
} = require('./constants');
const { logger, blacklistTokens } = require('./constants');
const { loadAllPoolsFromV2 } = require('./pools');
const { generateTriangularPaths } = require('./paths');
const { batchGetUniswapV2Reserves } = require('./multi');
const { streamNewBlocks } = require('./streams');
const { getTouchedPoolReserves } = require('./utils');

async function main() {
    const provider = new ethers.JsonRpcProvider(HTTPS_URL);

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

    let s = new Date();
    let reserves = await batchGetUniswapV2Reserves(HTTPS_URL, Object.keys(pools));
    let e = new Date();
    logger.info(`Batch reserves call took: ${(e - s) / 1000} seconds`);
    
    let eventEmitter = new EventEmitter();

    streamNewBlocks(WSS_URL, eventEmitter);
    
    eventEmitter.on('event', async (event) => {
        if (event.type == 'block') {
            let blockNumber = event.blockNumber;
            logger.info(`▪️ New Block #${blockNumber}`);

            let touchedReserves = await getTouchedPoolReserves(provider, blockNumber);
            let touchedPools = [];
            for (let address in touchedReserves) {
                let reserve = touchedReserves[address];
                if (address in reserves) {
                    reserves[address] = reserve;
                    touchedPools.push(address);
                }
            }

            let spreads = {};
            for (let idx = 0; idx < Object.keys(paths).length; idx++) {
                let path = paths[idx];
                let touchedPath = touchedPools.reduce((touched, pool) => {
                    return touched + (path.hasPool(pool) ? 1 : 0)
                }, 0);
                if (touchedPath > 0) {
                    let priceQuote = path.simulateV2Path(1, reserves);
                    let spread = (priceQuote / (10 ** usdcDecimals) - 1) * 100;
                    if (spread > 0) {
                        spreads[idx] = spread;
                    }
                }
            }

            console.log('▶️ Spread over 0%: ', spreads);
        }
    });
}

module.exports = {
    main,
};