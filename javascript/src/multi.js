const { ethers } = require('ethers');

const UniswapV2PairAbi = require('../abi/UniswapV2Pair.json');

const {
    MULTICALL_ADDRESS,
    MULTICALL_ABI,
} = require('./constants');

async function getUniswapV2Reserves(httpsUrl, poolAddresses) {
    const v2PairInterface = new ethers.Interface(UniswapV2PairAbi);

    const calls = poolAddresses.map(address => ({
        target: address,
        allowFailure: true,
        callData: v2PairInterface.encodeFunctionData('getReserves', []),
    }));

    const provider = new ethers.JsonRpcProvider(httpsUrl);
    const multicall = new ethers.Contract(MULTICALL_ADDRESS, MULTICALL_ABI, provider);
    const result = await multicall.aggregate3.staticCall(calls);

    let reserves = {};
    for (let i = 0; i < result.length; i++) {
        let response = result[i];
        if (response.success) {
            let decoded = v2PairInterface.decodeFunctionResult('getReserves', response.returnData);
            reserves[poolAddresses[i]] = [decoded[0], decoded[1]];
        }
    }

    return reserves;
}

async function batchGetUniswapV2Reserves(httpsUrl, poolAddresses) {
    let poolsCnt = poolAddresses.length;
    let batch = Math.ceil(poolsCnt / 200);
    let poolsPerBatch = Math.ceil(poolsCnt / batch);

    let promises = [];

    for (let i = 0; i < batch; i++) {
        let startIdx = i * poolsPerBatch;
        let endIdx = Math.min(startIdx + poolsPerBatch, poolsCnt);
        promises.push(
            getUniswapV2Reserves(httpsUrl, poolAddresses.slice(startIdx, endIdx))
        );
    }

    const results = await Promise.all(promises);
    const reserves = Object.assign(...results);
    return reserves;
}

module.exports = {
    getUniswapV2Reserves,
    batchGetUniswapV2Reserves,
};