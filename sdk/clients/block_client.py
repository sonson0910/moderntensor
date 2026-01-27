"""
Block Client
Handles block and chain information queries.
"""

import logging
from typing import Optional, Dict, Any, List, Callable
from .base import BaseRpcClient

logger = logging.getLogger(__name__)


class BlockClient(BaseRpcClient):
    """
    Client for block and chain information.
    Single Responsibility: Block/chain queries only.
    """

    def get_block_number(self) -> int:
        """
        Get current block height.

        Returns:
            Current block number
        """
        result = self._call_rpc("eth_blockNumber")
        return self._hex_to_int(result)

    def get_block(self, block_number: Optional[int] = None) -> Dict[str, Any]:
        """
        Get block by number.

        Args:
            block_number: Block number (None for latest)

        Returns:
            Block data
        """
        block_param = f"0x{block_number:x}" if block_number is not None else "latest"
        return self._call_rpc("eth_getBlockByNumber", [block_param, True])

    def get_block_hash(self, block_number: int) -> str:
        """
        Get block hash by number.

        Args:
            block_number: Block number

        Returns:
            Block hash
        """
        block = self.get_block(block_number)
        return block.get("hash", "") if block else ""

    def get_block_by_hash(self, block_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get block by hash.

        Args:
            block_hash: Block hash (0x...)

        Returns:
            Block data or None
        """
        return self._call_rpc("eth_getBlockByHash", [block_hash, True])

    def get_latest_blocks(self, count: int = 10) -> List[Dict[str, Any]]:
        """
        Get latest N blocks.

        Args:
            count: Number of blocks to retrieve

        Returns:
            List of block data
        """
        current = self.get_block_number()
        blocks = []
        for i in range(count):
            block_num = current - i
            if block_num < 0:
                break
            block = self.get_block(block_num)
            if block:
                blocks.append(block)
        return blocks

    def wait_for_block(self, target_block: int, timeout: int = 120) -> bool:
        """
        Wait until target block is reached.

        Args:
            target_block: Block number to wait for
            timeout: Maximum wait time in seconds

        Returns:
            True if block reached, False if timeout
        """
        import time
        start = time.time()
        while time.time() - start < timeout:
            current = self.get_block_number()
            if current >= target_block:
                return True
            time.sleep(1)
        return False
