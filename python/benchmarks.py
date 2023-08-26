import os
import csv
import time
import asyncio
import datetime
import aioprocessing
from uuid import uuid4

from web3 import Web3
from pathlib import Path
from typing import Callable
from flashbots import Flashbots
from flashbots.flashbots import FlashbotsBundleResponse

from constants import (
    HTTPS_URL,
    WSS_URL,
    PRIVATE_KEY,
    SIGNING_KEY,
    BOT_ADDRESS,
)
from pools import load_all_pools_from_v2
from paths import generate_triangular_paths
from bundler import Bundler, Flashloan
from utils import get_touched_pool_reserves, calculate_next_block_base_fee
from multi import get_uniswap_v2_reserves, batch_get_uniswap_v2_reserves
from streams import stream_new_blocks, stream_pending_transactions

ZERO_ADDRESS = '0x0000000000000000000000000000000000000000'

# Create benches directory if it doesn't exist
_DIR = Path(os.path.dirname(os.path.abspath(__file__)))
BENCHMARK_DIR = _DIR / 'benches'
os.makedirs(BENCHMARK_DIR, exist_ok=True)


async def logging_event_handler(event_queue: aioprocessing.AioQueue):
    w3 = Web3(Web3.HTTPProvider(HTTPS_URL))
    
    f = open(BENCHMARK_DIR / '.benchmark.csv', 'w', newline='')
    wr = csv.writer(f)
    
    while True:
        try:
            data = await event_queue.coro_get()

            if data['type'] == 'pending_tx':
                _ = w3.eth.get_transaction(data['tx_hash'])
                now = datetime.datetime.now().timestamp() * 1000000
                wr.writerow([data['tx_hash'], int(now)])
        except Exception as _:
            break
        
        
async def touched_pools_event_handler(event_queue: aioprocessing.AioQueue):
    w3 = Web3(Web3.HTTPProvider(HTTPS_URL))
    
    while True:
        try:
            data = await event_queue.coro_get()
            
            if data['type'] == 'block':
                s = time.time()
                block_number = data['block_number']
                reserves = get_touched_pool_reserves(w3, block_number)
                took = (time.time() - s) * 1000
                now = datetime.datetime.now()
                print(f'[{now}] Block #{block_number} {len(reserves)} pools touched | Took: {took} ms')
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
    
    # #####################
    # # 2️⃣ Get block info #
    # #####################
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
    # stream_func = stream_pending_transactions
    # handler_func = logging_event_handler
    # print('6. Logging receive time for pending transaction streams. Wait 180 seconds...')
    # asyncio.run(benchmark_streams(stream_func, handler_func, 180))
    
    #################################################
    # 7️⃣ Retrieving logs from a newly created block #
    #################################################
    # stream_func = stream_new_blocks
    # handler_func = touched_pools_event_handler
    # print('7. Starting touched pools with new blocks streams. Wait 300 seconds...')
    # asyncio.run(benchmark_streams(stream_func, handler_func, 300))
    
    ############################
    # 8️⃣ 3-hop path simulation #
    ############################
    took = []
    for path in paths:
        s = time.time()
        _ = path.simulate_v2_path(1, reserves)
        time_took = (time.time() - s) * 1000000
        took.append(time_took)
    total_took = sum(took)
    print(total_took)
    print(len(took))
    avg_took = total_took / len(took)
    print(f'8. 3-hop path simulation took: {total_took} microsecs in total ({len(took)} simulations / avg: {avg_took})')
    
    #################################
    # 9️⃣ Creating Flashbots bundles #
    #################################
    unit = 10 ** usdc_decimals
    gwei = 10 ** 9
    
    block_number = w3.eth.get_block('latest').number
    router_address = '0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F'
    
    bundler = Bundler(PRIVATE_KEY, SIGNING_KEY, HTTPS_URL, BOT_ADDRESS)
    
    s = time.time()
    routers = [router_address, router_address, router_address]
    path = paths[0]
    path_params = path.to_path_params(routers)
    amount_in = 1 * unit
    flashloan = Flashloan.NotUsed
    loan_from = ZERO_ADDRESS
    max_priority_fee_per_gas = 1 * gwei
    max_fee_per_gas = 50 * gwei
    order_tx = bundler.order_tx(path_params,
                                amount_in,
                                max_priority_fee_per_gas,
                                max_fee_per_gas,
                                flashloan,
                                loan_from)
    bundle = bundler.to_bundle(order_tx)
    took = (time.time() - s) * 1000
    print(f'9. Creating Flashbots bundle | Took: {took} ms')
    print(bundle)
    
    ################################
    # 🔟 Sending Flashbots bundles #
    ################################
    block = w3.eth.get_block('latest')
    next_base_fee = calculate_next_block_base_fee(block)
    max_priority_fee_per_gas = 1  # 1 wei...this will never get added
    max_fee_per_gas = next_base_fee + max_priority_fee_per_gas
    
    took_list = []
    for i in range(10):
        _s = time.time()
        s = time.time()
        common = bundler._common_fields
        amount_in = int(0.001 * 10 ** 18)
        tx = {
            **common,
            'to': bundler.sender.address,
            'from': bundler.sender.address,
            'value': amount_in,
            'data': '0x',
            'gas': 30000,
            'maxFeePerGas': max_fee_per_gas,
            'maxPriorityFeePerGas': max_priority_fee_per_gas,
        }
        bundle = bundler.to_bundle(tx)
        took = (time.time() - s) * 1000
        print(f'- Creating bundle took: {took} ms')
        
        s = time.time()
        flashbots: Flashbots = bundler.w3.flashbots
        
        try:
            simulated = flashbots.simulate(bundle, block_number)
        except Exception as e:
            print('Simulation error', e)
        took = (time.time() - s) * 1000
        print(f'- Running simulation took: {took} ms')
        # print(simulated)
            
        s = time.time()
        replacement_uuid = str(uuid4())    
        response: FlashbotsBundleResponse = flashbots.send_bundle(
            bundle,
            target_block_number=block_number + 1,
            opts={'replacementUuid': replacement_uuid},
        )
        
        took = (time.time() - s) * 1000
        total_took = (time.time() - _s) * 1000
        print(f'10. Sending Flashbots bundle {response.bundle_hash().hex()} | Took: {took} ms')
        
        took_list.append(total_took)
        
    print(sum(took_list))
    