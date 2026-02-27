"""
Subnet Mixin for LuxtensorClient

Wraps lux_* RPC methods for querying subnet data from MetagraphDB.
All reads go through MetagraphDB (RocksDB-backed persistent storage).
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class SubnetMixin:
    """
    Mixin providing subnet query and management methods.

    All query methods use the new `lux_*` RPC namespace backed by MetagraphDB.

    Query Methods:
        - get_all_subnets()              — List all subnets (lux_listSubnets)
        - get_subnet_info()              — Get subnet details (lux_getSubnetInfo)
        - get_subnet_count()             — Total subnet count (lux_getSubnetCount)
        - get_subnet_emissions()         — Emission info (lux_getEmissions)
        - get_subnet_tempo()             — Tempo field from subnet data
        - get_subnet_emission()          — Emission rate field from subnet data
        - get_subnet_owner()             — Owner address from subnet data
        - get_subnetwork_n()             — Neuron count for subnet
        - get_total_subnets()            — Alias for get_subnet_count()
        - get_max_subnets()              — Max neurons allowed in subnet
        - subnet_exists()                — Check subnet existence
        - get_metagraph()                — Full metagraph snapshot (lux_getMetagraph)

    Transaction Methods:
        - register_subnet()              — Register new subnet (subnet_create)
        - set_subnet_weights()           — Set subnet weights for root validators
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    # ------------------------------------------------------------------
    # Primary Query Methods
    # ------------------------------------------------------------------

    def get_all_subnets(self) -> List[Dict[str, Any]]:
        """
        Get list of all subnets from MetagraphDB.

        Returns:
            List of subnet dicts from `lux_listSubnets`
        """
        try:
            result = self._rpc()._call_rpc("lux_listSubnets", [])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning("Failed to get subnets: %s", e)
            return []

    def get_subnet_info(self, subnet_uid: int) -> Optional[Dict[str, Any]]:
        """
        Get subnet information from MetagraphDB.

        Args:
            subnet_uid: Subnet unique identifier

        Returns:
            Subnet info dict or None
        """
        try:
            return self._rpc()._call_rpc("lux_getSubnetInfo", [subnet_uid])
        except Exception as e:
            logger.warning("Failed to get subnet %s: %s", subnet_uid, e)
            return None

    def get_subnet_count(self) -> int:
        """
        Get total number of subnets.

        Returns:
            Subnet count
        """
        try:
            result = self._rpc()._call_rpc("lux_getSubnetCount", [])
            if isinstance(result, dict):
                return int(result.get("count", 0))
            return int(result) if result else 0
        except Exception as e:
            logger.warning("Failed to get subnet count: %s", e)
            return 0

    def subnet_exists(self, subnet_uid: int) -> bool:
        """
        Check if subnet exists in MetagraphDB.

        Args:
            subnet_uid: Subnet unique identifier

        Returns:
            True if subnet data is found
        """
        try:
            result = self._rpc()._call_rpc("lux_getSubnet", [subnet_uid])
            return result is not None
        except Exception:
            return False

    def get_metagraph(self, subnet_id: int) -> Dict[str, Any]:
        """
        Get full metagraph snapshot for a subnet.
        Includes subnet info, all neurons, and weight matrix.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Dict with subnet, neurons, weight_matrix, neuron_count, weight_count
        """
        try:
            result = self._rpc()._call_rpc("lux_getMetagraph", [subnet_id])
            return result if isinstance(result, dict) else {}
        except Exception as e:
            logger.error("Failed to get metagraph for subnet %s: %s", subnet_id, e)
            return {}

    # ------------------------------------------------------------------
    # Derived convenience methods (read from subnet data fields)
    # ------------------------------------------------------------------

    def get_subnet_tempo(self, subnet_id: int) -> int:
        """Get tempo (epoch length in blocks) for a subnet."""
        try:
            result = self._rpc()._call_rpc("lux_getSubnetInfo", [subnet_id])
            if result:
                return int(result.get("tempo", 0))
            return 0
        except Exception as e:
            logger.error("Error getting tempo for subnet %s: %s", subnet_id, e)
            return 0

    def get_subnet_emission(self, subnet_id: int) -> int:
        """Get emission rate for a subnet."""
        try:
            result = self._rpc()._call_rpc("lux_getEmissions", [subnet_id])
            if result:
                raw = result.get("emission_rate_decimal", "0")
                return int(raw) if raw else 0
            return 0
        except Exception as e:
            logger.error("Error getting emission for subnet %s: %s", subnet_id, e)
            return 0

    def get_subnet_emissions(self, total_emission: Optional[int] = None) -> List[Dict[str, Any]]:
        """
        Get emission info for all subnets.

        Args:
            total_emission: Ignored (kept for API compat with Bittensor SDK)

        Returns:
            List of EmissionShare dicts with id, name, emission_rate fields
        """
        try:
            subnets = self._rpc()._call_rpc("lux_listSubnets", [])
            if not subnets:
                return []
            return [
                {
                    "subnet_id": s.get("id"),
                    "name": s.get("name"),
                    "emission_rate": s.get("emission_rate"),
                    "emission_rate_decimal": s.get("emission_rate_decimal"),
                    "active": s.get("active"),
                }
                for s in subnets
            ]
        except Exception as e:
            logger.error("Failed to get subnet emissions: %s", e)
            return []

    def get_subnet_owner(self, subnet_id: int) -> str:
        """Get owner address of a subnet."""
        try:
            result = self._rpc()._call_rpc("lux_getSubnetInfo", [subnet_id])
            return result.get("owner", "") if result else ""
        except Exception as e:
            logger.error("Error getting owner for subnet %s: %s", subnet_id, e)
            return ""

    def get_subnet_registration_allowed(self, subnet_id: int) -> bool:
        """Check if registration is allowed (subnet active + not at max capacity)."""
        try:
            info = self._rpc()._call_rpc("lux_getSubnetInfo", [subnet_id])
            if info:
                return bool(info.get("active", True))
            return False
        except Exception as e:
            logger.error("Error checking registration for subnet %s: %s", subnet_id, e)
            return False

    def get_subnet_network_metadata(self, subnet_id: int) -> Dict[str, Any]:
        """Get network metadata for a subnet (full subnet data)."""
        try:
            result = self._rpc()._call_rpc("lux_getSubnetInfo", [subnet_id])
            return result if isinstance(result, dict) else {}
        except Exception as e:
            logger.error("Error getting metadata for subnet %s: %s", subnet_id, e)
            return {}

    def get_subnetwork_n(self, subnet_id: int) -> int:
        """Get number of neurons in a subnet."""
        try:
            result = self._rpc()._call_rpc("lux_getNeuronCount", [subnet_id])
            if isinstance(result, dict):
                return int(result.get("count", 0))
            return int(result) if result else 0
        except Exception as e:
            logger.error("Error getting subnetwork N for subnet %s: %s", subnet_id, e)
            return 0

    def get_total_subnets(self) -> int:
        """Get total number of subnets (alias for get_subnet_count)."""
        return self.get_subnet_count()

    def get_max_subnets(self) -> int:
        """Get max neurons allowed in subnet (from subnet data max_neurons field)."""
        try:
            # max_neurons is per-subnet limit, not a global limit
            # Return sum across all subnets or max_neurons of root subnet
            root = self._rpc()._call_rpc("lux_getSubnetInfo", [0])
            if root:
                return int(root.get("max_neurons", 0))
            return 0
        except Exception as e:
            logger.error("Error getting max subnets: %s", e)
            return 0

    def get_subnet_hyperparameters(self, subnet_uid: int) -> Dict[str, Any]:
        """
        Get subnet hyperparameters (derived from subnet info).

        Args:
            subnet_uid: Subnet unique identifier

        Returns:
            Hyperparameters dict
        """
        try:
            result = self._rpc()._call_rpc("lux_getSubnetInfo", [subnet_uid])
            if not result:
                return {}
            # Map SubnetData fields to hyperparameter names
            return {
                "tempo": result.get("tempo", 0),
                "max_neurons": result.get("max_neurons", 0),
                "min_stake": result.get("min_stake_decimal", "0"),
                "emission_rate": result.get("emission_rate_decimal", "0"),
                "active": result.get("active", False),
                "registration_allowed": result.get("active", False),
            }
        except Exception as e:
            logger.warning("Failed to get hyperparameters for subnet %s: %s", subnet_uid, e)
            return {}

    def get_activity_cutoff(self, subnet_id: int) -> int:
        """Get activity cutoff — not stored in MetagraphDB, returns 0."""
        return 0

    def get_difficulty(self, subnet_id: int) -> int:
        """Get registration difficulty — not stored in MetagraphDB, returns 0."""
        return 0

    def get_burn_cost(self, subnet_id: int) -> int:
        """Get burn cost — not stored in MetagraphDB, returns 0."""
        return 0

    # ------------------------------------------------------------------
    # Transaction Methods
    # ------------------------------------------------------------------

    def register_subnet(self, name: str, owner: str) -> Dict[str, Any]:
        """
        Register a new subnet.

        Args:
            name: Human-readable subnet name
            owner: Owner address (0x...)

        Returns:
            Registration result with netuid if successful
        """
        try:
            result = self._rpc()._call_rpc("subnet_create", [name, owner])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error("Failed to register subnet: %s", e)
            return {"success": False, "error": str(e)}

    def set_subnet_weights(self, validator: str, weights: Dict[int, float]) -> Dict[str, Any]:
        """
        Set subnet weights for a root validator.

        Args:
            validator: Root validator address
            weights: Dict of netuid -> weight (0.0 - 1.0)

        Returns:
            Result with success status
        """
        try:
            weights_json = {str(k): v for k, v in weights.items()}
            result = self._rpc()._call_rpc("weight_setWeights", [validator, weights_json])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error("Failed to set subnet weights: %s", e)
            return {"success": False, "error": str(e)}
