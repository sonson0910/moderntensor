"""
Staking Mixin for LuxtensorClient

Provides staking query methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

from .constants import HEX_ZERO

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class StakingMixin:
    """
    Mixin providing staking query and transaction methods.

    Query Methods:
        - get_stake() - Get stake for address
        - get_total_stake() - Get total network stake
        - get_stake_for_coldkey_and_hotkey() - Get stake for pair
        - get_all_stakes_for_coldkey() - Get all stakes for coldkey
        - get_delegates() - Get all delegates
        - batch_get_stakes() - Batch get stakes for pairs
        - get_all_stake_for_coldkey() - Get stake map for coldkey
        - get_all_stake_for_hotkey() - Get stake map for hotkey
        - get_total_stake_for_coldkey() - Total stake for coldkey
        - get_total_stake_for_hotkey() - Total stake for hotkey
        - get_delegate_info() - Get delegate information
        - get_delegate_take() - Get delegate commission rate
        - get_stake_history() - Get staking history
        - is_delegate() - Check if address is delegate

    Transaction Methods:
        - stake() - Stake tokens
        - unstake() - Unstake tokens
        - delegate() - Delegate to validator
        - undelegate() - Remove delegation
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def get_stake(self, address: str) -> int:
        """
        Get staked amount for address.

        Args:
            address: Account address (0x...)

        Returns:
            Staked amount in base units
        """
        try:
            result = self._rpc()._call_rpc("staking_getStake", [address])
            if isinstance(result, str):
                return int(result, 16) if result.startswith("0x") else int(result)
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
            result = self._rpc()._call_rpc("staking_getTotalStake", [])
            if isinstance(result, str):
                return int(result, 16) if result.startswith("0x") else int(result)
            return result if result else 0
        except Exception as e:
            logger.warning(f"Failed to get total stake: {e}")
            return 0

    def get_stake_for_coldkey_and_hotkey(self, coldkey: str, hotkey: str) -> int:
        """
        Get stake for coldkey-hotkey pair.

        Args:
            coldkey: Coldkey address
            hotkey: Hotkey address

        Returns:
            Stake amount
        """
        try:
            result = self._rpc()._call_rpc("staking_getStakeForPair", [coldkey, hotkey])
            if isinstance(result, str):
                return int(result, 16) if result.startswith("0x") else int(result)
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
            result = self._rpc()._call_rpc("staking_getAllStakesForColdkey", [coldkey])
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
            result = self._rpc()._call_rpc("staking_getDelegates", [])
            return result if result else []
        except Exception as e:
            logger.warning(f"Failed to get delegates: {e}")
            return []

    # Transaction Methods

    def stake(self, address: str, amount: int) -> Dict[str, Any]:
        """
        Stake tokens as a validator.

        Args:
            address: Validator address (0x...)
            amount: Amount to stake in base units

        Returns:
            Result with success status and new stake amount
        """
        try:
            result = self._rpc()._call_rpc("staking_stake", [address, str(amount)])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to stake for {address}: {e}")
            return {"success": False, "error": str(e)}

    def unstake(self, address: str, amount: int) -> Dict[str, Any]:
        """
        Unstake tokens from validator position.

        Args:
            address: Validator address (0x...)
            amount: Amount to unstake in base units

        Returns:
            Result with success status and remaining stake
        """
        try:
            result = self._rpc()._call_rpc("staking_unstake", [address, str(amount)])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to unstake for {address}: {e}")
            return {"success": False, "error": str(e)}

    def delegate(
        self, delegator: str, validator: str, amount: int, lock_days: int = 0
    ) -> Dict[str, Any]:
        """
        Delegate tokens to a validator.

        Args:
            delegator: Delegator address (0x...)
            validator: Validator address to delegate to (0x...)
            amount: Amount to delegate in base units
            lock_days: Optional lock period for bonus rewards (0, 30, 90, 180, 365)

        Returns:
            Result with success status and delegation info
        """
        try:
            result = self._rpc()._call_rpc(
                "staking_delegate", [delegator, validator, str(amount), str(lock_days)]
            )
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to delegate from {delegator} to {validator}: {e}")
            return {"success": False, "error": str(e)}

    def undelegate(self, delegator: str) -> Dict[str, Any]:
        """
        Remove delegation and return tokens.

        Args:
            delegator: Delegator address (0x...)

        Returns:
            Result with success status and returned amount
        """
        try:
            result = self._rpc()._call_rpc("staking_undelegate", [delegator])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to undelegate for {delegator}: {e}")
            return {"success": False, "error": str(e)}

    # Advanced Query Methods

    def batch_get_stakes(self, coldkey_hotkey_pairs: List[tuple]) -> List[int]:
        """
        Get stakes for multiple coldkey-hotkey pairs.

        Args:
            coldkey_hotkey_pairs: List of (coldkey, hotkey) tuples

        Returns:
            List of stake amounts
        """
        results = []
        for coldkey, hotkey in coldkey_hotkey_pairs:
            try:
                stake = self.get_stake_for_coldkey_and_hotkey(coldkey, hotkey)
                results.append(stake)
            except Exception as e:
                logger.warning(f"Error getting stake for {coldkey}-{hotkey}: {e}")
                results.append(0)
        return results

    def get_all_stake_for_coldkey(self, coldkey: str) -> Dict[str, int]:
        """
        Get all stakes for a coldkey across all hotkeys.

        Args:
            coldkey: Coldkey address

        Returns:
            Dictionary mapping hotkey to stake amount
        """
        try:
            result = self._rpc()._call_rpc("query_allStakeForColdkey", [coldkey])
            return result if result else {}
        except Exception as e:
            logger.error(f"Error getting all stakes for coldkey {coldkey}: {e}")
            return {}

    def get_all_stake_for_hotkey(self, hotkey: str) -> Dict[str, int]:
        """
        Get all stakes for a hotkey from all coldkeys.

        Args:
            hotkey: Hotkey address

        Returns:
            Dictionary mapping coldkey to stake amount
        """
        try:
            result = self._rpc()._call_rpc("query_allStakeForHotkey", [hotkey])
            return result if result else {}
        except Exception as e:
            logger.error(f"Error getting all stakes for hotkey {hotkey}: {e}")
            return {}

    def get_total_stake_for_coldkey(self, coldkey: str) -> int:
        """
        Get total stake for a coldkey.

        Args:
            coldkey: Coldkey address

        Returns:
            Total stake amount
        """
        try:
            result = self._rpc()._call_rpc("query_totalStakeForColdkey", [coldkey])
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting total stake for coldkey {coldkey}: {e}")
            return 0

    def get_total_stake_for_hotkey(self, hotkey: str) -> int:
        """
        Get total stake for a hotkey.

        Args:
            hotkey: Hotkey address

        Returns:
            Total stake amount
        """
        try:
            result = self._rpc()._call_rpc("query_totalStakeForHotkey", [hotkey])
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting total stake for hotkey {hotkey}: {e}")
            return 0

    # Delegate Methods

    def get_delegate_info(self, hotkey: str) -> Dict[str, Any]:
        """
        Get information about a specific delegate.

        Args:
            hotkey: Delegate hotkey address

        Returns:
            Delegate information
        """
        try:
            result = self._rpc()._call_rpc("query_delegateInfo", [hotkey])
            return result if result else {}
        except Exception as e:
            logger.error(f"Error getting delegate info for {hotkey}: {e}")
            return {}

    def get_delegate_take(self, hotkey: str) -> float:
        """
        Get delegate commission rate (take).

        Args:
            hotkey: Delegate hotkey address

        Returns:
            Commission rate (0-1, e.g., 0.18 = 18%)
        """
        try:
            result = self._rpc()._call_rpc("query_delegateTake", [hotkey])
            return float(result) if result else 0.0
        except Exception as e:
            logger.error(f"Error getting delegate take for {hotkey}: {e}")
            return 0.0

    def is_delegate(self, hotkey: str) -> bool:
        """
        Check if a hotkey is a delegate.

        Args:
            hotkey: Hotkey address

        Returns:
            True if is delegate, False otherwise
        """
        try:
            result = self._rpc()._call_rpc("query_isDelegate", [hotkey])
            return bool(result)
        except Exception as e:
            logger.error(f"Error checking if {hotkey} is delegate: {e}")
            return False

    def get_stake_history(self, address: str, limit: int = 10) -> List[Dict[str, Any]]:
        """
        Get staking history for an address.

        Args:
            address: Address to query
            limit: Maximum number of records to return

        Returns:
            List of stake events
        """
        try:
            result = self._rpc()._call_rpc("query_stakeHistory", [address, limit])
            return result if result else []
        except Exception as e:
            logger.error(f"Error getting stake history for {address}: {e}")
            return []

    def get_delegation(self, delegator: str) -> Optional[Dict[str, Any]]:
        """Get delegation info for a delegator."""
        try:
            result = self._rpc()._call_rpc("staking_getDelegation", [delegator])
            return result
        except Exception as e:
            logger.warning(f"Failed to get delegation for {delegator}: {e}")
            return None

    def get_nominators(self, hotkey: str) -> List[str]:
        """Get list of nominators for a delegate."""
        try:
            result = self._rpc()._call_rpc("query_nominators", [hotkey])
            return result
        except Exception as e:
            logger.error(f"Error getting nominators for {hotkey}: {e}")
            raise

    def get_staking_minimums(self) -> Dict[str, int]:
        """Get minimum staking requirements."""
        try:
            result = self._rpc()._call_rpc("staking_getMinimums", [])
            minimums = {}
            if result:
                min_stake = result.get("minValidatorStake", HEX_ZERO)
                min_del = result.get("minDelegation", HEX_ZERO)
                minimums["minValidatorStake"] = (
                    int(min_stake, 16) if min_stake.startswith("0x") else int(min_stake)
                )
                minimums["minDelegation"] = (
                    int(min_del, 16) if min_del.startswith("0x") else int(min_del)
                )
            return minimums
        except Exception as e:
            logger.warning(f"Failed to get staking minimums: {e}")
            return {"minValidatorStake": 0, "minDelegation": 0}
