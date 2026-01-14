"""
Blockchain Mixin for LuxtensorClient

Provides block and chain information methods.
"""

from typing import Optional, Dict, Any, TYPE_CHECKING

if TYPE_CHECKING:
    from .base import BaseClient


class BlockchainMixin:
    """
    Mixin providing blockchain query methods.

    Methods:
        - get_block_number()
        - get_block()
        - get_block_hash()
        - get_chain_info()
        - get_gas_price()
    """

    # Type hint for mixin (self will be BaseClient)
    _call_rpc: callable
    url: str
    network: str

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
            Block data dictionary
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
            Block hash string
        """
        block = self.get_block(block_number)
        return block.get("hash", "") if block else ""

    def get_gas_price(self) -> int:
        """
        Get current gas price.

        Returns:
            Gas price in LTS
        """
        result = self._call_rpc("eth_gasPrice")
        return int(result, 16) if isinstance(result, str) else result

    def get_chain_id(self) -> int:
        """
        Get chain ID.

        Returns:
            Chain ID as integer
        """
        result = self._call_rpc("eth_chainId")
        return int(result, 16) if isinstance(result, str) else result

    def get_network_version(self) -> str:
        """
        Get network version.

        Returns:
            Network version string
        """
        return self._call_rpc("net_version") or ""

    def get_client_version(self) -> str:
        """
        Get client version.

        Returns:
            Client version string (e.g., "LuxTensor/v1.0.0")
        """
        return self._call_rpc("web3_clientVersion") or ""

    def get_peer_count(self) -> int:
        """
        Get connected peer count.

        Returns:
            Number of connected peers
        """
        result = self._call_rpc("net_peerCount")
        return int(result, 16) if isinstance(result, str) else 0
