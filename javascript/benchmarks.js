const { ethers } = require('ethers');
const microtime = require('microtime');
const EventEmitter = require('events');

const {
    HTTPS_URL,
    WSS_URL,
    PRIVATE_KEY,
    SIGNING_KEY,
    BOT_ADDRESS,
} = require('./src/constants');
const { loadAllPoolsFromV2 } = require('./src/pools');
const { streamNewBlocks, streamPendingTransactions } = require('./src/streams');

function loggingEventHandler(eventEmitter) {
    eventEmitter.on('event', async (event) => {
        console.log(event);
    });
}

async function benchmarkStreams(streamFunc, handlerFunc, runTime) {
    let eventEmitter = new EventEmitter();

    const wss = await streamFunc(WSS_URL, eventEmitter);
    await handlerFunc(eventEmitter);

    setTimeout(async () => {
        await wss.destroy();
    }, runTime * 1000);
}

async function benchmarkFunction() {
    let s, took;

    // 1. Create HTTP provider
    s = microtime.now();
    const provider = new ethers.providers.JsonRpcProvider(HTTPS_URL);
    took = microtime.now() - s;
    console.log(`1. HTTP provider created | Took: ${took} microsec`);
    
    // 2. Get block info
    for (let i = 0; i < 10; i++) {
        s = microtime.now();
        let block = await provider.getBlock('latest');
        took = (microtime.now() - s) / 1000;
        console.log(`2. New block: #${block.number} | Took: ${took} ms`);
    }

    // Common variables used throughout
    const factoryAddresses = ['0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac'];
    const factoryBlocks = [10794229];
    const usdcAddress = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
    const usdcDecimals = 6;

    // 3. Retrieving cached pools data
    s = microtime.now();
    let pools = await loadAllPoolsFromV2(HTTPS_URL, factoryAddresses, factoryBlocks, 2000);
    took = (microtime.now() - s) / 1000;
    console.log(`3. Cached ${Object.keys(pools).length} pools data | Took: ${took} ms`);

    let streamFunc;
    let handlerFunc;

    // 6. Pending transaction async stream
    streamFunc = streamNewBlocks;
    handlerFunc = loggingEventHandler;
    await benchmarkStreams(streamFunc, handlerFunc, 10);
}

(async () => {
    await benchmarkFunction();
})();