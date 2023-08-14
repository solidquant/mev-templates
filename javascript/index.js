const ethers = require('ethers');
const v2FactoryABI = require('./abi/UniswapV2Factory.json');

require('dotenv').config();

const HTTPS_URL = process.env.HTTPS_URL;
const WSS_URL = process.env.WSS_URL;

const provider = new ethers.JsonRpcProvider(HTTPS_URL);
const wssProvider = new ethers.WebSocketProvider(WSS_URL);

const uniswapV2FactoryAddress = '0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f';
const sushiswapV2FactoryAddress = '0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac';

console.log(provider);


const sqrtx96ToPrice = (sqrtx96, decimals0, decimals1, token0_in) => {
    let price = Math.pow(sqrtx96 / Math.pow(2, 96), 2) * Math.pow(10, (decimals0 - decimals1));
    return token0_in ? price : 1 / price;
}

const p = sqrtx96ToPrice(
    1839874823181389578539566653045537,
    6,
    18,
    false
);

console.log(p);