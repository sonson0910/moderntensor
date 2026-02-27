"""
Reward Mixin for LuxtensorClient

Provides reward query and history methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, cast

from .constants import HEX_ZERO

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class RewardMixin:
    """
    Mixin providing reward query methods.

    Methods:
        - get_pending_rewards() - Get pending (unclaimed) rewards
        - get_reward_stats() - Get global reward statistics
        - get_reward_history() - Get reward history for address
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def get_pending_rewards(self, address: str) -> int:
        """
        Get pending (unclaimed) rewards for an address.

        Args:
            address: Account address (0x...)

        Returns:
            Pending rewards amount in base units
        """
        try:
            result = self._rpc()._call_rpc("rewards_getPending", [address])
            if isinstance(result, dict):
                pending = result.get("pending", HEX_ZERO)
                return int(pending, 16) if pending.startswith("0x") else int(pending)
            return 0
        except Exception as e:
            logger.warning("Failed to get pending rewards for %s: %s", address, e)
            return 0

    def get_reward_stats(self) -> Dict[str, Any]:
        """
        Get global reward executor statistics.

        Returns:
            Stats including current epoch, total pending, DAO balance, etc.
        """
        try:
            result = self._rpc()._call_rpc("rewards_getStats", [])
            return result if result else {}
        except Exception as e:
            logger.warning("Failed to get reward stats: %s", e)
            return {}

    def get_reward_history(self, address: str, limit: int = 10) -> List[Dict[str, Any]]:
        """
        Get reward history for an address.

        Args:
            address: Account address (0x...)
            limit: Maximum number of entries to return

        Returns:
            List of reward history entries
        """
        try:
            result = self._rpc()._call_rpc("rewards_getHistory", [address, limit])
            return result.get("history", []) if result else []
        except Exception as e:
            logger.warning("Failed to get reward history for %s: %s", address, e)
            return []
