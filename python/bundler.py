import eth_abi

from web3 import Web3
from enum import Enum
from loguru import logger
from flashbots import flashbot
from typing import Any, Dict, List
from eth_account.account import Account

from relay import send_bundle
from constants import (
    BOT_ABI,
    PRIVATE_RELAY,
)

ZERO_ADDRESS = '0x0000000000000000000000000000000000000000'


class Path:
    
    def __init__(self,
                 router: str,
                 token_in: str,
                 token_out: str):
        self.router = router
        self.token_in = token_in
        self.token_out = token_out
        
    def to_list(self):
        return [self.router, self.token_in, self.token_out]
    
    
class Flashloan(Enum):
    NotUsed = 0
    Balancer = 1
    UniswapV2 = 2


class Bundler:
    
    def __init__(self,
                 private_key: str,
                 signing_key: str,
                 https_url: str,
                 bot_address: str):
        
        self.sender = Account.from_key(private_key)  # owner of contract
        self.signer = Account.from_key(signing_key)  # flashbots repuatation
        
        self.w3 = Web3(Web3.HTTPProvider(https_url))
        flashbot(self.w3, self.signer, PRIVATE_RELAY)
        self.chain_id = self.w3.eth.chain_id
        self.bot = self.w3.eth.contract(address=bot_address, abi=BOT_ABI)
        
    def to_bundle(self, tx: Dict[str, Any]) -> List[Dict[str, Any]]:
        signed = self.sender.sign_transaction(tx)
        return [{'signed_transaction': signed.rawTransaction}]
    
    async def send_bundle(self,
                          bundle: List[Dict[str, Any]],
                          retry: int,
                          block_number: int = None):
        await send_bundle(self.w3, bundle, retry, block_number)
        
    def send_tx(self, transaction: Dict[str, Any]) -> str:
        signed = self.sender.sign_transaction(transaction)
        tx_hash = self.w3.eth.send_raw_transaction(signed.rawTransaction)
        logger.info(tx_hash)
        return tx_hash
        
    @property
    def _common_fields(self) -> Dict[str, any]:
        nonce = self.w3.eth.get_transaction_count(self.sender.address)
        return {
            'from': self.sender.address,
            'nonce': nonce,
            'chainId': self.chain_id
        }
    
    def transfer_in_tx(self,
                       amount_in: int,
                       max_priority_fee_per_gas: float,
                       max_fee_per_gas: float) -> Dict[str, Any]:
        """
        Transfer ETH/MATIC (mainCurrency) to bot contract
        The contract, upon receiving, will wrap the token into ERC20 token.
        """
        return {
            **self._common_fields,
            'to': self.bot.address,
            'value': Web3.to_wei(amount_in, 'ether'),
            'gas': 60000,
            'maxFeePerGas': max_fee_per_gas,
            'maxPriorityFeePerGas': max_priority_fee_per_gas,
        }

    def transfer_out_tx(self,
                        token: str,
                        max_priority_fee_per_gas: float,
                        max_fee_per_gas: float) -> Dict[str, Any]:
        """
        Recovers the specified token balance by calling "recoverToken(address)"
        """
        return self.bot.functions.recoverToken(token).build_transaction({
            **self._common_fields,
            'gas': 50000,
            'maxFeePerGas': max_fee_per_gas,
            'maxPriorityFeePerGas': max_priority_fee_per_gas,
        })

    def approve_tx(self,
                   router: str,
                   tokens: List[str],
                   force: bool = True,
                   max_priority_fee_per_gas: float = 0,
                   max_fee_per_gas: float = 0) -> Dict[str, Any]:
        """
        Approves the use of speicifed tokens to router.
        You can either force the approval or skip if already approved.
        This transaction calls "approveRouter(address, address[], bool)"
        """
        return self.bot.functions.approveRouter(
            router, tokens, force
        ).build_transaction({
            **self._common_fields,
            'gas': 55000 * len(tokens),
            'maxFeePerGas': max_fee_per_gas,
            'maxPriorityFeePerGas': max_priority_fee_per_gas,
        })

    def order_tx(self,
                 paths: List[Path],
                 amount_in: int,
                 max_priority_fee_per_gas: float,
                 max_fee_per_gas: float,
                 flashloan: Flashloan = Flashloan.NotUsed,
                 loan_from: str = ZERO_ADDRESS) -> Dict[str, Any]:
        """
        Sends an order transaction to the contract.
        There is only a "fallback" function for this.
        
        :param amount_in: should be int(amount * 10 ** decimals)
        
        We set the default as not using any flashloans.
        If you intend to use:
        
        1. Balancer flashloan: pass in
            - flashloan: Flashloan.Balancer
            - loan_from: Valut address
            
        2. Uniswap V2 flashswap: pass in
            - flashloan: Flashloan.UniswapV2
            - loan_from: UniswapV2Pair address
        """
        nhop = len(paths)
        
        calldata_types = ['uint', 'uint', 'address']
        path_types = ['address', 'address', 'address'] * nhop
        calldata_types = calldata_types + path_types
        
        calldata_raw = [amount_in, flashloan.value, loan_from]
        
        for path in paths:
            calldata_raw.extend(path.to_list())
            
        calldata = eth_abi.encode(calldata_types, calldata_raw)
        
        return {
            **self._common_fields,
            'to': self.bot.address,
            'value': 0,
            'data': '0x' + calldata.hex(),
            'gas': 600000,
            'maxFeePerGas': max_fee_per_gas,
            'maxPriorityFeePerGas': max_priority_fee_per_gas,
        }


if __name__ == '__main__':
    """
    Real TXs tested on Polygon mainnet
    """
    from constants import HTTPS_URL
    
    # addresses generated by: https://vanity-eth.tk/
    private_key = 'b3e5dc08b18918cce982438a28877e440aafc01fef4c314b95d0609bf946585f'
    signing_key = '34f55bef77aca52be9f7506da40205f8ecd7e863fd3b465a5db9950247422caf'
    
    bot_address = '0xEc1f2DADF368D5a20D494a2974bC19e421812017'
    
    bundler = Bundler(private_key, signing_key, HTTPS_URL, bot_address)
    
    GWEI = 10 ** 9
    
    # Transfer to bot contract tx
    transfer_in_tx = bundler.transfer_in_tx(amount_in=1,
                                            max_priority_fee_per_gas=50 * GWEI,
                                            max_fee_per_gas=100 * GWEI)
    print('Transfer in tx: ', transfer_in_tx)
    
    # Transfer out from bot contract tx
    weth_address = '0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619'
    usdc_address = '0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174'
    transfer_out_tx = bundler.transfer_out_tx(token=usdc_address,
                                              max_priority_fee_per_gas=50 * GWEI,
                                              max_fee_per_gas=100 * GWEI)
    print('Transfer out tx: ', transfer_out_tx)
    
    # Approve routers tx
    sushiswap_v2_router = '0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506'
    tokens = [weth_address, usdc_address]
    approve_tx = bundler.approve_tx(router=sushiswap_v2_router,
                                    tokens=tokens,
                                    force=False,
                                    max_priority_fee_per_gas=50 * GWEI,
                                    max_fee_per_gas=100 * GWEI)
    print('Approve tx: ', approve_tx)
    
    # Order tx
    
    # A 2-hop path: WETH --> USDC --> WETH
    paths = [
        Path(sushiswap_v2_router, weth_address, usdc_address),
        Path(sushiswap_v2_router, usdc_address, weth_address)
    ]
    amount_in = Web3.to_wei(1, 'ether')
    balancer_vault = '0xBA12222222228d8Ba445958a75a0704d566BF2C8'
    
    order_tx = bundler.order_tx(paths=paths,
                                amount_in=amount_in,
                                max_priority_fee_per_gas=50 * GWEI,
                                max_fee_per_gas=100 * GWEI,
                                flashloan=Flashloan.Balancer,
                                loan_from=balancer_vault)
    print('Order tx: ', order_tx)
    
    # Bundle tx
    bundle = bundler.to_bundle(order_tx)
    print(bundle)
    
    # Sending real transactions
    import time
    import asyncio
    
    from constants import logger, PRIVATE_KEY, SIGNING_KEY, BOT_ADDRESS
    
    from pools import load_all_pools_from_v2
    from paths import generate_triangular_paths
    from multi import batch_get_uniswap_v2_reserves
    from utils import estimated_next_block_gas
    
    bundler = Bundler(PRIVATE_KEY, SIGNING_KEY, HTTPS_URL, BOT_ADDRESS)
    
    sushiswap_v2_factory_address = '0xc35DADB65012eC5796536bD9864eD8773aBc74C4'
    sushiswap_v2_factory_block = 11333218

    w3 = Web3(Web3.HTTPProvider(HTTPS_URL))
    pools = load_all_pools_from_v2(HTTPS_URL, sushiswap_v2_factory_address, sushiswap_v2_factory_block, 50000)
    
    usdt_address = '0xc2132D05D31c914a87C6611C10748AEb04B58e8F'
    paths = generate_triangular_paths(pools, usdt_address)
    
    # Filter pools that were used in arb paths
    pools = {}
    for path in paths:
        pools[path.pool_1.address] = path.pool_1
        pools[path.pool_2.address] = path.pool_2
        pools[path.pool_3.address] = path.pool_3

    reserves = batch_get_uniswap_v2_reserves(HTTPS_URL, pools)

    path = paths[0]
    sushiswap_v2_router = '0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506'
    
    swap_paths = []
    
    for i in range(path.nhop):
        pool = getattr(path, f'pool_{i + 1}')
        zero_for_one = getattr(path, f'zero_for_one_{i + 1}')
        if zero_for_one:
            token_in, token_out = pool.token0, pool.token1
        else:
            token_in, token_out = pool.token1, pool.token0
        swap_path = Path(sushiswap_v2_router, token_in, token_out)
        swap_paths.append(swap_path)
        
    amount_in = 1 * 10 ** 6
    
    estimated_gas = asyncio.run(estimated_next_block_gas('polygon'))
    
    order_tx = bundler.order_tx(swap_paths,
                                amount_in,
                                estimated_gas['max_priority_fee_per_gas'] * 2,
                                estimated_gas['max_fee_per_gas'] * 2,
                                Flashloan.Balancer,
                                balancer_vault)
    tx_hash = bundler.send_tx(order_tx)
    print(tx_hash)
    
    while True:
        tx = w3.eth.get_transaction(tx_hash)
        print(tx)
        time.sleep(0.5)
