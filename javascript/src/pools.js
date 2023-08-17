const { ethers } = require('ethers');
const fs = require('fs');
const path = require('path');
const cliProgress = require('cli-progress');

const { logger, CACHED_POOLS_FILE } = require('./constants');

const Erc20Abi = [
    'function decimals() external view returns (uint8)'
];

const V2FactoryAbi = [
    'event PairCreated(address indexed token0, address indexed token1, address pair, uint)'
];

const DexVariant = {
    UniswapV2: 2,
    UniswapV3: 3,
};

class Pool {
    constructor(
        address,
        version,
        token0,
        token1,
        decimals0,
        decimals1,
        fee
    ) {
        this.address = address;
        this.version = version;
        this.token0 = token0;
        this.token1 = token1;
        this.decimals0 = decimals0;
        this.decimals1 = decimals1;
        this.fee = fee;
    }

    cacheRow() {
        return [
            this.address,
            this.version,
            this.token0,
            this.token1,
            this.decimals0,
            this.decimals1,
            this.fee,
        ];
    }
}

const range = (start, stop, step) => {
    let loopCnt = Math.ceil((stop - start) / step);
    let rangeArray = [];
    for (let i = 0; i < loopCnt; i++) {
        let fromBlock = start + (i * step);
        let toBlock = Math.min(fromBlock + step, stop);
        rangeArray.push([fromBlock, toBlock]);
    }
    return rangeArray;
}

function loadCachedPools() {
    let cacheFile = path.join(__dirname, '..', CACHED_POOLS_FILE);
    let pools = {}
    if (fs.existsSync(cacheFile)) {
        const content = fs.readFileSync(cacheFile, 'utf-8');
        const rows = content.split('\n');
        for (let row of rows) {
            if (row == '') continue;
            row = row.split(',');
            if (row[0] == 'address') continue;
            let version = row[1] == '2' ? DexVariant.UniswapV2 : DexVariant.UniswapV3;
            let pool = new Pool(row[0],
                                version,
                                row[2],
                                row[3],
                                parseInt(row[4]),
                                parseInt(row[5]),
                                parseInt(row[6]))
            pools[row[0]] = pool;
        }
    }
    return pools;
}

function cacheSyncedPools(pools) {
    const columns = ['address', 'version', 'token0', 'token1', 'decimals0', 'decimals1', 'fee'];
    let data = columns.join(',') + '\n';
    for (let address in pools) {
        let pool = pools[address];
        let row = pool.cacheRow().join(',') + '\n';
        data += row;
    }
    let cacheFile = path.join(__dirname, '..', CACHED_POOLS_FILE);
    fs.writeFileSync(cacheFile, data, { encoding: 'utf-8' });
}

async function loadAllPoolsFromV2(
    httpsUrl,
    factoryAddresses,
    fromBlocks,
    chunk
) {
    /*
    Retrieves historical events from Uniswap V2 factories.

    Whenever a new pool is created from the Uniswap V2 factory,
    a "PairCreated" event is emitted. We request for all the PairCreated
    events from the block these factories were deployed.

    👉 NOTE: the process takes a really long time, because it has room for improvement.
    This function will make requests to the RPC endpoint one batch at a time,
    each looking at events from block range of: [fromBlock, toBlock] chunk size.
    */
    let pools = loadCachedPools();
    if (Object.keys(pools).length > 0) {
        return pools;
    }

    const provider = new ethers.providers.JsonRpcProvider(httpsUrl);
    const toBlock = await provider.getBlockNumber();
    
    const decimals = {};
    pools = {};

    for (let i = 0; i < factoryAddresses.length; i++) {
        const factoryAddress = factoryAddresses[i];
        const fromBlock = fromBlocks[i];

        const v2Factory = new ethers.Contract(factoryAddress, V2FactoryAbi, provider);

        const requestParams = range(fromBlock, toBlock, chunk);

        const progress = new cliProgress.SingleBar({}, cliProgress.Presets.shades_classic);
        progress.start(requestParams.length);

        for (let i = 0; i < requestParams.length; i++) {
            const params = requestParams[i];
            const filter = v2Factory.filters.PairCreated;
            const events = await v2Factory.queryFilter(filter, params[0], params[1]);

            for (let event of events) {
                let token0 = event.args[0];
                let token1 = event.args[1];

                let decimals0;
                let decimals1;

                try {
                    if (token0 in decimals) {
                        decimals0 = decimals[token0];
                    } else {
                        let token0Contract = new ethers.Contract(token0, Erc20Abi, provider);
                        decimals0 = await token0Contract.decimals();
                        decimals[token0] = decimals0;
                    }

                    if (token1 in decimals) {
                        decimals1 = decimals[token1];
                    } else {
                        let token1Contract = new ethers.Contract(token1, Erc20Abi, provider);
                        decimals1 = await token1Contract.decimals();
                        decimals[token1] = decimals1;
                    }
                } catch (_) {
                    // some token contracts don't exist anymore: eth_call error
                    logger.warn(`Check if tokens: ${token0} / ${token1} still exists`);
                    continue;
                }

                let pool = new Pool(event.args[2],
                                    DexVariant.UniswapV2,
                                    token0,
                                    token1,
                                    decimals0,
                                    decimals1,
                                    300);
                pools[event.args[2]] = pool;
            }

            progress.update(i + 1);
        }

        progress.stop();
    }

    cacheSyncedPools(pools);
    return pools;
}

module.exports = {
    loadAllPoolsFromV2,
};