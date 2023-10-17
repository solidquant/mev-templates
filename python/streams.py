import json
import eth_abi
import asyncio
import eth_utils
import websockets
import aioprocessing

from web3 import Web3
from loguru import logger
from typing import Dict, List

from pools import Pool, DexVariant
from multi import get_uniswap_v2_reserves
from utils import calculate_next_block_base_fee, estimated_next_block_gas


async def stream_new_blocks(wss_url: str,
                            event_queue: aioprocessing.AioQueue,
                            debug: bool = False,
                            chain: str = 'ethereum'):    
    async with websockets.connect(wss_url) as ws:
        wss = Web3.WebsocketProvider(wss_url)
        subscription = wss.encode_rpc_request('eth_subscribe', ['newHeads'])
        
        await ws.send(subscription)
        _ = await ws.recv()

        while True:
            try:
                msg = await asyncio.wait_for(ws.recv(), timeout=60 * 10)
                block = json.loads(msg)['params']['result']
                block_number = int(block['number'], base=16)
                base_fee = int(block['baseFeePerGas'], base=16)
                next_base_fee = calculate_next_block_base_fee(block)
                estimate_gas = await estimated_next_block_gas(chain)
                event = {
                    'type': 'block',
                    'block_number': block_number,
                    'base_fee': base_fee,
                    'next_base_fee': next_base_fee,
                    **estimate_gas,
                }
                if not debug:
                    event_queue.put(event)
                else:
                    logger.info(event)
            except Exception as e:
                print(e)


async def stream_pending_transactions(wss_url: str,
                                      event_queue: aioprocessing.AioQueue,
                                      debug: bool = False):
    async with websockets.connect(wss_url) as ws:
        wss = Web3.WebsocketProvider(wss_url)
        subscription = wss.encode_rpc_request('eth_subscribe', ['newPendingTransactions'])
        
        await ws.send(subscription)
        _ = await ws.recv()

        while True:
            msg = await asyncio.wait_for(ws.recv(), timeout=60 * 10)
            response = json.loads(msg)
            tx_hash = response['params']['result']
            event = {
                'type': 'pending_tx',
                'tx_hash': tx_hash
            }

            if not debug:
                event_queue.put(event)
            else:
                print(event)


async def stream_uniswap_v2_events(https_url: str,
                                   wss_url: str,
                                   pools: Dict[str, Pool],
                                   event_queue: aioprocessing.AioQueue,
                                   debug: bool = False):

    w3 = Web3(Web3.HTTPProvider(https_url))

    block_number = w3.eth.get_block_number()

    reserves = get_uniswap_v2_reserves(w3, pools)

    pools = {
        addr.lower(): pool for addr, pool in pools.items()
        if pool.version == DexVariant.UniswapV2
    }

    def _publish(block_number: int,
                 pool: Pool,
                 data: List[int] = []):

        if len(data) == 2:
            # initial publishing occurs without data(=Sync event data)
            reserves[pool.address][0] = data[0]
            reserves[pool.address][1] = data[1]

        reserve_update = {
            pool.token0: reserves[pool.address][0],
            pool.token1: reserves[pool.address][1],
        }

        pool_update = {
            'type': 'pool_update',
            'block_number': block_number,
            'pool': pool.address,
            'reserves': reserve_update,
        }

        if not debug:
            event_queue.put(pool_update)
        else:
            logger.info(pool_update)

    """
    Send initial reserve data so that price can be calculated even if the pool is idle
    """
    for address, pool in pools.items():
        _publish(block_number, pool)

    # Subscribe to Sync events from all the pools we input
    sync_event_selector = w3.keccak(text='Sync(uint112,uint112)').hex()

    async with websockets.connect(wss_url) as ws:
        wss = Web3.WebsocketProvider(wss_url)
        params = ['logs', {'topics': [sync_event_selector]}]
        subscription = wss.encode_rpc_request('eth_subscribe', params)

        await ws.send(subscription)
        _ = await ws.recv()

        while True:
            msg = await asyncio.wait_for(ws.recv(), timeout=60 * 10)
            event = json.loads(msg)['params']['result']
            address = event['address'].lower()

            if address in pools:
                block_number = int(event['blockNumber'], base=16)
                pool = pools[address]
                data = eth_abi.decode(
                    ['uint112', 'uint112'],
                    eth_utils.decode_hex(event['data'])
                )
                _publish(block_number, pool, data)


if __name__ == '__main__':
    import os
    import nest_asyncio
    from functools import partial
    from dotenv import load_dotenv

    from utils import reconnecting_websocket_loop
    from pools import load_all_pools_from_v2

    nest_asyncio.apply()

    load_dotenv(override=True)

    HTTPS_URL = os.getenv('HTTPS_URL')
    WSS_URL = os.getenv('WSS_URL')

    uniswap_v2_factory_addresses = ['0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac']
    uniswap_v2_factory_blocks = 10794229

    pools = load_all_pools_from_v2(HTTPS_URL, uniswap_v2_factory_addresses, uniswap_v2_factory_blocks, 50000)

    new_blocks_stream = reconnecting_websocket_loop(
        partial(stream_new_blocks, WSS_URL, None, True, 'ethereum'),
        tag='new_blocks_stream'
    )

    pending_transactions_stream = reconnecting_websocket_loop(
        partial(stream_pending_transactions, WSS_URL, None, True),
        tag='pending_transactions_stream'
    )

    # uniswap_v2_stream = reconnecting_websocket_loop(
    #     partial(stream_uniswap_v2_events, HTTPS_URL, WSS_URL, pools, None, True),
    #     tag='uniswap_v2_stream'
    # )

    """
    Issue:
    An error has occurred with uniswap_v2_stream websocket: As of 3.10, the *loop* parameter was removed from Lock() since it is no longer necessary
    
    Solution:
    pip install --upgrade websockets
    """
    loop = asyncio.get_event_loop()
    loop.run_until_complete(asyncio.wait([
        new_blocks_stream,
        # pending_transactions_stream,
        # uniswap_v2_stream,
    ]))
