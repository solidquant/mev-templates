const cliProgress = require('cli-progress');

const { logger } = require('./constants');
const { Path } = require('./bundler');
const { UniswapV2Simulator } = require('./simulator');

const range = (start, stop, step) => {
    let loopCnt = Math.ceil((stop - start) / step);
    let rangeArray = [];
    for (let i = 0; i < loopCnt; i++) {
        let num = start + (i * step);
        rangeArray.push(num);
    }
    return rangeArray;
}

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
        let isPool1 = this.pool1.address.toLowerCase() == pool.toLowerCase();
        let isPool2 = this.pool2.address.toLowerCase() == pool.toLowerCase();
        let isPool3 = this.pool3.address.toLowerCase() == pool.toLowerCase();
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

    simulateV2Path(amountIn, reserves) {
        let tokenInDecimals = this.zeroForOne1 ? this.pool1.decimals0 : this.pool1.decimals1;
        let amountOut = amountIn * 10 ** tokenInDecimals;

        let sim = new UniswapV2Simulator();
        let nhop = this.nhop();
        for (let i = 0; i < nhop; i++) {
            let pool = this[`pool${i + 1}`];
            let zeroForOne = this[`zeroForOne${i + 1}`];
            let reserve0 = reserves[pool.address][0];
            let reserve1 = reserves[pool.address][1];
            let fee = pool.fee;
            let reserveIn = zeroForOne ? reserve0 : reserve1;
            let reserveOut = zeroForOne ? reserve1 : reserve0;
            amountOut = sim.getAmountOut(amountOut, reserveIn, reserveOut, fee);
        }
        return amountOut;
    }

    optimizeAmountIn(maxAmountIn, stepSize, reserves) {
        let tokenInDecimals = this.zeroForOne1 ? this.pool1.decimals0 : this.pool1.decimals1;
        let optimizedIn = 0;
        let profit = 0;
        for (let amountIn of range(0, maxAmountIn, stepSize)) {
            let amountOut = this.simulateV2Path(amountIn, reserves);
            let thisProfit = amountOut - (amountIn * (10 ** tokenInDecimals));
            if (thisProfit >= profit) {
                optimizedIn = amountIn;
                profit = thisProfit;
            } else {
                break;
            }
        }
        return [optimizedIn, profit / (10 ** tokenInDecimals)];
    }

    toPathParams(routers) {
        let pathParams = [];
        for (let i = 0; i < this.nhop(); i++) {
            let pool = this[`pool${i + 1}`];
            let zeroForOne = this[`zeroForOne${i + 1}`];
            let tokenIn = zeroForOne ? pool.token0 : pool.token1;
            let tokenOut = zeroForOne ? pool.token1 : pool.token0;
            let path = new Path(routers[i], tokenIn, tokenOut);
            pathParams.push(path);
        }
        return pathParams;
    }
}


function generateTriangularPaths(pools, tokenIn) {
    /*
    This can easily be refactored into a recursive function to support the
    generation of n-hop paths. However, I left it as a 3-hop path generating function
    just for demonstration. This will be easier to follow along.

    👉 The recursive version can be found here (Python):
    https://github.com/solidquant/whack-a-mole/blob/main/data/dex.py
    */
    const paths = [];

    pools = Object.values(pools);

    const progress = new cliProgress.SingleBar({}, cliProgress.Presets.shades_classic);
    progress.start(pools.length);

    for (let i = 0; i < pools.length; i++) {
        let pool1 = pools[i];
        let canTrade1 = (pool1.token0 == tokenIn) || (pool1.token1 == tokenIn);
        if (canTrade1) {
            let zeroForOne1 = pool1.token0 == tokenIn;
            let [tokenIn1, tokenOut1] = zeroForOne1 ? [pool1.token0, pool1.token1] : [pool1.token1, pool1.token0];
            if (tokenIn1 != tokenIn) {
                continue;
            }

            for (let j = 0; j < pools.length; j++) {
                let pool2 = pools[j];
                let canTrade2 = (pool2.token0 == tokenOut1) || (pool2.token1 == tokenOut1);
                if (canTrade2) {
                    let zeroForOne2 = pool2.token0 == tokenOut1;
                    let [tokenIn2, tokenOut2] = zeroForOne2 ? [pool2.token0, pool2.token1] : [pool2.token1, pool2.token0];
                    if (tokenOut1 != tokenIn2) {
                        continue;
                    }

                    for (let k = 0; k < pools.length; k++) {
                        let pool3 = pools[k];
                        let canTrade3 = (pool3.token0 == tokenOut2) || (pool3.token1 == tokenOut2);
                        if (canTrade3) {
                            let zeroForOne3 = pool3.token0 == tokenOut2;
                            let [tokenIn3, tokenOut3] = zeroForOne3 ? [pool3.token0, pool3.token1] : [pool3.token1, pool3.token0];
                            if (tokenOut2 != tokenIn3) {
                                continue;
                            }

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