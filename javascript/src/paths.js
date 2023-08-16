const cliProgress = require('cli-progress');

const { logger } = require('./constants');

class ArbPath {
    constructor(
        pool1,
        pool2,
        pool3,
        zeroForOne1,
        zeroForOne2,
        zeroForOne3
    ) {
        this.pool1 = pool1;
        this.pool2 = pool2;
        this.pool3 = pool3;
        this.zeroForOne1 = zeroForOne1;
        this.zeroForOne2 = zeroForOne2;
        this.zeroForOne3 = zeroForOne3;
    }

    nhop() {
        return this.pool3 === undefined ? 2 : 3;
    }

    hasPool(pool) {
        let isPool1 = this.pool1 == pool;
        let isPool2 = this.pool2 == pool;
        let isPool3 = this.pool3 == pool;
        return isPool1 || isPool2 || isPool3;
    }

    shouldBlacklist(blacklistTokens) {
        for (let i = 0; i < this.nhop(); i++) {
            let pool = this[`pool${i + 1}`];
            if ((pool.token0 in blacklistTokens) || (pool.token1 in blacklistTokens)) {
                return true;
            }
            return false;
        }
    }
}


function generateTriangularPaths(pools, tokenIn) {
    const paths = [];

    pools = Object.values(pools);

    const progress = new cliProgress.SingleBar({}, cliProgress.Presets.shades_classic);
    progress.start(pools.length);

    for (let i = 0; i < pools.length; i++) {
        let pool1 = pools[i];
        let canTrade1 = (pool1.token0 == tokenIn) || (pool1.token1 == tokenIn);
        if (canTrade1) {
            let zeroForOne1 = pool1.token0 == tokenIn;
            let tokenOut1 = zeroForOne1 ? pool1.token1 : pool1.token0;

            for (let j = 0; j < pools.length; j++) {
                let pool2 = pools[j];
                let canTrade2 = (pool2.token0 == tokenOut1) || (pool2.token1 == tokenOut1);
                if (canTrade2) {
                    let zeroForOne2 = pool2.token0 == tokenOut1;
                    let tokenOut2 = zeroForOne2 ? pool2.token1 : pool2.token0;

                    for (let k = 0; k < pools.length; k++) {
                        let pool3 = pools[k];
                        let canTrade3 = (pool3.token0 == tokenOut2) || (pool3.token1 == tokenOut2);
                        if (canTrade3) {
                            let zeroForOne3 = pool3.token0 == tokenOut2;
                            let tokenOut3 = zeroForOne3 ? pool3.token1 : pool3.token0;

                            if (tokenOut3 == tokenIn) {
                                let uniquePoolCnt = [...new Set([
                                    pool1.address,
                                    pool2.address,
                                    pool3.address,
                                ])].length;

                                if (uniquePoolCnt < 3) {
                                    continue;
                                }

                                let arbPath = new ArbPath(pool1,
                                                          pool2,
                                                          pool3,
                                                          zeroForOne1,
                                                          zeroForOne2,
                                                          zeroForOne3);
                                paths.push(arbPath);
                            }
                        }
                    }
                }
            }
        }
        progress.update(i + 1);
    }

    progress.stop();
    logger.info(`Generated ${paths.length} 3-hop arbitrage paths`);
    return paths;
}

module.exports = {
    ArbPath,
    generateTriangularPaths,
};