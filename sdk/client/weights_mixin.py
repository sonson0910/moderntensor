"""
Weights Mixin for LuxtensorClient

Wraps lux_* and weight_* RPC methods for querying and committing
neuron weight data stored in MetagraphDB.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, Tuple, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class WeightsMixin:
    """
    Mixin providing weight query and commit methods.

    Query methods use `lux_*` namespace (reading from MetagraphDB).
    Write methods use `weight_*` namespace.

    Methods:
        - get_weights()                  — Weights set by a neuron (lux_getWeights)
        - get_all_weights()              — Full weight matrix for a subnet (lux_getAllWeights)
        - get_weight_version()           — Weight version for subnet
        - get_weight_commit()            — Last weight commit block
        - get_weight_rate_limit()        — Blocks between allowed weight sets
        - get_max_weight_limit()         — Max weight value
        - get_min_allowed_weights()      — Minimum number of weights a neuron must set
        - commit_weights()               — Commit weight vector on-chain
        - commit_weights_merkle()        — Commit Merkle root of weights
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    # ------------------------------------------------------------------
    # Query Methods
    # ------------------------------------------------------------------

    def get_weights(
        self, subnet_uid: int, from_uid: int
    ) -> List[Dict[str, Any]]:
        """
        Get weights that a specific neuron has set in a subnet.

        Args:
            subnet_uid: Subnet identifier
            from_uid: Source neuron UID (the neuron that set the weights)

        Returns:
            List of WeightData dicts with from_uid, to_uid, weight, block_set
        """
        try:
            result = self._rpc()._call_rpc("lux_getWeights", [subnet_uid, from_uid])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning(
                f"Failed to get weights for neuron {from_uid} in subnet {subnet_uid}: {e}"
            )
            return []

    def get_all_weights(self, subnet_uid: int) -> List[Dict[str, Any]]:
        """
        Get the complete weight matrix for a subnet.

        Args:
            subnet_uid: Subnet identifier

        Returns:
            List of all WeightData entries across all neurons in the subnet
        """
        try:
            result = self._rpc()._call_rpc("lux_getAllWeights", [subnet_uid])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning("Failed to get all weights for subnet %s: %s", subnet_uid, e)
            return []

    def get_weight_matrix(self, subnet_uid: int) -> List[Tuple[int, int, float]]:
        """
        Get the weight matrix as a list of (from_uid, to_uid, weight) tuples.

        Args:
            subnet_uid: Subnet identifier

        Returns:
            List of (from_uid, to_uid, weight) tuples
        """
        try:
            weights = self.get_all_weights(subnet_uid)
            return [
                (
                    int(w.get("from_uid", 0)),
                    int(w.get("to_uid", 0)),
                    float(w.get("weight", 0.0)),
                )
                for w in weights
            ]
        except Exception as e:
            logger.warning("Failed to get weight matrix for subnet %s: %s", subnet_uid, e)
            return []

    def get_weight_version(self, subnet_uid: int) -> int:
        """
        Get the weight version key for a subnet.
        Used to prevent replay of stale weight commits.

        Args:
            subnet_uid: Subnet identifier

        Returns:
            Weight version integer or 0
        """
        try:
            result = self._rpc()._call_rpc("weight_getVersion", [subnet_uid])
            if isinstance(result, dict):
                return int(result.get("version", 0))
            return int(result) if result else 0
        except Exception as e:
            logger.warning("Failed to get weight version for subnet %s: %s", subnet_uid, e)
            return 0

    def get_weight_commit(self, subnet_uid: int, uid: int) -> int:
        """
        Get the last block at which a neuron committed weights.

        Args:
            subnet_uid: Subnet identifier
            uid: Neuron UID

        Returns:
            Block number of last commit or 0
        """
        try:
            weights = self._rpc()._call_rpc("lux_getWeights", [subnet_uid, uid])
            if weights and isinstance(weights, list) and len(weights) > 0:
                # Return the highest block_set value
                blocks = [w.get("block_set", 0) for w in weights]
                return max(blocks) if blocks else 0
            return 0
        except Exception as e:
            logger.warning(
                f"Failed to get weight commit for neuron {uid} in subnet {subnet_uid}: {e}"
            )
            return 0

    def get_weight_rate_limit(self, subnet_uid: int) -> int:
        """
        Get the rate limit (blocks) between allowed weight commits for this subnet.

        Args:
            subnet_uid: Subnet identifier

        Returns:
            Number of blocks between allowed weight sets
        """
        try:
            result = self._rpc()._call_rpc("weight_getRateLimit", [subnet_uid])
            if isinstance(result, dict):
                return int(result.get("rate_limit", 0))
            return int(result) if result else 0
        except Exception as e:
            logger.warning("Failed to get weight rate limit for subnet %s: %s", subnet_uid, e)
            return 0

    def get_max_weight_limit(self, subnet_uid: int) -> float:
        """
        Get maximum weight value allowed (normalized to [0.0, 1.0]).

        Args:
            subnet_uid: Subnet identifier

        Returns:
            Max weight limit (float), or 1.0 if not configured
        """
        try:
            result = self._rpc()._call_rpc("weight_getMaxLimit", [subnet_uid])
            if isinstance(result, dict):
                return float(result.get("max_weight", 1.0))
            return float(result) if result else 1.0
        except Exception as e:
            logger.warning("Failed to get max weight limit for subnet %s: %s", subnet_uid, e)
            return 1.0

    def get_min_allowed_weights(self, subnet_uid: int) -> int:
        """
        Get minimum number of weight assignments required per commit.

        Args:
            subnet_uid: Subnet identifier

        Returns:
            Minimum weight count or 0
        """
        try:
            result = self._rpc()._call_rpc("weight_getMinAllowed", [subnet_uid])
            if isinstance(result, dict):
                return int(result.get("min_weights", 0))
            return int(result) if result else 0
        except Exception as e:
            logger.warning("Failed to get min allowed weights for subnet %s: %s", subnet_uid, e)
            return 0

    # ------------------------------------------------------------------
    # Write Methods
    # ------------------------------------------------------------------

    def commit_weights(
        self,
        subnet_uid: int,
        from_uid: int,
        weights: Dict[int, float],
        signature: str,
    ) -> Dict[str, Any]:
        """
        Commit a weight vector for a neuron on-chain.

        Args:
            subnet_uid: Subnet identifier
            from_uid: Source neuron UID (the neuron setting weights)
            weights: Dict of {to_uid: weight_value} (weights should sum to 1.0)
            signature: Secp256k1 signature over the weight commit payload

        Returns:
            Result dict with success status
        """
        try:
            # Normalize weights to u16 range [0, 65535]
            weights_norm = {str(uid): float(w) for uid, w in weights.items()}
            params = {
                "subnet_id": subnet_uid,
                "from_uid": from_uid,
                "weights": weights_norm,
                "signature": signature,
            }
            result = self._rpc()._call_rpc("weight_setWeights", [params])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error("Failed to commit weights for neuron %s: %s", from_uid, e)
            return {"success": False, "error": str(e)}

    def commit_weights_merkle(
        self,
        subnet_uid: int,
        from_uid: int,
        merkle_root: str,
        version_key: int,
        signature: str,
    ) -> Dict[str, Any]:
        """
        Commit a Merkle root of weights (two-phase commit scheme).

        In Phase 1, only the Merkle root is committed on-chain.
        In Phase 2 (reveal), the actual weights can be verified against the root.

        Args:
            subnet_uid: Subnet identifier
            from_uid: Source neuron UID
            merkle_root: Hex-encoded Merkle root of the weight vector
            version_key: Weight version to prevent replay
            signature: Secp256k1 signature over commit payload

        Returns:
            Result dict with success, tx_hash
        """
        try:
            params = {
                "subnet_id": subnet_uid,
                "from_uid": from_uid,
                "merkle_root": merkle_root,
                "version_key": version_key,
                "signature": signature,
            }
            result = self._rpc()._call_rpc("weight_commitMerkle", [params])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error("Failed to commit Merkle weights for neuron %s: %s", from_uid, e)
            return {"success": False, "error": str(e)}
