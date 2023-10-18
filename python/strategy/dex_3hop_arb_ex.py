import time
import asyncio
import threading
import aioprocessing
from web3 import Web3
from functools import partial

from pools import load_all_pools_from_v2
from paths import generate_triangular_paths
from multi import batch_get_uniswap_v2_reserves
from utils import (
    reconnecting_websocket_loop,
    get_touched_pool_reserves,
)
from streams import stream_new_blocks
from simulator import UniswapV2Simulator
from bundler import Path, Bundler, Flashloan

from constants import (
    HTTPS_URL,
    WSS_URL,
    PRIVATE_KEY,
    SIGNING_KEY,
    BOT_ADDRESS,
    logger,
)

# change if needed, the token below is a placeholder
blacklist_tokens = ['0x9469603F3Efbcf17e4A5868d81C701BDbD222555']


async def event_handler(event_queue: aioprocessing.AioQueue):
    w3 = Web3(Web3.HTTPProvider(HTTPS_URL))
    
    factory_addresses = [
        '0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac',  # Uniswap V2 (Ethereum)
    ]
    factory_blocks = [
        10794229,
    ]
    
    # Retrieve all Sushiswap V2 pools
    pools = load_all_pools_from_v2(HTTPS_URL,
                                   factory_addresses,
                                   factory_blocks,
                                   50000)
    
    logger.info(f'Initial pool count: {len(pools)}')
    
    # Create triangular paths using USDT as the starting/ending token
    usdc_address = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48'
    usdc_decimals = 6
    
    paths = generate_triangular_paths(pools, usdc_address)
    
    # Filter pools that were used in arb paths
    pools = {}
    for path in paths:
        if not path.should_blacklist(blacklist_tokens):
            pools[path.pool_1.address] = path.pool_1
            pools[path.pool_2.address] = path.pool_2
            pools[path.pool_3.address] = path.pool_3
        
    logger.info(f'New pool count: {len(pools)}')
    
    # Send multicall request to retrieve all reserves data for the pools
    s = time.time()
    reserves = batch_get_uniswap_v2_reserves(HTTPS_URL, pools)
    e = time.time()
    logger.info(f'Batch reserves call took: {e - s} seconds')
    
    sim = UniswapV2Simulator()
    
    def _get_weth_price(_reserves: dict):
        """
        Retrieves the price of WMATIC in USDC
        """
        usdc_weth_address = '0x397FF1542f962076d0BFE58eA045FfA2d347ACa0'
        pool = pools[usdc_weth_address]
        reserve = _reserves[usdc_weth_address]
        price = sim.reserves_to_price(reserve[0],
                                      reserve[1],
                                      pool.decimals0,
                                      pool.decimals1,
                                      True)
        return price
    
    bundler = Bundler(PRIVATE_KEY, SIGNING_KEY, HTTPS_URL, BOT_ADDRESS)
    
    while True:
        data = await event_queue.coro_get()
        
        block_number = data['block_number']
        touched_reserves = get_touched_pool_reserves(w3, block_number)
        touched_pools = []
        for address, reserve in touched_reserves.items():
            if address in reserves:
                reserves[address] = reserve
                touched_pools.append(address)
        
        spreads = {}
        for idx, path in enumerate(paths):
            touched_path = sum([path.has_pool(pool) for pool in touched_pools]) >= 1
            if touched_path:
                try:
                    # get the price quote by using 1 USDT as amount_in
                    price_quote = path.simulate_v2_path(1, reserves)
                    spread = (price_quote / 1000000 - 1) * 100
                    if spread > 0:
                        spreads[idx] = spread
                except:
                    continue
                    
        # calculated estimated cost of bet
        weth_price = _get_weth_price(reserves)
        base_fee = int(data['next_base_fee'] * 1.1)
        gas_cost_in_weth = (base_fee * 550000) / 10 ** 18  # estimated gas usage for 3-hop swap + flashloan
        gas_cost = weth_price * gas_cost_in_weth
        
        sorted_spreads = sorted(spreads.items(), key=lambda x: x[1], reverse=True)[:1]
        print(f'Block #{block_number}: {sorted_spreads}')
        
        for spread in sorted_spreads:
            path_idx = spread[0]
            path = paths[path_idx]
            amount_in, expected_profit = path.optimize_amount_in(1000, 10, reserves)
            excess_profit = expected_profit - gas_cost
            print(f'Spread found: {spread}. Amount in: {amount_in} / Expected profit: {expected_profit} / Gas cost: {gas_cost}')
            
            if excess_profit > 0:
                uniswap_v2_router = '0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F'
                balancer_vault = '0xBA12222222228d8Ba445958a75a0704d566BF2C8'
                
                swap_paths = []
    
                for i in range(path.nhop):
                    pool = getattr(path, f'pool_{i + 1}')
                    zero_for_one = getattr(path, f'zero_for_one_{i + 1}')
                    if zero_for_one:
                        token_in, token_out = pool.token0, pool.token1
                    else:
                        token_in, token_out = pool.token1, pool.token0
                    swap_path = Path(uniswap_v2_router, token_in, token_out)
                    swap_paths.append(swap_path)
                    print(f'- {i} {token_in} --> {token_out}')
                    
                # base_fee = data['next_base_fee']
                # profit_in_wmatic = (excess_profit / wmatic_price) * (10 ** 18)
                # profit_per_gas = int(profit_in_wmatic / 550000)
                # max_priority_fee_per_gas = int(profit_per_gas * 0.9)
                # max_fee_per_gas = base_fee + max_priority_fee_per_gas
                # print(f'max_priority_fee_per_gas: {max_priority_fee_per_gas / 10 ** 18}')
                # print(f'max_fee_per_gas: {max_fee_per_gas / 10 ** 18}')
                
                order_tx = bundler.order_tx(swap_paths,
                                            amount_in * 10 ** usdc_decimals,
                                            data['max_priority_fee_per_gas'] * 3,
                                            data['max_fee_per_gas'] * 4,
                                            Flashloan.Balancer,
                                            balancer_vault)
                
                t = threading.Thread(target=bundler.send_tx, args=(order_tx,))
                t.start()
                # tx_hash = bundler.send_tx(order_tx)
                # print(f'Block #{block_number}: {tx_hash}')
                print(order_tx)
                print('\n')


def main():
    """
    A sample MEV code of triangular arbitrage on Sushiswap V2 Polygon
    """
    logger.info('Starting strategy')
    
    event_queue = aioprocessing.AioQueue()
    
    new_blocks_stream = reconnecting_websocket_loop(
        partial(stream_new_blocks, WSS_URL, event_queue, False, 'polygon'),
        tag='new_blocks_stream'
    )
    
    event_handler_loop = event_handler(event_queue)
    
    loop = asyncio.get_event_loop()
    loop.run_until_complete(asyncio.wait([
        new_blocks_stream,
        event_handler_loop,
    ]))
    

if __name__ == '__main__':
    main()