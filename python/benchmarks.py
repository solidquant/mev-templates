import time
import asyncio
import aioprocessing
from web3 import Web3
from typing import Callable
from functools import partial

from constants import HTTPS_URL, WSS_URL
from pools import load_all_pools_from_v2
from paths import generate_triangular_paths
from multi import get_uniswap_v2_reserves, batch_get_uniswap_v2_reserves
from streams import stream_new_blocks, stream_pending_transactions


async def logging_event_handler(event_queue: aioprocessing.AioQueue):
    w3 = Web3(Web3.HTTPProvider(HTTPS_URL))
    
    while True:
        try:
            data = await event_queue.coro_get()

            if data['type'] == 'pending_tx':
                print(data)
        except Exception as _:
            break
            

async def benchmark_streams(stream_func: Callable,
                            handler_func: Callable,
                            run_time: int):
    
    event_queue = aioprocessing.AioQueue()
        
    stream_task = asyncio.create_task(stream_func(WSS_URL, event_queue, False))
    handler_task = asyncio.create_task(handler_func(event_queue))
    
    await asyncio.sleep(run_time)
    event_queue.put(0)
    
    stream_task.cancel()
    handler_task.cancel()


if __name__ == '__main__':
    print('Starting benchmark')
    
    ###########################
    # 1️⃣ Create HTTP provider #
    ###########################
    s = time.time()
    w3 = Web3(Web3.HTTPProvider(HTTPS_URL))
    took = (time.time() - s) * 1000000
    print(f'1. HTTP provider created | Took: {took} microsec')
    
    #####################
    # 2️⃣ Get block info #
    #####################
    for i in range(10):
        s = time.time()
        block = w3.eth.get_block('latest')
        took = (time.time() - s) * 1000
        print(f'2. New block: #{block["number"]} | Took: {took} ms')
    
    # Common variables used throughout 
    factory_addresses = ['0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac']
    factory_blocks = [10794229]
    usdc_address = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48'
    usdc_decimals = 6
        
    ###################################
    # 3️⃣ Retrieving cached pools data #  
    ################################### 
    s = time.time()
    pools = load_all_pools_from_v2(HTTPS_URL, factory_addresses, factory_blocks)
    took = (time.time() - s) * 1000
    print(f'3. Cached {len(pools)} pools data | Took: {took} ms')
    
    ##########################################
    # 4️⃣ Generate triangular arbitrage paths #
    ##########################################
    s = time.time()
    paths = generate_triangular_paths(pools, usdc_address)
    took = (time.time() - s) * 1000
    print(f'4. Generated {len(paths)} 3-hop paths | Took: {took} ms')

    ###########################################################
    # 5️⃣ Multicall test: calling 250 requests using multicall #
    # This is used quite often in real bots.                  #
    ###########################################################
    
    # Single multicall
    s = time.time()
    reserves = get_uniswap_v2_reserves(HTTPS_URL, list(pools.keys())[0:250])
    took = (time.time() - s) * 1000
    print(f'5. Multicall result for {len(reserves)} | Took: {took} ms')
    
    # Batch multicall (thousands of requests asynchronously)
    s = time.time()
    reserves = batch_get_uniswap_v2_reserves(HTTPS_URL, pools)
    took = (time.time() - s) * 1000
    print(f'5. Bulk multicall result for {len(reserves)} | Took: {took} ms')
    
    #######################################
    # 6️⃣ Pending transaction async stream #
    #######################################
    stream_func = stream_pending_transactions
    handler_func = logging_event_handler
    asyncio.run(benchmark_streams(stream_func, handler_func, 20))
    