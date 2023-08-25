const { addColors, createLogger, format, transports } = require('winston');
const { combine, timestamp, printf, colorize } = format;

require('dotenv').config();

const colors = {
    error: 'red',
    warn: 'yellow',
    info: 'black',
    http: 'magenta',
    debug: 'blue',
};

addColors(colors);

const logFormat = printf(({ level, message, timestamp }) => {
    return `${timestamp} [${level.toUpperCase()}] ${message}`;
});

const logger = createLogger({
    format: combine(
        timestamp(),
        logFormat,
        colorize({ all: true }),
    ),
    transports: [new transports.Console()],
});

const blacklistTokens = ['0x9469603F3Efbcf17e4A5868d81C701BDbD222555'];

// Multicall related
const MULTICALL_ADDRESS = '0xcA11bde05977b3631167028862bE2a173976CA11';

const MULTICALL_ABI = [
    // https://github.com/mds1/multicall
    'function aggregate(tuple(address target, bytes callData)[] calls) payable returns (uint256 blockNumber, bytes[] returnData)',
    'function aggregate3(tuple(address target, bool allowFailure, bytes callData)[] calls) payable returns (tuple(bool success, bytes returnData)[] returnData)',
    'function aggregate3Value(tuple(address target, bool allowFailure, uint256 value, bytes callData)[] calls) payable returns (tuple(bool success, bytes returnData)[] returnData)',
    'function blockAndAggregate(tuple(address target, bytes callData)[] calls) payable returns (uint256 blockNumber, bytes32 blockHash, tuple(bool success, bytes returnData)[] returnData)',
    'function getBasefee() view returns (uint256 basefee)',
    'function getBlockHash(uint256 blockNumber) view returns (bytes32 blockHash)',
    'function getBlockNumber() view returns (uint256 blockNumber)',
    'function getChainId() view returns (uint256 chainid)',
    'function getCurrentBlockCoinbase() view returns (address coinbase)',
    'function getCurrentBlockDifficulty() view returns (uint256 difficulty)',
    'function getCurrentBlockGasLimit() view returns (uint256 gaslimit)',
    'function getCurrentBlockTimestamp() view returns (uint256 timestamp)',
    'function getEthBalance(address addr) view returns (uint256 balance)',
    'function getLastBlockHash() view returns (bytes32 blockHash)',
    'function tryAggregate(bool requireSuccess, tuple(address target, bytes callData)[] calls) payable returns (tuple(bool success, bytes returnData)[] returnData)',
    'function tryBlockAndAggregate(bool requireSuccess, tuple(address target, bytes callData)[] calls) payable returns (uint256 blockNumber, bytes32 blockHash, tuple(bool success, bytes returnData)[] returnData)',
];

module.exports = {
    // env variables
    HTTPS_URL: process.env.HTTPS_URL,
    WSS_URL: process.env.WSS_URL,
    CHAIN_ID: process.env.CHAIN_ID || 1,
    BLOCKNATIVE_TOKEN: process.env.BLOCKNATIVE_TOKEN,
    PRIVATE_KEY: process.env.PRIVATE_KEY,
    SIGNING_KEY: process.env.SIGNING_KEY,
    BOT_ADDRESS: process.env.BOT_ADDRESS,

    // abi
    BOT_ABI: require('../abi/V2ArbBot.json'),

    // logging
    logger,

    // cache
    CACHED_POOLS_FILE: '.cached-pools.csv',

    // blacklist
    blacklistTokens,

    // multicall
    MULTICALL_ADDRESS,
    MULTICALL_ABI,

    // flashbots
    PRIVATE_RELAY: 'https://relay.flashbots.net',

    ZERO_ADDRESS: '0x0000000000000000000000000000000000000000',
};