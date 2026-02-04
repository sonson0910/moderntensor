"""
Utilities Mixin for LuxtensorClient

Provides utility methods for health checks, address validation,
block waiting, and transaction queries.
"""

import logging
import time
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

from .constants import DEFAULT_QUERY_LIMIT

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class UtilsMixin:
    """Mixin providing utility methods."""

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def health_check(self) -> Dict[str, Any]:
        """Perform comprehensive node health check."""
        try:
            block_number = self._rpc()._call_rpc("eth_blockNumber", [])
            block_height = int(block_number, 16) if isinstance(block_number, str) else block_number

            peer_count_hex = self._rpc()._call_rpc("net_peerCount", [])
            peer_count = int(peer_count_hex, 16) if isinstance(peer_count_hex, str) else 0

            syncing = self._rpc()._call_rpc("eth_syncing", [])
            is_syncing = syncing is not False

            version = self._rpc()._call_rpc("web3_clientVersion", []) or "unknown"

            is_healthy = block_height > 0 and not is_syncing

            return {
                "is_healthy": is_healthy,
                "block_number": block_height,
                "peer_count": peer_count,
                "syncing": is_syncing,
                "version": version,
                "timestamp": int(time.time()),
            }
        except Exception as e:
            logger.error(f"Health check failed: {e}")
            return {
                "is_healthy": False,
                "error": str(e),
                "timestamp": int(time.time()),
            }

    def validate_address(self, address: str) -> bool:
        """Validate Ethereum address format."""
        if not address or not address.startswith("0x") or len(address) != 42:
            return False
        try:
            int(address, 16)
            return True
        except ValueError:
            return False

    def wait_for_block(
        self,
        target_block: int,
        timeout: int = 60,
        poll_interval: float = 1.0,
    ) -> bool:
        """Wait until blockchain reaches target block height."""
        start_time = time.time()

        while time.time() - start_time < timeout:
            try:
                current_block = self._rpc()._call_rpc("eth_blockNumber", [])
                current_height = (
                    int(current_block, 16) if isinstance(current_block, str) else current_block
                )

                if current_height >= target_block:
                    logger.info(f"Reached target block {target_block}")
                    return True

                time.sleep(poll_interval)
            except Exception as e:
                logger.warning(f"Error checking block height: {e}")
                time.sleep(poll_interval)

        logger.warning(f"Timeout waiting for block {target_block}")
        return False

    # NOTE: get_transaction() and get_transaction_receipt() moved to TransactionMixin
    # to avoid method conflicts in multiple inheritance

    def estimate_gas(self, transaction: Dict[str, Any]) -> int:
        """Estimate gas required for transaction."""
        try:
            result = self._rpc()._call_rpc("eth_estimateGas", [transaction])
            return int(result, 16) if isinstance(result, str) else result
        except Exception as e:
            logger.error(f"Gas estimation failed: {e}")
            return 21000

    def to_wei(self, amount: float) -> int:
        """Convert from MDT to wei (1 MDT = 10^18 wei)."""
        return int(amount * 10**18)

    def from_wei(self, amount: int) -> float:
        return amount / 10**18

    # Network Information

    def get_network_info(self) -> Dict[str, Any]:
        """Get comprehensive network information."""
        try:
            result = self._rpc()._call_rpc("query_networkInfo")
            return result
        except Exception as e:
            logger.error(f"Error getting network info: {e}")
            raise

    def get_network_version(self) -> str:
        """Get network protocol version."""
        try:
            return self._rpc()._call_rpc("system_version")
        except Exception as e:
            logger.error(f"Error getting network version: {e}")
            raise

    def get_peer_count(self) -> int:
        """Get number of connected peers."""
        try:
            return int(self._rpc()._call_rpc("system_peerCount"))
        except Exception as e:
            logger.error(f"Error getting peer count: {e}")
            raise

    def is_connected(self) -> bool:
        """Check if connected to Luxtensor."""
        try:
            self.get_block_number()  # type: ignore[attr-defined]
            return True
        except Exception:  # ✅ Explicit exception type (was bare except:)
            return False

    def is_syncing(self) -> bool:
        """Check if node is syncing."""
        try:
            result = self._rpc()._call_rpc("eth_syncing", [])
            return result is not False
        except Exception as e:
            logger.error(f"Error checking sync status: {e}")
            return False

    def switch_network(self, url: str, network: str = "testnet") -> None:
        """Switch to a different network."""
        self.url = url
        self.network = network
        logger.info(f"Switched to network: {network} at {url}")

    def get_network_state_summary(self) -> Dict[str, Any]:
        """Get a comprehensive summary of network state."""
        try:
            return {
                "block_number": self.get_block_number(),  # type: ignore[attr-defined]
                "total_subnets": self.get_total_subnets(),  # type: ignore[attr-defined]
                "total_neurons": self.get_total_neurons(),  # type: ignore[attr-defined]
                "total_stake": self.get_total_stake(),  # type: ignore[attr-defined]
                "total_issuance": self.get_total_issuance(),  # type: ignore[attr-defined]
                "network_info": self.get_network_info(),  # type: ignore[attr-defined]
            }
        except Exception as e:
            logger.error(f"Error getting network state summary: {e}")
            raise

    # UID/Hotkey Lookups

    def get_uid_for_hotkey(self, subnet_id: int, hotkey: str) -> Optional[int]:
        """Get neuron UID for a hotkey in a subnet."""
        try:
            result = self._rpc()._call_rpc("query_uidForHotkey", [subnet_id, hotkey])
            return int(result) if result is not None else None
        except Exception as e:
            logger.error(f"Error getting UID for hotkey {hotkey}: {e}")
            raise

    def get_hotkey_for_uid(self, subnet_id: int, neuron_uid: int) -> Optional[str]:
        """Get hotkey for a neuron UID in a subnet."""
        try:
            result = self._rpc()._call_rpc("query_hotkeyForUid", [subnet_id, neuron_uid])
            return result
        except Exception as e:
            logger.error(f"Error getting hotkey for UID {neuron_uid}: {e}")
            raise

    # Root Network

    def get_root_validators(self) -> List[str]:
        """Get list of root network validators."""
        try:
            result = self._rpc()._call_rpc("query_rootValidators")
            return result if result else []
        except Exception as e:
            logger.error(f"Error getting root validators: {e}")
            raise

    def get_root_validator_count(self) -> int:
        """Get count of root network validators."""
        try:
            result = self._rpc()._call_rpc("query_rootValidatorCount")
            return int(result)
        except Exception as e:
            logger.error(f"Error getting root validator count: {e}")
            raise

    def is_root_validator(self, hotkey: str) -> bool:
        """Check if hotkey is a root validator."""
        try:
            result = self._rpc()._call_rpc("query_isRootValidator", [hotkey])
            return bool(result)
        except Exception as e:
            logger.error(f"Error checking root validator status: {e}")
            raise

    def get_root_config(self) -> Dict[str, Any]:
        """Get root network configuration."""
        try:
            result = self._rpc()._call_rpc("query_rootConfig")
            return result
        except Exception as e:
            logger.error(f"Error getting root config: {e}")
            raise

    def get_root_network_validators(self) -> List[Dict[str, Any]]:
        """Get detailed root network validator information."""
        try:
            result = self._rpc()._call_rpc("query_rootNetworkValidators")
            return result if result else []
        except Exception as e:
            logger.error(f"Error getting root network validators: {e}")
            raise

    # Validators

    def get_validators(self, subnet_id: int) -> List[int]:
        """Get list of validator UIDs for a subnet."""
        try:
            result = self._rpc()._call_rpc("query_validators", [subnet_id])
            return result if result else []
        except Exception as e:
            logger.error(f"Error getting validators for subnet {subnet_id}: {e}")
            raise

    def has_validator_permit(self, subnet_id: int, neuron_uid: int) -> bool:
        """Check if neuron has validator permit."""
        try:
            result = self._rpc()._call_rpc("query_validatorPermit", [subnet_id, neuron_uid])
            return bool(result)
        except Exception as e:
            logger.error(f"Error checking validator permit: {e}")
            raise

    def get_validator_status(self, hotkey: str) -> Dict[str, Any]:
        """Get comprehensive validator status."""
        try:
            result = self._rpc()._call_rpc("query_validatorStatus", [hotkey])
            return result
        except Exception as e:
            logger.error(f"Error getting validator status: {e}")
            raise

    # Transfers

    def get_transfer_history(
        self, address: str, limit: int = DEFAULT_QUERY_LIMIT
    ) -> List[Dict[str, Any]]:
        """Get transfer history for an address."""
        try:
            result = self._rpc()._call_rpc("query_transferHistory", [address, limit])
            return result if result else []
        except Exception as e:
            logger.error(f"Error getting transfer history: {e}")
            raise

    def get_transactions_for_address(
        self, address: str, limit: int = DEFAULT_QUERY_LIMIT
    ) -> List[Dict[str, Any]]:
        """Get transactions for an address."""
        try:
            result = self._rpc()._call_rpc("query_transactionsForAddress", [address, limit])
            return result if result else []
        except Exception as e:
            logger.error(f"Error getting transactions for address: {e}")
            raise
