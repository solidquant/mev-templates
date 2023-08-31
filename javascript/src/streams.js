const { ethers } = require('ethers');

const { calculateNextBlockBaseFee, estimateNextBlockGas } = require('./utils');

function streamNewBlocks(wssUrl, eventEmitter) {
    const wss = new ethers.providers.WebSocketProvider(wssUrl);

    wss.on('block', async (blockNumber) => {
        let block = await wss.getBlock(blockNumber);
        let nextBaseFee = calculateNextBlockBaseFee(block);
        let estimateGas = await estimateNextBlockGas(); 

        eventEmitter.emit('event', {
            type: 'block',
            blockNumber: block.number,
            baseFee: BigInt(block.baseFeePerGas),
            nextBaseFee,
            ...estimateGas,
        });
    });

    return wss;
}

function streamPendingTransactions(wssUrl, eventEmitter) {
    const wss = new ethers.providers.WebSocketProvider(wssUrl);
    
    wss.on('pending', async (txHash) => {
        eventEmitter.emit('event', {
            type: 'pendingTx',
            txHash,
        });
    });

    return wss;
}

function streamUniswapV2Events(wssUrl, eventEmitter) {
    // This stream isn't used in the example DEX arb,
    // but is here to demonstrate how to subscribe to events.
    const wss = new ethers.providers.WebSocketProvider(wssUrl);

    const syncEventSelector = ethers.utils.id('Sync(uint112,uint112)');
    const filter = {topics: [syncEventSelector]};

    wss.on(filter, async (event) => {
        eventEmitter.emit('event', event);
    });

    return wss;
}

module.exports = {
    streamNewBlocks,
    streamPendingTransactions,
    streamUniswapV2Events,
};