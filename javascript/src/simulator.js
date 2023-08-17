class UniswapV2Simulator {
    constructor() {}

    reservesToPrice(
        reserve0,
        reserve1,
        decimals0,
        decimals1,
        token0In
    ) {
        reserve0 = Number(reserve0);
        reserve1 = Number(reserve1);
        decimals0 = Number(decimals0);
        decimals1 = Number(decimals1);

        let price = (reserve1 / reserve0) * 10 ** (decimals0 - decimals1);
        return token0In ? price : 1 / price;
    }

    getAmountOut(
        amountIn,
        reserveIn,
        reserveOut,
        fee
    ) {
        amountIn = BigInt(amountIn);
        reserveIn = BigInt(reserveIn);
        reserveOut = BigInt(reserveOut);
        fee = BigInt(fee);

        fee = fee / BigInt(100);
        let amountInWithFee = amountIn * (BigInt(1000) - fee);
        let numerator = amountInWithFee * reserveOut;
        let denominator = (reserveIn * BigInt(1000)) + amountInWithFee;
        return denominator == 0 ? 0 : Number(numerator / denominator);
    }
}

module.exports = {
    UniswapV2Simulator,
};