const { ethers } = require('ethers');
const axios = require('axios');

const { 
    BLOCKNATIVE_TOKEN,
    CHAIN_ID,
} = require('./constants');

const calculateNextBlockBaseFee = (block) => {
    let baseFee = BigInt(block.baseFeePerGas);
    let gasUsed = BigInt(block.gasUsed);
    let gasLimit = BigInt(block.gasLimit);

    let targetGasUsed = gasLimit / BigInt(2);
    targetGasUsed = targetGasUsed == BigInt(0) ? BigInt(1) : targetGasUsed;

    let newBaseFee;

    if (gasUsed > targetGasUsed) {
        newBaseFee = baseFee + ((baseFee * (gasUsed - targetGasUsed)) / targetGasUsed) / BigInt(8);
    } else {
        newBaseFee = baseFee - ((baseFee * (targetGasUsed - gasUsed)) / targetGasUsed) / BigInt(8);
    }

    const rand = BigInt(Math.floor(Math.random() * 10));
    return newBaseFee + rand;
};

async function estimateNextBlockGas() {
    let estimate = {};
    if (!BLOCKNATIVE_TOKEN || ![1, 137].includes(parseInt(CHAIN_ID))) return estimate;
    const url = `https://api.blocknative.com/gasprices/blockprices?chainid=${CHAIN_ID}`;
    const response = await axios.get(url, {
        headers: { Authorization: BLOCKNATIVE_TOKEN },
    });
    if (response.data) {
        let gwei = 10 ** 9;
        let res = response.data;
        let estimatedPrice = res.blockPrices[0].estimatedPrices[0];
        estimate['maxPriorityFeePerGas'] = BigInt(parseInt(estimatedPrice['maxPriorityFeePerGas'] * gwei));
        estimate['maxFeePerGas'] = BigInt(parseInt(estimatedPrice['maxFeePerGas'] * gwei));
    }
    return estimate;
}

async function getTouchedPoolReserves(provider, blockNumber) {
    const syncEventSelector = ethers.utils.id('Sync(uint112,uint112)');
    const filter = {
        fromBlock: blockNumber,
        toBlock: blockNumber,
        topics: [syncEventSelector],
    };

    let abiCoder = new ethers.utils.AbiCoder();
    let logs = await provider.getLogs(filter);
    let txIdx = {};
    let reserves = {};
    for (let log of logs) {
        let address = log.address;
        let idx = log.transactionIndex;
        let prevTxIdx = txIdx[address] || 0;
        if (idx >= prevTxIdx) {
            let decoded = abiCoder.decode(
                ['uint112', 'uint112'], log.data
            );
            reserves[address] = [BigInt(decoded[0]), BigInt(decoded[1])];
            txIdx[address] = idx;
        }
    }
    return reserves;
}

module.exports = {
    calculateNextBlockBaseFee,
    estimateNextBlockGas,
    getTouchedPoolReserves,
};