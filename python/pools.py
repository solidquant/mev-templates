import os
import csv
import web3
import json

from tqdm import tqdm
from enum import Enum
from web3 import Web3
from time import sleep
from random import randint
from typing import Dict, List, Optional
from concurrent.futures import ThreadPoolExecutor, as_completed

from constants import *  

RATE_LIMIT = 10  # Limit requests, adjust this to rate limit 


class DexVariant(Enum):
    UniswapV2 = 2
    UniswapV3 = 3


class Pool:

    def __init__(self,
                 address: str,
                 version: DexVariant,
                 token0: str,
                 token1: str,
                 decimals0: int,
                 decimals1: int,
                 fee: int):

        self.address = address
        self.version = version
        self.token0 = token0
        self.token1 = token1
        self.decimals0 = decimals0
        self.decimals1 = decimals1
        self.fee = fee

    def cache_row(self):
        return [
            self.address,
            self.version.value,
            self.token0,
            self.token1,
            self.decimals0,
            self.decimals1,
            self.fee,
        ]
        

def fetch_events(params: tuple,
                 factory_address: str,
                 v2_factory: web3.contract.Contract):
    sleep(randint(10, 50) / 100.0)  # Adding some jitter to avoid rate-limiting
    try:
        events = v2_factory.events.PairCreated.get_logs(fromBlock=params[0], toBlock=params[1])
        return [(factory_address, event) for event in events]
    except Exception as e:
        print(f'Error fetching events: {e}')
        return []


def load_cached_pools() -> Optional[Dict[str, Pool]]:
    if os.path.exists(CACHED_POOLS_FILE):
        f = open(CACHED_POOLS_FILE, 'r')
        rdr = csv.reader(f)
        pools = {}

        for row in rdr:
            if row[0] == 'address':
                continue
            version = DexVariant.UniswapV2 if row[1] == '2' else DexVariant.UniswapV3
            pool = Pool(address=row[0],
                        version=version,
                        token0=row[2],
                        token1=row[3],
                        decimals0=int(row[4]),
                        decimals1=int(row[5]),
                        fee=int(row[6]))
            pools[row[0]] = pool
        logger.info(f'Loaded pools from cache: {CACHED_POOLS_FILE} ({len(pools)} pools)')

        return pools


def cache_synced_pools(pool: Pool):
    if os.path.exists(CACHED_POOLS_FILE):
        f = open(CACHED_POOLS_FILE, 'r')
        rdr = csv.reader(f)
        existing_pools = [row[0] for row in rdr]
        f.close()
        if pool.address in existing_pools:
            return
        f = open(CACHED_POOLS_FILE, 'a', newline='')
    else:
        f = open(CACHED_POOLS_FILE, 'w', newline='')
        wr = csv.writer(f)
        columns = ['address', 'version', 'token0', 'token1', 'decimals0', 'decimals1', 'fee']
        wr.writerow(columns)

    wr = csv.writer(f)  
    wr.writerow(pool.cache_row())
    f.close()


def load_all_pools_from_v2(https_url: str,
                           factory_addresses: List[str],
                           from_blocks: List[int],
                           chunk: int = 100000) -> Dict[str, Pool]:

    # Load cached pools
    pools = load_cached_pools()
    
    # If cached pools exist, skip the fetching part
    if pools is not None:
        print("Pools already exist in cache. Skipping fetching.")
        return pools
    
    pools = pools or {}
    v2_factory_abi = json.load(open(ABI_PATH / 'UniswapV2Factory.json', 'r'))
    erc20_abi = json.load(open(ABI_PATH / 'ERC20.json', 'r'))
    w3 = Web3(Web3.HTTPProvider(https_url))
    to_block = w3.eth.get_block_number()
    decimals: Dict[str, int] = {}

    with ThreadPoolExecutor(max_workers=RATE_LIMIT) as executor:
        rate_limit_futures = []

        for i in range(len(factory_addresses)):
            factory_address = factory_addresses[i]
            from_block = from_blocks[i]
            v2_factory = w3.eth.contract(address=factory_address, abi=v2_factory_abi)

            block_range = list(range(from_block, to_block, chunk))
            request_params = [(block_range[i], block_range[i + 1]) for i in range(len(block_range) - 1)]

            for params in request_params:
                future = executor.submit(fetch_events, params, factory_address, v2_factory)
                rate_limit_futures.append(future)

        with tqdm(total=len(rate_limit_futures), desc='Processing events', ascii=' =', leave=True) as pbar:
            for future in as_completed(rate_limit_futures):
                for factory_address, event in future.result():
                    args = event.args
                    token0 = args.token0
                    token1 = args.token1

                    try:
                        if token0 in decimals:
                            decimals0 = decimals[token0]
                        else:
                            token0_contract = w3.eth.contract(address=token0, abi=erc20_abi)
                            decimals0 = token0_contract.functions.decimals().call()
                            decimals[token0] = decimals0

                        if token1 in decimals:
                            decimals1 = decimals[token1]
                        else:
                            token1_contract = w3.eth.contract(address=token1, abi=erc20_abi)
                            decimals1 = token1_contract.functions.decimals().call()
                            decimals[token1] = decimals1
                    except Exception as _:
                        continue

                    pool = Pool(address=args.pair,
                                version=DexVariant.UniswapV2,
                                token0=args.token0,
                                token1=args.token1,
                                decimals0=decimals0,
                                decimals1=decimals1,
                                fee=300)
                    if args.pair not in pools:
                        pools[args.pair] = pool
                        cache_synced_pools(pool)

                pbar.update(1)

    return pools


if __name__ == '__main__':
    """
    Example code can be run on Ethereum
    """
    factory_addresses = [
        '0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac', # Uniswap v2
    ]
    factory_blocks = [
        10794229,
    ]

    pools = load_all_pools_from_v2(HTTPS_URL, factory_addresses, factory_blocks, 100000)
    print(pools)