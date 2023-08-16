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
    // transports: [new transports.Console()],
});

const blacklistTokens = ['0x9469603F3Efbcf17e4A5868d81C701BDbD222555'];

module.exports = {
    // env variables
    HTTPS_URL: process.env.HTTPS_URL,
    WSS_URL: process.env.WSS_URL,
    CHAIN_ID: process.env.CHAIN_ID,
    BLOCKNATIVE_TOKEN: process.env.BLOCKNATIVE_TOKEN,
    PRIVATE_KEY: process.env.PRIVATE_KEY,
    SIGNING_KEY: process.env.SIGNING_KEY,
    BOT_ADDRESS: process.env.BOT_ADDRESS,

    // logging
    logger,

    // cache
    CACHED_POOLS_FILE: '.cached-pools.csv',

    // blacklist
    blacklistTokens,
};