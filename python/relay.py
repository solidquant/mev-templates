import asyncio

from web3 import Web3
from uuid import uuid4
from flashbots import Flashbots
from typing import Any, Dict, List
from web3.exceptions import TransactionNotFound
from flashbots.flashbots import FlashbotsBundleResponse


async def send_bundle(w3: Web3,
                      bundle: List[Dict[str, Any]],
                      retry: int,
                      block_number: int = None) -> list:

    flashbots: Flashbots = w3.flashbots

    left_retries = retry

    if not block_number:
        block_number = w3.eth.block_number

    receipts = []

    while left_retries >= 0:
        print(f'Sending bundles at: #{block_number}')
        try:
            flashbots.simulate(bundle, block_number)
        except Exception as e:
            print('Simulation error', e)
            break

        replacement_uuid = str(uuid4())
        response: FlashbotsBundleResponse = flashbots.send_bundle(
            bundle,
            target_block_number=block_number + 1,
            opts={'replacementUuid': replacement_uuid},
        )

        while w3.eth.block_number < response.target_block_number:
            await asyncio.sleep(1)

        try:
            receipts = list(
                map(lambda tx: w3.eth.get_transaction_receipt(tx['hash']), response.bundle)
            )
            print(f'\nBundle was mined in block {receipts[0].blockNumber}\a')
            break
        except TransactionNotFound:
            print(f'Bundle not found in block {block_number + 1}')
            flashbots.cancel_bundles(replacement_uuid)
            left_retries -= 1
            block_number += 1

    return receipts