import os
import json

from pathlib import Path
from loguru import logger
from dotenv import load_dotenv

load_dotenv(override=True)

HTTPS_URL = os.getenv('HTTPS_URL')
WSS_URL = os.getenv('WSS_URL')
BLOCKNATIVE_TOKEN = os.getenv('BLOCKNATIVE_TOKEN')
PRIVATE_KEY = os.getenv('PRIVATE_KEY')
SIGNING_KEY = os.getenv('SIGNING_KEY')
BOT_ADDRESS = os.getenv('BOT_ADDRESS')

_DIR = Path(os.path.dirname(os.path.abspath(__file__)))

ABI_PATH = _DIR / 'abi'
CACHED_POOLS_FILE = _DIR / '.cached-pools.csv'

# retrieved from Etherscan
ERC20_ABI = json.load(open(ABI_PATH / 'ERC20.json', 'r'))
UNISWAP_V2_FACTORY_ABI = json.load(open(ABI_PATH / 'UniswapV2Factory.json', 'r'))
WETH_ABI = json.load(open(ABI_PATH / 'WETH.json', 'r'))

# compiled using Foundry
BOT_ABI = json.load(open(ABI_PATH / 'V2ArbBot.json', 'r'))['abi']

PRIVATE_RELAY = 'https://relay.flashbots.net'
# PRIVATE_RELAY = 'https://bor.txrelay.marlin.org/'
