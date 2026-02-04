"""
Balance Mixin for LuxtensorClient

Provides balance query methods for different balance types.
"""

import logging
from typing import TYPE_CHECKING, Any, cast

from .constants import HEX_ZERO
from .types import RewardBalance

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


def _parse_hex_or_int(value: Any) -> int:
    """Parse value as hex string or int."""
    if not value:
        return 0
    if isinstance(value, str) and value.startswith("0x"):
        return int(value, 16)
    return int(value)


class BalanceMixin:
    """
    Mixin providing balance query methods.

    Methods:
        - get_free_balance() - Get free (transferable) balance
        - get_reserved_balance() - Get reserved (locked) balance
        - get_reward_balance() - Get full reward balance info
        - get_total_issuance() - Get total token issuance
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def get_free_balance(self, address: str) -> int:
        """
        Get free (transferable) balance for an address.

        Args:
            address: Account address

        Returns:
            Free balance in base units
        """
        try:
            return int(self._rpc()._call_rpc("balances_free", [address]))
        except Exception as e:
            logger.error(f"Error getting free balance: {e}")
            raise

    def get_reserved_balance(self, address: str) -> int:
        """
        Get reserved (locked) balance for an address.

        Args:
            address: Account address

        Returns:
            Reserved balance in base units
        """
        try:
            return int(self._rpc()._call_rpc("balances_reserved", [address]))
        except Exception as e:
            logger.error(f"Error getting reserved balance: {e}")
            raise

    def get_reward_balance(self, address: str) -> RewardBalance:
        """
        Get full reward balance info for an address.

        Args:
            address: Account address (0x...)

        Returns:
            RewardBalance with available, pendingRewards, staked, and lockedUntil values
        """
        try:
            result = self._rpc()._call_rpc("rewards_getBalance", [address])
            if not result:
                return {
                    "available": 0,
                    "pendingRewards": 0,
                    "staked": 0,
                    "lockedUntil": 0,
                }

            # Parse hex/int values using helper
            balance: RewardBalance = {
                "available": _parse_hex_or_int(result.get("available", HEX_ZERO)),
                "pendingRewards": _parse_hex_or_int(result.get("pendingRewards", HEX_ZERO)),
                "staked": _parse_hex_or_int(result.get("staked", HEX_ZERO)),
                "lockedUntil": result.get("lockedUntil", 0),
            }
            return balance

        except Exception as e:
            logger.warning(f"Failed to get reward balance for {address}: {e}")
            return {"available": 0, "pendingRewards": 0, "staked": 0, "lockedUntil": 0}

    def get_total_issuance(self) -> int:
        """
        Get total token issuance.

        Returns:
            Total tokens issued
        """
        try:
            result = self._rpc()._call_rpc("query_totalIssuance")
            return int(result)
        except Exception as e:
            logger.error(f"Error getting total issuance: {e}")
            raise
