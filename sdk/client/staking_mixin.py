"""
Staking Mixin for LuxtensorClient

Provides staking query methods.
"""

import logging
import time
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

from .constants import HEX_ZERO

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)

# Default TTL for delegate cache (seconds)
_DELEGATE_CACHE_TTL = 30


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

    # ------------------------------------------------------------------
    # Internal: Delegate Cache (fixes N+1 queries — C2)
    # ------------------------------------------------------------------

    _delegates_cache: Optional[List[Dict[str, Any]]] = None
    _delegates_cache_time: float = 0.0

    def _get_cached_delegates(self, ttl_seconds: int = _DELEGATE_CACHE_TTL) -> List[Dict[str, Any]]:
        """
        Get delegates with TTL-based caching.

        Avoids the N+1 query problem where get_delegate_info(),
        get_delegate_take(), is_delegate(), and get_nominators() each
        make a separate RPC call to fetch the full delegate list.

        Args:
            ttl_seconds: Cache time-to-live in seconds (default: 30)

        Returns:
            List of delegate info dicts
        """
        now = time.monotonic()
        if self._delegates_cache is not None and (now - self._delegates_cache_time) < ttl_seconds:
            return self._delegates_cache

        try:
            result = self._rpc()._call_rpc("staking_getDelegates", [])
            self._delegates_cache = result if result else []
            self._delegates_cache_time = now
            return self._delegates_cache
        except Exception as e:
            logger.warning(f"Failed to fetch delegates: {e}")
            # Return stale cache if available, otherwise empty
            return self._delegates_cache if self._delegates_cache is not None else []

    def invalidate_delegate_cache(self) -> None:
        """Invalidate the delegate cache (call after stake/unstake mutations)."""
        self._delegates_cache = None
        self._delegates_cache_time = 0.0

    # ------------------------------------------------------------------
    # Query Methods
    # ------------------------------------------------------------------

    def get_stake(self, address: str) -> int:
        """
        Get staked amount for address.

        Args:
            address: Account address (0x...)

        Returns:
            Staked amount in base units

        Raises:
            Exception: If RPC call fails
        """
        result = self._rpc()._call_rpc("staking_getStake", [address])
        return self._rpc()._parse_hex_int(result)

    def get_total_stake(self) -> int:
        """
        Get total staked in network.

        Returns:
            Total stake amount in base units

        Raises:
            Exception: If RPC call fails
        """
        result = self._rpc()._call_rpc("staking_getTotalStake", [])
        return self._rpc()._parse_hex_int(result)

    def get_stake_for_coldkey_and_hotkey(self, coldkey: str, hotkey: str) -> int:
        """
        Get stake for coldkey-hotkey pair.

        Args:
            coldkey: Coldkey address
            hotkey: Hotkey address

        Returns:
            Stake amount

        Raises:
            Exception: If RPC call fails
        """
        result = self._rpc()._call_rpc("staking_getStakeForPair", [coldkey, hotkey])
        return self._rpc()._parse_hex_int(result)

    def get_all_stakes_for_coldkey(self, coldkey: str) -> Dict[str, int]:
        """
        Get all stakes for a coldkey.

        Args:
            coldkey: Coldkey address

        Returns:
            Dict of hotkey -> stake amount

        Raises:
            Exception: If RPC call fails
        """
        result = self._rpc()._call_rpc("staking_getAllStakesForColdkey", [coldkey])
        return result if result else {}

    def get_delegates(self) -> List[Dict[str, Any]]:
        """
        Get all delegates.

        Returns:
            List of delegate info

        Raises:
            Exception: If RPC call fails
        """
        result = self._rpc()._call_rpc("staking_getDelegates", [])
        return result if result else []

    # ------------------------------------------------------------------
    # Transaction Methods
    # ------------------------------------------------------------------

    def stake(self, address: str, amount: int, timestamp: int, signature: str) -> Dict[str, Any]:
        """
        Stake tokens as a validator.

        Args:
            address: Validator address (0x...)
            amount: Amount to stake in base units
            timestamp: Unix timestamp (must be within 5 min of server time)
            signature: Hex-encoded secp256k1 signature of
                       "stake:{hex_address}:{amount}" using personal_sign format

        Returns:
            Result with success status and new stake amount
        """
        try:
            result = self._rpc()._call_rpc("staking_stake", [address, str(amount), str(timestamp), signature])
            self.invalidate_delegate_cache()
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to stake for {address}: {e}")
            return {"success": False, "error": str(e)}

    def unstake(self, address: str, amount: int, timestamp: int, signature: str) -> Dict[str, Any]:
        """
        Unstake tokens from validator position.

        Args:
            address: Validator address (0x...)
            amount: Amount to unstake in base units
            timestamp: Unix timestamp (must be within 5 min of server time)
            signature: Hex-encoded secp256k1 signature of
                       "unstake:{hex_address}:{amount}" using personal_sign format

        Returns:
            Result with success status and remaining stake
        """
        try:
            result = self._rpc()._call_rpc("staking_unstake", [address, str(amount), str(timestamp), signature])
            self.invalidate_delegate_cache()
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to unstake for {address}: {e}")
            return {"success": False, "error": str(e)}

    def delegate(
        self, delegator: str, validator: str, amount: int,
        timestamp: int, signature: str, lock_days: int = 0
    ) -> Dict[str, Any]:
        """
        Delegate tokens to a validator.

        Args:
            delegator: Delegator address (0x...)
            validator: Validator address to delegate to (0x...)
            amount: Amount to delegate in base units
            timestamp: Unix timestamp (must be within 5 min of server time)
            signature: Hex-encoded secp256k1 signature of
                       "delegate:{hex_delegator}:{hex_validator}:{amount}"
                       using personal_sign format
            lock_days: Optional lock period for bonus rewards (0, 30, 90, 180, 365)

        Returns:
            Result with success status and delegation info
        """
        try:
            params = [delegator, validator, str(amount), str(timestamp), signature]
            if lock_days > 0:
                params.append(str(lock_days))
            result = self._rpc()._call_rpc("staking_delegate", params)
            self.invalidate_delegate_cache()
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to delegate from {delegator} to {validator}: {e}")
            return {"success": False, "error": str(e)}

    def undelegate(self, delegator: str, timestamp: int, signature: str) -> Dict[str, Any]:
        """
        Remove delegation and return tokens.

        Args:
            delegator: Delegator address (0x...)
            timestamp: Unix timestamp (must be within 5 min of server time)
            signature: Hex-encoded secp256k1 signature of
                       "undelegate:{hex_delegator}" using personal_sign format

        Returns:
            Result with success status and returned amount
        """
        try:
            result = self._rpc()._call_rpc("staking_undelegate", [delegator, str(timestamp), signature])
            self.invalidate_delegate_cache()
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to undelegate for {delegator}: {e}")
            return {"success": False, "error": str(e)}

    # ------------------------------------------------------------------
    # Batch Query — uses JSON-RPC batch call (C3)
    # ------------------------------------------------------------------

    def batch_get_stakes(self, coldkey_hotkey_pairs: List[tuple]) -> List[int]:
        """
        Get stakes for multiple coldkey-hotkey pairs using batch RPC.

        Sends all requests in a single HTTP call instead of sequential loops.

        Args:
            coldkey_hotkey_pairs: List of (coldkey, hotkey) tuples

        Returns:
            List of stake amounts (0 for any that failed)
        """
        if not coldkey_hotkey_pairs:
            return []

        requests = [
            {"method": "staking_getStakeForPair", "params": [coldkey, hotkey]}
            for coldkey, hotkey in coldkey_hotkey_pairs
        ]

        results = self._rpc()._call_rpc_batch(requests)
        return [self._rpc()._parse_hex_int(r) for r in results]

    # ------------------------------------------------------------------
    # Advanced Query Methods
    # ------------------------------------------------------------------

    def get_all_stake_for_coldkey(self, coldkey: str) -> Dict[str, int]:
        """
        Get all stakes for a coldkey across all hotkeys.

        Args:
            coldkey: Coldkey address

        Returns:
            Dictionary mapping hotkey to stake amount

        Raises:
            Exception: If RPC call fails
        """
        result = self._rpc()._call_rpc("query_allStakeForColdkey", [coldkey])
        return result if result else {}

    def get_all_stake_for_hotkey(self, hotkey: str) -> Dict[str, int]:
        """
        Get all stakes for a hotkey from all coldkeys.

        Args:
            hotkey: Hotkey address

        Returns:
            Dictionary mapping coldkey to stake amount

        Raises:
            Exception: If RPC call fails
        """
        result = self._rpc()._call_rpc("query_allStakeForHotkey", [hotkey])
        return result if result else {}

    def get_total_stake_for_coldkey(self, coldkey: str) -> int:
        """
        Get total stake for a coldkey.

        Args:
            coldkey: Coldkey address

        Returns:
            Total stake amount

        Raises:
            Exception: If RPC call fails
        """
        result = self._rpc()._call_rpc("query_totalStakeForColdkey", [coldkey])
        return int(result) if result else 0

    def get_total_stake_for_hotkey(self, hotkey: str) -> int:
        """
        Get total stake for a hotkey.

        Args:
            hotkey: Hotkey address

        Returns:
            Total stake amount

        Raises:
            Exception: If RPC call fails
        """
        result = self._rpc()._call_rpc("query_totalStakeForHotkey", [hotkey])
        return int(result) if result else 0

    # ------------------------------------------------------------------
    # Delegate Methods — uses cached delegates (C2)
    # ------------------------------------------------------------------

    def get_delegate_info(self, hotkey: str) -> Dict[str, Any]:
        """
        Get information about a specific delegate.

        Uses cached delegate list to avoid N+1 RPC queries.

        Args:
            hotkey: Delegate hotkey address

        Returns:
            Delegate information
        """
        delegates = self._get_cached_delegates()
        for d in delegates:
            if d.get("hotkey") == hotkey or d.get("address") == hotkey:
                return d
        return {}

    def get_delegate_take(self, hotkey: str) -> float:
        """
        Get delegate commission rate (take).

        Uses cached delegate list to avoid N+1 RPC queries.

        Args:
            hotkey: Delegate hotkey address

        Returns:
            Commission rate (0-1, e.g., 0.18 = 18%)
        """
        delegates = self._get_cached_delegates()
        for d in delegates:
            if d.get("hotkey") == hotkey or d.get("address") == hotkey:
                take = d.get("take", d.get("commission", 0.0))
                return float(take) if take else 0.0
        return 0.0

    def is_delegate(self, hotkey: str) -> bool:
        """
        Check if a hotkey is a delegate.

        Uses cached delegate list to avoid N+1 RPC queries.

        Args:
            hotkey: Hotkey address

        Returns:
            True if is delegate, False otherwise
        """
        delegates = self._get_cached_delegates()
        return any(
            d.get("hotkey") == hotkey or d.get("address") == hotkey
            for d in delegates
        )

    def get_stake_history(self, address: str, limit: int = 10) -> List[Dict[str, Any]]:
        """
        Get staking history for an address.

        Args:
            address: Address to query
            limit: Maximum number of records to return

        Returns:
            List of stake events

        Raises:
            NotImplementedError: This endpoint is not yet available on LuxTensor server
        """
        raise NotImplementedError(
            "Stake history is not yet available on the LuxTensor RPC server. "
            "Track this feature at https://github.com/moderntensor/luxtensor/issues"
        )

    def get_delegation(self, delegator: str) -> Optional[Dict[str, Any]]:
        """Get delegation info for a delegator."""
        return self._rpc()._safe_call_rpc("staking_getDelegation", [delegator])

    def get_nominators(self, hotkey: str) -> List[str]:
        """
        Get list of nominators for a delegate.

        Uses cached delegate list to avoid N+1 RPC queries.
        """
        delegates = self._get_cached_delegates()
        for d in delegates:
            if d.get("hotkey") == hotkey or d.get("address") == hotkey:
                nominators = d.get("nominators", [])
                return nominators if isinstance(nominators, list) else []
        return []

    def get_staking_minimums(self) -> Dict[str, int]:
        """Get minimum staking requirements."""
        result = self._rpc()._safe_call_rpc("staking_getMinimums", [])
        if not result:
            return {"minValidatorStake": 0, "minDelegation": 0}

        parse = self._rpc()._parse_hex_int
        return {
            "minValidatorStake": parse(result.get("minValidatorStake", HEX_ZERO)),
            "minDelegation": parse(result.get("minDelegation", HEX_ZERO)),
        }
