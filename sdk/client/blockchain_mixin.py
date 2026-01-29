"""
Blockchain Mixin for LuxtensorClient

Provides block query methods.
"""

from typing import Dict, Any, Optional
import logging

logger = logging.getLogger(__name__)


class BlockchainMixin:
    """
    Mixin providing blockchain query methods.

    Methods:
        - get_block_number()
        - get_block()
        - get_block_hash()
        - get_chain_info()
    """

    def get_block_number(self) -> int:
        """
        Get current block height.

        Returns:
            Current block number
        """
        result = self._call_rpc("eth_blockNumber")
        return int(result, 16) if isinstance(result, str) else result

    def get_block(self, block_number: Optional[int] = None) -> Dict[str, Any]:
        """
        Get block by number.

        Args:
            block_number: Block number (None for latest)

        Returns:
            Block data
        """
        if block_number is not None:
            block_param = f"0x{block_number:x}"
        else:
            block_param = "latest"

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

    def get_chain_info(self):
        """
        Get chain information.

        Returns:
            ChainInfo object
        """
        from .base import ChainInfo

        try:
            chain_id = self._call_rpc("eth_chainId")
            block = self.get_block_number()
            version = self._call_rpc("web3_clientVersion")

            return ChainInfo(
                chain_id=chain_id,
                network=self.network,
                block_height=block,
                version=version or "unknown"
            )
        except Exception as e:
            logger.warning(f"Failed to get chain info: {e}")
            return ChainInfo(
                chain_id="0x0",
                network=self.network,
                block_height=0,
                version="unknown"
            )
