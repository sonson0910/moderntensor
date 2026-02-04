"""
Blockchain Mixin for LuxtensorClient

Provides blockchain query methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, Optional, cast

from .constants import HEX_ZERO

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class BlockchainMixin:
    """
    Mixin providing blockchain query methods.

    Requires:
        RPCProvider protocol (provided by BaseClient)

    Methods:
        - get_block_number() - Get current block number
        - get_block_by_number() - Get block details
        - get_block_hash() - Get block hash
        - get_transaction_count() - Get transaction count
        - get_chain_info() - Get chain information
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def get_block_number(self) -> int:
        """
        Get current block height.

        Returns:
            Current block number
        """
        result = self._rpc()._call_rpc("eth_blockNumber")
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

        return self._rpc()._call_rpc("eth_getBlockByNumber", [block_param, True])

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
            chain_id = self._rpc()._call_rpc("eth_chainId")
            block = self.get_block_number()
            version = self._rpc()._call_rpc("web3_clientVersion")

            return ChainInfo(
                chain_id=chain_id,
                network=self.network,
                block_height=block,
                version=version or "unknown",
            )
        except Exception as e:
            logger.warning(f"Failed to get chain info: {e}")
            return ChainInfo(
                chain_id=HEX_ZERO, network=self.network, block_height=0, version="unknown"
            )
