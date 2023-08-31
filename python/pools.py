import csv
import json

from tqdm import tqdm
from enum import Enum
from web3 import Web3
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
        f = open(CACHED_POOLS_FILE, 'a', newline='')
    else:
        f = open(CACHED_POOLS_FILE, 'w', newline='')
        wr = csv.writer(f)
        columns = ['address', 'version', 'token0', 'token1', 'decimals0', 'decimals1', 'fee']
        wr.writerow(columns)

    wr = csv.writer(f)  
    wr.writerow(pool.cache_row())
    f.close()
    logger.info(f'Saved pool to: {CACHED_POOLS_FILE}')



def load_all_pools_from_v2(https_url: str,
                           factory_addresses: List[str],
                           from_blocks: List[int],
                           chunk: int = 2000) -> Dict[str, Pool]:
    """
    This function will retrieve all PairCreated events from factory_addresses, starting from from_blocks.
    The request will be made to look at events that occur in a chunk number of blocks every call.
    """
    pools = load_cached_pools() or {}
    
    v2_factory_abi = json.load(open(ABI_PATH / 'UniswapV2Factory.json', 'r'))
    erc20_abi = json.load(open(ABI_PATH / 'ERC20.json', 'r'))
    
    w3 = Web3(Web3.HTTPProvider(https_url))
    to_block = w3.eth.get_block_number()
    
    decimals: Dict[str, int] = {}

    for i in range(len(factory_addresses)):
        factory_address = factory_addresses[i]
        from_block = from_blocks[i]
        
        v2_factory = w3.eth.contract(address=factory_address, abi=v2_factory_abi)

        block_range = list(range(from_block, to_block, chunk))
        request_params = [(block_range[i], block_range[i + 1]) for i in range(len(block_range) - 1)]

        for params in tqdm(request_params,
                           total=len(request_params),
                           ncols=100,
                           desc=f'Uniswap V2 {factory_address[:10]}... Sync',
                           ascii=' =',
                           leave=True):
            events = v2_factory.events.PairCreated.get_logs(fromBlock=params[0], toBlock=params[1])
            for event in events:
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
                    # some token contracts don't exist anymore: eth_call error
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

    return pools


if __name__ == '__main__':
    factory_addresses = [
        '0xc35DADB65012eC5796536bD9864eD8773aBc74C4',  # Sushiswap V2 (Polygon)
        '0x5757371414417b8C6CAad45bAeF941aBc7d3Ab32',  # Uniswap V2 (Polygon)
    ]
    factory_blocks = [
        11333218,
        11799757,
    ]

    pools = load_all_pools_from_v2(HTTPS_URL, factory_addresses, factory_blocks, 100000)
    print(pools)
