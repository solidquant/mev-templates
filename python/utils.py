import json
import random
import aiohttp
import asyncio
import eth_abi
import websockets

from web3 import Web3
from typing import Any, Callable, Dict, List

from constants import BLOCKNATIVE_TOKEN

GWEI = 10 ** 9


async def reconnecting_websocket_loop(stream_fn: Callable, tag: str):
    while True:
        try:
            await stream_fn()

        except (websockets.ConnectionClosedError, websockets.ConnectionClosedOK) as e:
            print(f'{tag} websocket connection closed: {e}')
            print('Reconnecting...')
            await asyncio.sleep(2)

        except Exception as e:
            print(f'An error has occurred with {tag} websocket: {e}')
            await asyncio.sleep(2)


def calculate_next_block_base_fee(block: Dict[str, Any]) -> int:
    try:
        base_fee = int(block['baseFeePerGas'], base=16)
        gas_used = int(block['gasUsed'], base=16)
        gas_limit = int(block['gasLimit'], base=16)
    except TypeError:
        # used in benchmarks
        base_fee = block['baseFeePerGas']
        gas_used = block['gasUsed']
        gas_limit = block['gasLimit']

    target_gas_used = gas_limit / 2
    target_gas_used = 1 if target_gas_used == 0 else target_gas_used

    if gas_used > target_gas_used:
        new_base_fee = base_fee + \
            ((base_fee * (gas_used - target_gas_used)) / target_gas_used) / 8
    else:
        new_base_fee = base_fee - \
            ((base_fee * (target_gas_used - gas_used)) / target_gas_used) / 8

    return int(new_base_fee + random.randint(0, 9))


async def estimated_next_block_gas(chain: str = 'ethereum') -> Dict[str, float]:
    """
    This function does not run if the environment variable for BLOCKNATIVE_TOKEN is left blank
    """
    estimate = {}
    if BLOCKNATIVE_TOKEN:
        chain_id = 1 if chain == 'ethereum' else 137
        headers = {'Authorization': BLOCKNATIVE_TOKEN}
        async with aiohttp.ClientSession(headers=headers) as session:
            async with session.get(f'https://api.blocknative.com/gasprices/blockprices?chainid={chain_id}') as r:
                res = await r.json()
                estimated_price = res['blockPrices'][0]['estimatedPrices'][0]

                estimate['max_priority_fee_per_gas'] = int(estimated_price['maxPriorityFeePerGas'] * GWEI)
                estimate['max_fee_per_gas'] = int(estimated_price['maxFeePerGas'] * GWEI)
    return estimate


async def get_access_list(tx: Dict[str, Any], https_url: str) -> List[Dict[str, Any]]:
    """
    Reference: https://www.rareskills.io/post/eip-2930-optional-access-list-ethereum
    """
    access_list = []
    async with aiohttp.ClientSession() as session:
        async with session.post(
            url=https_url,
            headers={'content-type': 'application/json'},
            data=json.dumps({
                'id': 1,
                'method': 'eth_createAccessList',
                'jsonrpc': '2.0',
                'params': [{
                    'from': tx.get('from'),
                    'to': tx.get('to'),
                    'data': tx.get('data', '0x'),
                    'value': hex(tx.get('value', 0)),
                    'gas': hex(500000)
                }, 'latest']
            })
        ) as r:
            res = await r.json()
            access_list = res['result']['accessList']
    return access_list


def get_touched_pool_reserves(w3: Web3, block_number: int) -> Dict[str, List[int]]:
    """
    Whenever a new block is created, you can retrieve all the logs from that new block.
    We get all the Sync events from the new block and see what pools were touched.
    This makes it easier for us to calculate price spread & simulate price impact.
    """
    sync_event_selector = w3.keccak(text='Sync(uint112,uint112)').hex()
    event_filter = w3.eth.filter({
        'fromBlock': block_number,
        'toBlock': block_number,
        'topics': [sync_event_selector]
    })
    logs = event_filter.get_all_entries()
    tx_idx = {}
    reserves = {}
    for log in logs:
        if sync_event_selector == log['topics'][0].hex():
            address = log['address']
            idx = log['transactionIndex']
            prev_tx_idx = tx_idx.get(address, 0)
            if idx >= prev_tx_idx:
                decoded = eth_abi.decode(['uint112', 'uint112'], bytes.fromhex(log['data'][2:]))
                reserves[address] = list(decoded)
                tx_idx[address] = idx
    return reserves


if __name__ == '__main__':
    import asyncio
    from web3 import Web3
    from bundler import Bundler, Path, Flashloan
    from constants import (
        HTTPS_URL,
        PRIVATE_KEY,
        SIGNING_KEY,
        BOT_ADDRESS,
    )
    
    estimated_gas = asyncio.run(estimated_next_block_gas('polygon'))
    print(estimated_gas)
    
    bundler = Bundler(PRIVATE_KEY, SIGNING_KEY, HTTPS_URL, BOT_ADDRESS)
    
    balancer_vault = '0xBA12222222228d8Ba445958a75a0704d566BF2C8'
    uniswap_v2_router = '0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F'
    weth_address = '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2'
    usdc_address = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48'
    
    GWEI = 10 ** 9
    
    paths = [
        Path(uniswap_v2_router, weth_address, usdc_address)    
    ]
    amount_in = Web3.to_wei(1, 'ether')
    order_tx = bundler.order_tx(paths,
                                amount_in,
                                50 * GWEI,
                                100 * GWEI,
                                Flashloan.Balancer,
                                balancer_vault)
    
    al = asyncio.run(get_access_list(order_tx, HTTPS_URL))
    print(al)
    
    print('\n')
    
    transfer_in_tx = bundler.transfer_in_tx(amount_in=1,
                                            max_priority_fee_per_gas=50 * GWEI,
                                            max_fee_per_gas=100 * GWEI)
    al = asyncio.run(get_access_list(transfer_in_tx, HTTPS_URL))
    print(al)
    """
    Output: [
        {
            'address': '0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270',
            'storageKeys': ['0x4a6e2ef050e40bf25d23b5186844bf84cd8f55b0f5b8c7f700ad16e66f71a5e0']
        }
    ]
    """
    
    print('\n')
    
    w3 = Web3(Web3.HTTPProvider(HTTPS_URL))
    block_number = w3.eth.get_block_number()
    touched_pools = get_touched_pool_reserves(w3, block_number)
    print(touched_pools)
