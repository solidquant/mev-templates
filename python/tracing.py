import json
import aiohttp
import aioprocessing

from loguru import logger
from typing import List, Optional


async def get_geth_touched_pools(https_url: str, tx_hash: str) -> Optional[List[str]]:
    async with aiohttp.ClientSession() as session:
        req = {
            'id': 1,
            'method': 'debug_traceTransaction',
            'jsonrpc': '2.0',
            'params': [
                tx_hash,
                {'tracer': 'prestateTracer'}
            ]
        }
        headers = {
            'Content-Type': 'application/json'
        }
        request = await session.post(https_url, data=json.dumps(req), headers=headers)
        res = await request.json()

        result = res.get('result')

        if result:
            addresses_touched = list(result.keys())
        else:
            addresses_touched = []

        return addresses_touched


async def get_parity_touched_pools(https_url: str, tx_hash: str) -> Optional[List[str]]:
    async with aiohttp.ClientSession() as session:
        req = {
            'id': 1,
            'method': 'trace_replayTransaction',
            'jsonrpc': '2.0',
            'params': [tx_hash, ['stateDiff']]
        }
        request = await session.post(https_url, data=json.dumps(req))
        res = await request.json()

        result = res.get('result')

        if result:
            state_diff = result['stateDiff']
            addresses_touched = list(state_diff.keys())
        else:
            addresses_touched = []

        return addresses_touched


# TEST event_handler
async def test_event_handler(https_url: str, event_queue: aioprocessing.AioQueue):
    import time

    while True:
        data = await event_queue.coro_get()

        if data['type'] == 'pending_tx':
            s = time.time()
            geth_touched_pools = await get_geth_touched_pools(https_url, data['tx_hash'])
            parity_touched_pools = await get_parity_touched_pools(https_url, data['tx_hash'])
            e = time.time()

            logger.info(f'{data["tx_hash"]}: took {e - s} sec')
            logger.info(geth_touched_pools)
            logger.info(parity_touched_pools)


if __name__ == '__main__':
    """
    Reference:
    
    https://medium.com/@solidquant/how-i-spend-my-days-mempool-watching-part-1-transaction-prediction-through-evm-tracing-77f4c99207f
    """
    import os
    import asyncio
    import nest_asyncio
    from functools import partial
    from dotenv import load_dotenv

    from utils import reconnecting_websocket_loop
    from streams import stream_pending_transactions

    nest_asyncio.apply()

    load_dotenv(override=True)

    HTTPS_URL = os.getenv('HTTPS_URL')
    WSS_URL = os.getenv('WSS_URL')

    event_queue = aioprocessing.AioQueue()

    # Start the mempool stream
    pending_transactions_stream = reconnecting_websocket_loop(
        partial(stream_pending_transactions, WSS_URL, event_queue, False),
        tag='pending_transactions_stream'
    )

    event_handler = test_event_handler(HTTPS_URL, event_queue)

    loop = asyncio.get_event_loop()
    loop.run_until_complete(asyncio.wait([
        pending_transactions_stream,
        event_handler,
    ]))
