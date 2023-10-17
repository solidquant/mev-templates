import math
import multiprocessing

from web3 import Web3
from typing import Dict
from multicall import Call, Multicall

from pools import Pool
from constants import logger


def get_uniswap_v2_reserves(https_url: str, pools: Dict[str, Pool]):
    w3 = Web3(Web3.HTTPProvider(https_url))
    signature = 'getReserves()((uint112,uint112,uint32))'  # reserve0, reserve1, blockTimestampLast

    calls = []
    for pool_address in pools:
        call = Call(
            pool_address,
            signature,
            [(pool_address, lambda x: x)]
        )
        calls.append(call)

    multicall = Multicall(calls, _w3=w3)
    result = multicall()
    reserves = {k: list(v)[:2] for k, v in result.items()}
    """
    reserves:
    {
        '0xF4b8A02D4e8D76070bD7092B54D2cBbe90fa72e9': [17368643486106939361172, 31867695075486],
        '0x80067013d7F7aF4e86b3890489AcAFe79F31a4Cb': [5033262526671305584632, 9254792586342]
    }
    """
    return reserves


def batch_get_uniswap_v2_reserves(https_url: str, pools: Dict[str, Pool]):
    mp = multiprocessing.Pool()
    
    pools_cnt = len(pools)
    batch = math.ceil(pools_cnt / 250)
    pools_per_batch = math.ceil(pools_cnt / batch)
    
    args = []
    
    for i in range(batch):
        start_idx = i * pools_per_batch
        end_idx = min(start_idx + pools_per_batch, pools_cnt)
        args.append((https_url, list(pools.keys())[start_idx:end_idx]))
        
    results = mp.starmap(get_uniswap_v2_reserves, args)
    
    reserves = {}
    for result in results:
        reserves = {**reserves, **result}
        
    return reserves


if __name__ == '__main__':
    import os
    import time
    from dotenv import load_dotenv

    from pools import load_all_pools_from_v2
    from paths import generate_triangular_paths

    load_dotenv(override=True)

    HTTPS_URL = os.getenv('HTTPS_URL')

    # Example on Ethereum
    uniswap_v2_factory_addresses = ['0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac']
    uniswap_v2_factory_blocks = [10794229]

    w3 = Web3(Web3.HTTPProvider(HTTPS_URL))
    pools = load_all_pools_from_v2(HTTPS_URL, uniswap_v2_factory_addresses, uniswap_v2_factory_blocks, 50000)
    logger.info(f'Pool count: {len(pools)}')
    
    usdt_address = '0xdAC17F958D2ee523a2206206994597C13D831ec7'
    paths = generate_triangular_paths(pools, usdt_address)
    
    # Filter pools that were used in arb paths
    pools = {}
    for path in paths:
        pools[path.pool_1.address] = path.pool_1
        pools[path.pool_2.address] = path.pool_2
        pools[path.pool_3.address] = path.pool_3
        
    logger.info(f'New pool count: {len(pools)}')

    """
    It seems like passing in thousands of pools as input halts to a stop somehow,
    or it simply takes too long to retrieve data.
    
    Thus, filter out the pools you're going to use in arb. paths,
    and batch request those pools.
    
    Benchmark: requesting 929 pools takes 3 seconds
    """
    s = time.time()
    reserves = batch_get_uniswap_v2_reserves(HTTPS_URL, pools)
    e = time.time()
    logger.info(f'Took: {e - s} seconds')
    logger.info(len(reserves))
    logger.info(reserves)
