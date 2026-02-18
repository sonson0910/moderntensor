"""
Stake Client
Handles staking, delegation, and rewards operations.
"""

import logging
from typing import Optional, Dict, Any, List
from .base import BaseRpcClient

logger = logging.getLogger(__name__)


class StakeClient(BaseRpcClient):
    """
    Client for staking operations.
    Single Responsibility: Staking/delegation/rewards only.
    """

    # ========================================================================
    # Staking Queries
    # ========================================================================

    def get_stake(self, address: str) -> int:
        """
        Get staked amount for address.

        Args:
            address: Account address

        Returns:
            Staked amount in base units
        """
        try:
            result = self._call_rpc("staking_getStake", [address])
            if isinstance(result, dict):
                return self._hex_to_int(result.get("stake", "0x0"))
            return self._hex_to_int(result)
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
            return self._hex_to_int(result)
        except Exception as e:
            logger.warning(f"Failed to get total stake: {e}")
            return 0

    def get_stake_for_coldkey_and_hotkey(self, coldkey: str, hotkey: str) -> int:
        """
        Get stake for a coldkey-hotkey pair.

        Args:
            coldkey: Coldkey address
            hotkey: Hotkey address

        Returns:
            Staked amount
        """
        try:
            result = self._call_rpc("staking_getStakeForPair", [coldkey, hotkey])
            return self._hex_to_int(result)
        except Exception as e:
            logger.warning(f"Failed to get stake for pair: {e}")
            return 0

    def get_all_stake_for_coldkey(self, coldkey: str) -> Dict[str, int]:
        """
        Get all stakes for a coldkey.

        Args:
            coldkey: Coldkey address

        Returns:
            Dict mapping hotkey -> stake amount
        """
        try:
            result = self._call_rpc("staking_getAllStakesForColdkey", [coldkey])
            if result:
                return {k: self._hex_to_int(v) for k, v in result.items()}
            return {}
        except Exception as e:
            logger.warning(f"Failed to get stakes for coldkey: {e}")
            return {}

    # ========================================================================
    # Staking Actions
    # ========================================================================

    def stake(self, address: str, amount: int, timestamp: int, signature: str) -> Dict[str, Any]:
        """
        Stake tokens as a validator.

        Args:
            address: Validator address
            amount: Amount to stake in base units
            timestamp: Unix timestamp (must be within 5 min of server time)
            signature: Hex-encoded signature of "stake:{hex_addr}:{amount}"

        Returns:
            Result with success status
        """
        try:
            result = self._call_rpc("staking_stake", [address, str(amount), str(timestamp), signature])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to stake for {address}: {e}")
            return {"success": False, "error": str(e)}

    def unstake(self, address: str, amount: int, timestamp: int, signature: str) -> Dict[str, Any]:
        """
        Unstake tokens.

        Args:
            address: Validator address
            amount: Amount to unstake
            timestamp: Unix timestamp (must be within 5 min of server time)
            signature: Hex-encoded signature of "unstake:{hex_addr}:{amount}"

        Returns:
            Result with success status
        """
        try:
            result = self._call_rpc("staking_unstake", [address, str(amount), str(timestamp), signature])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to unstake for {address}: {e}")
            return {"success": False, "error": str(e)}

    # ========================================================================
    # Delegation
    # ========================================================================

    def delegate(
        self, delegator: str, validator: str, amount: int,
        timestamp: int, signature: str, lock_days: int = 0
    ) -> Dict[str, Any]:
        """
        Delegate tokens to a validator.

        Args:
            delegator: Delegator address
            validator: Validator address
            amount: Amount to delegate
            timestamp: Unix timestamp (must be within 5 min of server time)
            signature: Hex-encoded signature of
                       "delegate:{hex_delegator}:{hex_validator}:{amount}"
            lock_days: Lock period for bonus (0, 30, 90, 180, 365)

        Returns:
            Result with success status
        """
        try:
            params = [delegator, validator, str(amount), str(timestamp), signature]
            if lock_days > 0:
                params.append(str(lock_days))
            result = self._call_rpc("staking_delegate", params)
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to delegate: {e}")
            return {"success": False, "error": str(e)}

    def undelegate(self, delegator: str, timestamp: int, signature: str) -> Dict[str, Any]:
        """
        Remove delegation.

        Args:
            delegator: Delegator address
            timestamp: Unix timestamp (must be within 5 min of server time)
            signature: Hex-encoded signature of "undelegate:{hex_delegator}"

        Returns:
            Result with success status
        """
        try:
            result = self._call_rpc("staking_undelegate", [delegator, str(timestamp), signature])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to undelegate: {e}")
            return {"success": False, "error": str(e)}

    def get_delegation(self, delegator: str) -> Optional[Dict[str, Any]]:
        """
        Get delegation info.

        Args:
            delegator: Delegator address

        Returns:
            Delegation info or None
        """
        try:
            return self._call_rpc("staking_getDelegation", [delegator])
        except Exception as e:
            logger.warning(f"Failed to get delegation: {e}")
            return None

    def get_delegates(self) -> List[Dict[str, Any]]:
        """
        Get all delegates (validators accepting delegation).

        Returns:
            List of delegate info
        """
        try:
            result = self._call_rpc("staking_getDelegates", [])
            return result if result else []
        except Exception as e:
            logger.warning(f"Failed to get delegates: {e}")
            return []

    # ========================================================================
    # Rewards
    # ========================================================================

    def get_pending_rewards(self, address: str) -> int:
        """
        Get pending rewards.

        Args:
            address: Account address

        Returns:
            Pending rewards amount
        """
        try:
            result = self._call_rpc("rewards_getPending", [address])
            if isinstance(result, dict):
                return self._hex_to_int(result.get("pending", "0x0"))
            return self._hex_to_int(result)
        except Exception as e:
            logger.warning(f"Failed to get pending rewards: {e}")
            return 0

    def claim_rewards(self, address: str) -> Dict[str, Any]:
        """
        Claim pending rewards.

        Args:
            address: Account address

        Returns:
            Claim result
        """
        try:
            result = self._call_rpc("rewards_claim", [address])
            return result if result else {"success": False, "claimed": 0}
        except Exception as e:
            logger.error(f"Failed to claim rewards: {e}")
            return {"success": False, "error": str(e)}

    def get_staking_minimums(self) -> Dict[str, int]:
        """
        Get minimum staking requirements.

        Returns:
            Dict with minValidatorStake and minDelegation
        """
        try:
            result = self._call_rpc("staking_getMinimums", [])
            if result:
                return {
                    "minValidatorStake": self._hex_to_int(result.get("minValidatorStake", "0x0")),
                    "minDelegation": self._hex_to_int(result.get("minDelegation", "0x0")),
                }
            return {"minValidatorStake": 0, "minDelegation": 0}
        except Exception as e:
            logger.warning(f"Failed to get staking minimums: {e}")
            return {"minValidatorStake": 0, "minDelegation": 0}
