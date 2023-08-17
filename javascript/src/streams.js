const { ethers } = require('ethers');

const { calculateNextBlockBaseFee, estimateNextBlockGas } = require('./utils');

function streamNewBlocks(wssUrl, eventEmitter) {
    const wss = new ethers.WebSocketProvider(wssUrl);

    wss.on('block', async (blockNumber) => {
        let block = await wss.getBlock(blockNumber);
        let nextBaseFee = calculateNextBlockBaseFee(block);
        let estimateGas = await estimateNextBlockGas(); 

        eventEmitter.emit('event', {
            type: 'block',
            blockNumber: block.number,
            baseFee: block.baseFeePerGas,
            nextBaseFee,
            ...estimateGas,
        });
    });
}

function streamPendingTransactions(wssUrl, eventEmitter) {
    const wss = new ethers.WebSocketProvider(wssUrl);
    
    wss.on('pending', async (txHash) => {
        eventEmitter.emit('event', {
            type: 'pendingTx',
            txHash,
        });
    });
}

function streamUniswapV2Events(wssUrl, eventEmitter) {
    // This stream isn't used in the example DEX arb,
    // but is here to demonstrate how to subscribe to events.
    const wss = new ethers.WebSocketProvider(wssUrl);

    const syncEventSelector = ethers.id('Sync(uint112,uint112)');
    const filter = {topics: [syncEventSelector]};

    wss.on(filter, async (event) => {
        eventEmitter.emit('event', event);
    });
}

module.exports = {
    streamNewBlocks,
    streamPendingTransactions,
    streamUniswapV2Events,
};