"""
Weights Mixin for LuxtensorClient

Provides weight-related query methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class WeightsMixin:
    """
    Mixin providing weight-related query methods.

    Requires:
        RPCProvider protocol (provided by BaseClient)

    Methods:
        - get_weights_version() - Get weights version for subnet
        - get_weight_commits() - Get weight commit hashes
        - get_weights_rate_limit() - Get rate limit for setting weights
        - get_max_weight_limit() - Get maximum weight limit
        - get_min_allowed_weights() - Get minimum allowed weights
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self) -> "WeightsMixin":
            """At runtime, return self (duck typing)."""
            return self

    def get_weights_version(self, subnet_id: int) -> int:
        """
        Get weights version for a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Weights version number
        """
        try:
            result = self._rpc()._call_rpc("query_weightsVersion", [subnet_id])
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting weights version for subnet {subnet_id}: {e}")
            return 0

    def get_weight_commits(self, subnet_id: int) -> List[str]:
        """
        Get weight commit hashes for a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            List of commit hashes
        """
        try:
            result = self._rpc()._call_rpc("query_weightCommits", [subnet_id])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.error(f"Error getting weight commits for subnet {subnet_id}: {e}")
            return []

    def get_weights_rate_limit(self, subnet_id: int) -> int:
        """
        Get rate limit for setting weights.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Rate limit in blocks
        """
        try:
            # Derive from subnet_getHyperparameters
            hp = self._rpc()._call_rpc("subnet_getHyperparameters", [subnet_id])
            if hp:
                val = hp.get("weights_set_rate_limit", hp.get("weightsSetRateLimit", 0))
                return int(val) if val else 0
            return 0
        except Exception as e:
            logger.error(f"Error getting weights rate limit for subnet {subnet_id}: {e}")
            return 0

    def get_max_weight_limit(self, subnet_id: int) -> float:
        """
        Get maximum weight limit for a single weight.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Maximum weight as float (0.0 to 1.0)
        """
        try:
            # Derive from subnet_getHyperparameters
            hp = self._rpc()._call_rpc("subnet_getHyperparameters", [subnet_id])
            if hp:
                val = hp.get("max_weight_limit", hp.get("maxWeightLimit", 0))
                return float(val) / 65535.0 if val else 0.0
            return 0.0
        except Exception as e:
            logger.error(f"Error getting max weight limit for subnet {subnet_id}: {e}")
            return 0.0

    def get_min_allowed_weights(self, subnet_id: int) -> int:
        """
        Get minimum number of weights that must be set.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Minimum allowed weights count
        """
        try:
            # Derive from subnet_getHyperparameters
            hp = self._rpc()._call_rpc("subnet_getHyperparameters", [subnet_id])
            if hp:
                val = hp.get("min_allowed_weights", hp.get("minAllowedWeights", 0))
                return int(val) if val else 0
            return 0
        except Exception as e:
            logger.error(f"Error getting min allowed weights for subnet {subnet_id}: {e}")
            return 0

    def commit_weights_merkle(
        self,
        subnet_id: int,
        weights: List[tuple[int, int]],
        epoch: int,
        validator_address: str,
    ) -> Dict[str, Any]:
        """
        Commit weights via Merkle root (gas-optimized).

        This method uses Merkle batching to submit weights with fixed 21K gas
        regardless of miner count. Recommended for subnets with 100+ miners.

        Args:
            subnet_id: Subnet identifier
            weights: List of (miner_uid, weight) tuples
            epoch: Current epoch number
            validator_address: Validator's address (0x prefixed)

        Returns:
            Dict with merkleRoot, gasUsed, minerCount, etc.

        Example:
            >>> weights = [(1, 100), (2, 200), (3, 300)]
            >>> result = client.commit_weights_merkle(1, weights, 100, "0x123...")
            >>> print(result["gasUsed"])  # Always 21000
        """
        try:
            weights_array = [[uid, weight] for uid, weight in weights]
            result = self._rpc()._call_rpc(
                "weight_setWeights",
                [subnet_id, weights_array, epoch, validator_address],
            )
            return result if result else {}
        except Exception as e:
            logger.error(f"Error committing Merkle weights for subnet {subnet_id}: {e}")
            return {"success": False, "error": str(e)}

