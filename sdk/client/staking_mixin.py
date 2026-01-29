"""
Staking Mixin for LuxtensorClient

Provides staking query methods.
"""

from typing import Dict, Any, List
import logging

logger = logging.getLogger(__name__)


class StakingMixin:
    """
    Mixin providing staking query methods.

    Methods:
        - get_stake()
        - get_total_stake()
        - get_stake_for_coldkey_and_hotkey()
        - get_all_stakes_for_coldkey()
        - get_delegates()
    """

    def get_stake(self, address: str) -> int:
        """
        Get staked amount for address.

        Args:
            address: Account address (0x...)

        Returns:
            Staked amount in base units
        """
        try:
            result = self._call_rpc("staking_getStake", [address])
            if isinstance(result, str):
                return int(result, 16) if result.startswith('0x') else int(result)
            return result if result else 0
        except Exception as e:
            logger.warning(f"Failed to get stake for {address}: {e}")
            return 0

    def get_total_stake(self) -> int:
        """
        Get total staked in network.

        Returns:
            Total stake amount in base units
        """
        try:
            result = self._call_rpc("staking_getTotalStake", [])
            if isinstance(result, str):
                return int(result, 16) if result.startswith('0x') else int(result)
            return result if result else 0
        except Exception as e:
            logger.warning(f"Failed to get total stake: {e}")
            return 0

    def get_stake_for_coldkey_and_hotkey(
        self,
        coldkey: str,
        hotkey: str
    ) -> int:
        """
        Get stake for coldkey-hotkey pair.

        Args:
            coldkey: Coldkey address
            hotkey: Hotkey address

        Returns:
            Stake amount
        """
        try:
            result = self._call_rpc("staking_getStakeForPair", [coldkey, hotkey])
            if isinstance(result, str):
                return int(result, 16) if result.startswith('0x') else int(result)
            return result if result else 0
        except Exception as e:
            logger.warning(f"Failed to get stake for pair: {e}")
            return 0

    def get_all_stakes_for_coldkey(self, coldkey: str) -> Dict[str, int]:
        """
        Get all stakes for a coldkey.

        Args:
            coldkey: Coldkey address

        Returns:
            Dict of hotkey -> stake amount
        """
        try:
            result = self._call_rpc("staking_getAllStakesForColdkey", [coldkey])
            return result if result else {}
        except Exception as e:
            logger.warning(f"Failed to get stakes for coldkey: {e}")
            return {}

    def get_delegates(self) -> List[Dict[str, Any]]:
        """
        Get all delegates.

        Returns:
            List of delegate info
        """
        try:
            result = self._call_rpc("staking_getDelegates", [])
            return result if result else []
        except Exception as e:
            logger.warning(f"Failed to get delegates: {e}")
            return []
