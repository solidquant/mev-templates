class UniswapV2Simulator:

    def __init__(self):
        pass

    def reserves_to_price(self,
                          reserve0: float,
                          reserve1: float,
                          decimals0: float,
                          decimals1: float,
                          token0_in: bool):
        """
        Returns the price quote of Uniswap V2 variant pools
        This is a simple quote, thus, does not account for price impact

        To get the amount out of amount in, use UniswapV2Simulator.get_amount_out
        for consideration of both fees and price impact
        """
        price = reserve1 / reserve0 * 10 ** (decimals0 - decimals1)
        return price if token0_in else 1 / price

    def get_amount_out(self,
                       amount_in: float,
                       reserve_in: float,
                       reserve_out: float,
                       fee: float = 300):
        """
        Fee in Uniswap V2 variants are 0.3%
        However, for variants that have different fee rates,
        fee can be overrided

        fee in get_amount_out, get_amount_in are used in the format
        that is saved into storage_array from dex.DEX class for consistency
        with Uniswap V3 variant pools
        """
        fee = fee // 100
        amount_in_with_fee = amount_in * (1000 - fee)
        numerator = amount_in_with_fee * reserve_out
        denominator = (reserve_in * 1000) + amount_in_with_fee
        if denominator == 0:
            return 0
        return int(numerator / denominator)

    def get_amount_in(self,
                      amount_out: float,
                      reserve_in: float,
                      reserve_out: float,
                      fee: float = 300):
        fee = fee // 100
        numerator = reserve_in * amount_out * 1000
        denominator = (reserve_out - amount_out) * (1000 - fee)
        return int(numerator / denominator + 1)

    def get_max_amount_in(self,
                          reserve0: float,
                          reserve1: float,
                          decimals0: float,
                          decimals1: float,
                          fee: float,
                          token0_in: bool,
                          max_amount_in: float,
                          step_size: float,
                          slippage_tolerance_lower: float,
                          slippage_tolerance_upper: float) -> float:
        """
        Calculates the maximum amount_in we can swap to get amount_out
        This method accounts for both: 1. fee, 2. price impact
        Also, we calculate the price quote using reserves and use that price
        to account for slippage tolerance
        We make sure that:

        amount_out >= price_quote * (1 - slippage_tolerance)

        This method uses binary search to find the optimized amount_in value
        To reduce the search space, we pre-set values such as: max_amount_in, step_size,
                                                               slippage_tolerance_lower/upper

        * Slippage tips:

        1. Setting slippage_tolerance_lower: 0, slippage_tolerance_upper: 0.001
        will find the amount_in with a slippage below 0.1% --> this method is faster

        2. However, if you want to fine tune your amount_in, you should set the tolerance level like:
        slippage_tolerance_lower: 0.0009, slippage_tolerance_upper: 0.001

        :param max_amount_in: the max_amount_in used in binary search
        :param step_size: the order step_size. ex) 0.01, 0.1, 1, 10, etc...
        :param slippage_tolerance_lower: 0.01 (1%), 0.005 (0.5%), ...
        :param slippage_tolerance_upper: 0.01 (1%), ...
        """
        fee_pct = fee / 10000.0 / 100.0
        price_quote = self.reserves_to_price(reserve0,
                                             reserve1,
                                             decimals0,
                                             decimals1,
                                             token0_in)
        price_quote = price_quote * (1 - fee_pct)

        if token0_in:
            decimal_in, decimal_out = decimals0, decimals1
            reserve_in, reserve_out = reserve0, reserve1
        else:
            decimal_in, decimal_out = decimals1, decimals0
            reserve_in, reserve_out = reserve1, reserve0

        optimized_in = 0

        left = 0
        right = max_amount_in

        max_amount_out = self.get_amount_out(right * (10 ** decimal_in),
                                             reserve_in,
                                             reserve_out,
                                             fee)
        amount_out_rate = max_amount_out / right / (10 ** decimal_out)
        slippage = (price_quote - amount_out_rate) / price_quote

        if slippage < slippage_tolerance_lower:
            """
            If the maximum amount_in value is within the slippage tolerance level,
            we simply return that value
            """
            optimized_in = right
        else:
            while left <= right:
                mid = ((left + right) / 2) // step_size / (1 / step_size)
                amount_out = self.get_amount_out(mid * (10 ** decimal_in),
                                                 reserve_in,
                                                 reserve_out,
                                                 fee)
                amount_out_rate = amount_out / mid / (10 ** decimal_out)
                slippage = (price_quote - amount_out_rate) / price_quote
                if slippage_tolerance_lower <= slippage <= slippage_tolerance_upper:
                    optimized_in = mid
                    break
                else:
                    if slippage < slippage_tolerance_lower:
                        left = mid
                    else:
                        right = mid

        return optimized_in


if __name__ == '__main__':
    import os
    
    from dotenv import load_dotenv
    from web3 import Web3

    from pools import load_all_pools_from_v2
    from paths import generate_triangular_paths, simulate_v2_path
    
    from constants import logger
    from multi import batch_get_uniswap_v2_reserves

    load_dotenv(override=True)

    HTTPS_URL = os.getenv('HTTPS_URL')

    # Example on Ethereum
    uniswap_v2_factory_addresses = ['0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac']
    uniswap_v2_factory_blocks = 10794229

    w3 = Web3(Web3.HTTPProvider(HTTPS_URL))
    pools = load_all_pools_from_v2(HTTPS_URL, uniswap_v2_factory_addresses, uniswap_v2_factory_blocks, 50000)
    
    usdt_address = '0xdAC17F958D2ee523a2206206994597C13D831ec7'
    paths = generate_triangular_paths(pools, usdt_address)
    
    # Filter pools that were used in arb paths
    pools = {}
    for path in paths:
        pools[path.pool_1.address] = path.pool_1
        pools[path.pool_2.address] = path.pool_2
        pools[path.pool_3.address] = path.pool_3

    reserves = batch_get_uniswap_v2_reserves(HTTPS_URL, pools)
    
    # Do the below if you know the real amount_in
    path = paths[0]
    amount_out = simulate_v2_path(path, 1000000, reserves)
    print(path, amount_out)
    
    amount_out = path.simulate_v2_path(1, reserves)
    print(amount_out)
    
    # optimizing amount_in
    max_amount_in = 10000
    step_size = 100
    amount_ins = list(range(max_amount_in // step_size))
    amount_outs = [path.simulate_v2_path(int(step_size * i), reserves) for i in amount_ins]
    print(amount_outs)
    