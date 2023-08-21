import csv
import time
import math
import multiprocessing

from tqdm import tqdm
from enum import Enum
from web3 import Web3
from multicall import Call, Multicall
from typing import Dict, List, Optional

from constants import *


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
                 symbol0: str,
                 symbol1: str,
                 fee: int):

        self.address = address
        self.version = version
        self.token0 = token0
        self.token1 = token1
        self.decimals0 = decimals0
        self.decimals1 = decimals1
        self.symbol0 = symbol0
        self.symbol1 = symbol1
        self.fee = fee

    def cache_row(self):
        return [
            self.address,
            self.version.value,
            self.token0,
            self.token1,
            self.decimals0,
            self.decimals1,
            self.symbol0,
            self.symbol1,
            self.fee,
        ]


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
                        symbol0=row[6],
                        symbol1=row[7],
                        fee=int(row[8]))
            pools[row[0]] = pool
        logger.info(f'Loaded pools from cache: {CACHED_POOLS_FILE} ({len(pools)} pools)')

        return pools


def cache_synced_pools(pools: Dict[str, Pool]):
    f = open(CACHED_POOLS_FILE, 'w', newline='')
    wr = csv.writer(f)
    columns = ['address', 'version', 'token0', 'token1', 'decimals0', 'decimals1', 'symbol0', 'symbol1', 'fee']
    wr.writerow(columns)
    for _, pool in pools.items():
        wr.writerow(pool.cache_row())
    f.close()
    logger.info(f'Saved pools to: {CACHED_POOLS_FILE} ({len(pools)} pools)')


def _load_pools(https_url: str, factory_address: str, from_block: int, to_block: int):
    w3 = Web3(Web3.HTTPProvider(https_url))
    v2_factory_abi = json.load(open(ABI_PATH / 'UniswapV2Factory.json', 'r'))
    v2_factory = w3.eth.contract(address=factory_address, abi=v2_factory_abi)
    events = v2_factory.events.PairCreated.get_logs(fromBlock=from_block, toBlock=to_block)
    pool_addresses: Dict[str, List[str]] = {}
    for event in events:
        args = event.args
        pool_addresses[args.pair] = [args.token0, args.token1]
    return pool_addresses


def _batch_get_token_info(https_url: str, tokens: List[str]):
    w3 = Web3(Web3.HTTPProvider(https_url))
    decimals_sig = 'decimals()(uint8)'
    symbol_sig = 'symbol()(string)'

    calls = []
    for token in tokens:
        calls.append(Call(token, decimals_sig, [(f'{token}_decimals', lambda x: x)]))
        calls.append(Call(token, symbol_sig, [(f'{token}_symbol', lambda x: x)]))

    multicall = Multicall(calls, require_success=False, _w3=w3)
    result = multicall()

    decimals = {}
    symbols = {}

    for k, v in result.items():
        addr = k.split('_')[0]
        data_type = k.split('_')[1]

        if data_type == 'decimals':
            decimals[addr] = v

        if data_type == 'symbol':
            symbols[addr] = v

    return {'decimals': decimals, 'symbols': symbols}


def load_all_pools_from_v2(https_url: str,
                           factory_addresses: List[str],
                           from_blocks: List[int],
                           chunk: int = 2000) -> Dict[str, Pool]:
    """
    This function will retrieve all PairCreated events from factory_addresses, starting from from_blocks.
    The request will be made to look at events that occur in a chunk number of blocks every call.
    """
    # pools = load_cached_pools()
    # if pools:
    #     return pools

    w3 = Web3(Web3.HTTPProvider(https_url))
    to_block = w3.eth.get_block_number()

    decimals: Dict[str, int] = {}
    symbols: Dict[str, str] = {}

    raw_pools = {}
    pools = {}

    for i in range(len(factory_addresses)):
        factory_address = factory_addresses[i]
        from_block = from_blocks[i]

        block_range = list(range(from_block, to_block, chunk))
        request_params = [(block_range[i], block_range[i + 1]) for i in range(len(block_range) - 1)]

        mp = multiprocessing.Pool()

        args = []

        for params in request_params:
            args.append((https_url, factory_address, params[0], params[1]))

        batch_num = 10
        batch_size = math.ceil(len(args) / batch_num)
        batch_range = range(0, batch_num)
        for batch in tqdm(batch_range, total=len(batch_range), ncols=100, desc=f'PairCreated logs', ascii=' =', leave=True):
            _from = batch * batch_size
            _to = min(_from + batch_size, len(args))
            _args = args[_from:_to]

            results = mp.starmap(_load_pools, _args)

            for result in results:
                for pool_address, tokens in result.items():
                    raw_pools[pool_address] = tokens
                    if tokens[0] not in decimals:
                        decimals[tokens[0]] = 0
                        symbols[tokens[0]] = ''
                    if tokens[1] not in decimals:
                        decimals[tokens[1]] = 0
                        symbols[tokens[1]] = ''

    mp = multiprocessing.Pool()

    tokens_cnt = len(decimals)
    batch = math.ceil(tokens_cnt / 100)
    tokens_per_batch = math.ceil(tokens_cnt / batch)

    args = []

    for i in range(batch):
        start_idx = i * tokens_per_batch
        end_idx = min(start_idx + tokens_per_batch, tokens_cnt)
        args.append((https_url, list(decimals.keys())[start_idx:end_idx]))

    token_info = mp.starmap(_batch_get_token_info, args)
    print(token_info)

    # cache_synced_pools(pools)
    return pools


if __name__ == '__main__':
    factory_addresses = [
        '0xc35DADB65012eC5796536bD9864eD8773aBc74C4',  # Sushiswap V2 (Polygon)
    ]
    factory_blocks = [
        # 11333218,
        17000000
    ]

    pools = load_all_pools_from_v2(HTTPS_URL, factory_addresses, factory_blocks, 100000)
    print(pools)