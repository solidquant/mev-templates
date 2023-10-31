from tqdm import tqdm
from typing import Dict, List, Optional

from pools import Pool
from bundler import Path
from constants import logger
from simulator import UniswapV2Simulator


class ArbPath:
    
    def __init__(self,
                 pool_1: Pool,
                 pool_2: Pool,
                 pool_3: Optional[Pool],
                 zero_for_one_1: bool,
                 zero_for_one_2: bool,
                 zero_for_one_3: bool):
        
        self.pool_1 = pool_1
        self.pool_2 = pool_2
        self.pool_3 = pool_3
        self.zero_for_one_1 = zero_for_one_1
        self.zero_for_one_2 = zero_for_one_2
        self.zero_for_one_3 = zero_for_one_3
        
    @property
    def nhop(self) -> int:
        return 2 if self.pool_3 is None else 3
    
    def has_pool(self, pool: str) -> bool:
        is_pool_1 = self.pool_1.address == pool
        is_pool_2 = self.pool_2.address == pool
        is_pool_3 = self.pool_3.address == pool
        return is_pool_1 or is_pool_2 or is_pool_3
    
    def should_blacklist(self, blacklist_tokens: List[str]) -> bool:
        for i in range(self.nhop):
            pool = getattr(self, f'pool_{i + 1}')
            if pool.token0 in blacklist_tokens or pool.token1 in blacklist_tokens:
                return True
        return False
    
    def simulate_v2_path(self, amount_in: float, reserves: Dict[str, Pool]):
        """
        Only works for v2 pool only paths
        """
        token_in_decimals = self.pool_1.decimals0 if self.zero_for_one_1 else self.pool_1.decimals1
        real_amount_in = int(amount_in * (10 ** token_in_decimals))
        return simulate_v2_path(self, real_amount_in, reserves)
    
    def optimize_amount_in(self,
                           max_amount_in: int,
                           step_size: int,
                           reserves: Dict[str, Pool]) -> (int, int):
        # a simple brute force profit optimization
        token_in_decimals = self.pool_1.decimals0 if self.zero_for_one_1 else self.pool_1.decimals1
        optimized_in = 0
        profit = 0
        for amount_in in range(0, max_amount_in, step_size):
            amount_out = self.simulate_v2_path(amount_in, reserves)
            this_profit = amount_out - (amount_in * (10 ** token_in_decimals))
            if this_profit >= profit:
                optimized_in = amount_in
                profit = this_profit
            else:
                break
        return optimized_in, profit / (10 ** token_in_decimals)
    
    def to_path_params(self, routers: List[str]) -> List[Path]:
        path_params = []
        for i in range(self.nhop):
            pool = getattr(self, f'pool_{i + 1}')
            zero_for_one = getattr(self, f'zero_for_one_{i + 1}')
            if zero_for_one:
                token_in, token_out = pool.token0, pool.token1
            else:
                token_in, token_out = pool.token1, pool.token0
            path = Path(routers[i], token_in, token_out)
            path_params.append(path)
        return path_params
    
    
def simulate_v2_path(path: ArbPath, amount_in: int, reserves: Dict[str, Pool]) -> int:
    sim = UniswapV2Simulator()
    
    for i in range(path.nhop):
        pool = getattr(path, f'pool_{i + 1}')
        zero_for_one = getattr(path, f'zero_for_one_{i + 1}')
        reserve0 = reserves[pool.address][0]
        reserve1 = reserves[pool.address][1]
        fee = pool.fee
        if zero_for_one:
            reserve_in, reserve_out = reserve0, reserve1
        else:
            reserve_in, reserve_out = reserve1, reserve0
        amount_out = sim.get_amount_out(amount_in, reserve_in, reserve_out, fee)
        amount_in = amount_out
        
    return amount_out


def generate_triangular_paths(pools: Dict[str, Pool], token_in: str) -> List[ArbPath]:
    """
    A straightforward triangular arbitrage path finder.
    This call will find both 2-hop paths, 3-hop paths, but not more.
    Also, we define triangular arb. paths as a 3-hop swap path starting
    with token_in and ending with token_in:
    
    token_in --> token1 --> token2 --> token_in
    
    NOTE: this function is highly recursive, and can easily extend to n-hop paths.
    Refer to https://github.com/solidquant/whack-a-mole/blob/main/data/dex.py
    __generate_paths function for this.
    """
    paths = []
    
    pools = list(pools.values())
    
    for pool_1 in tqdm(pools,
                       total=len(pools),
                       ncols=100,
                       desc=f'Generating paths',
                       ascii=' =',
                       leave=True):
        pools_in_path = []
        pools_in_path.append(pool_1.address)
        can_trade_1 = (pool_1.token0 == token_in) or (pool_1.token1 == token_in)
        if can_trade_1:
            zero_for_one_1 = pool_1.token0 == token_in
            (token_in_1, token_out_1) = (pool_1.token0, pool_1.token1) if zero_for_one_1 else (pool_1.token1, pool_1.token0)
            if token_in_1 != token_in:
                continue
            
            for j in range(len(pools)):
                pool_2 = pools[j]
                pools_in_path.append(pool_2.address)
                can_trade_2 = (pool_2.token0 == token_out_1) or (pool_2.token1 == token_out_1)
                if can_trade_2:
                    zero_for_one_2 = pool_2.token0 == token_out_1
                    (token_in_2, token_out_2) = (pool_2.token0, pool_2.token1) if zero_for_one_2 else (pool_2.token1, pool_2.token0)
                    if token_out_1 != token_in_2:
                        continue
                    
                    for k in range(len(pools)):
                        pool_3 = pools[k]
                        pools_in_path.append(pool_3.address)
                        can_trade_3 = (pool_3.token0 == token_out_2) or (pool_3.token1 == token_out_2)
                        if can_trade_3:
                            zero_for_one_3 = pool_3.token0 == token_out_2
                            (token_in_3, token_out_3) = (pool_3.token0, pool_3.token1) if zero_for_one_3 else (pool_3.token1, pool_3.token0)
                            if token_out_2 != token_in_3:
                                continue
                            
                            if token_out_3 == token_in:
                                unique_pool_cnt = len(set(pools_in_path))
                                
                                if unique_pool_cnt < 3:
                                    continue
                                
                                arb_path = ArbPath(pool_1=pool_1,
                                                   pool_2=pool_2,
                                                   pool_3=pool_3,
                                                   zero_for_one_1=zero_for_one_1,
                                                   zero_for_one_2=zero_for_one_2,
                                                   zero_for_one_3=zero_for_one_3)
                                paths.append(arb_path)
                                
    logger.info(f'Generated {len(paths)} 3-hop arbitrage paths')
    return paths


if __name__ == '__main__':
    from constants import HTTPS_URL
    from pools import load_all_pools_from_v2
    
    factory_addresses = [
        '0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac',  # Uniswap V2 (Ethereum)
    ]
    factory_blocks = [
        10794229,
    ]
    
    pools = load_all_pools_from_v2(HTTPS_URL,
                                   factory_addresses,
                                   factory_blocks,
                                   50000)
    
    token_in = '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2'  # WETH
    paths = generate_triangular_paths(pools, token_in)
    # logger.info(paths)