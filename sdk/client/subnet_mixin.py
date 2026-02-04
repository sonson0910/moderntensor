"""
Subnet Mixin for LuxtensorClient

Provides subnet query methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class SubnetMixin:
    """
    Mixin providing subnet query and management methods.

    Query Methods:
        - get_all_subnets() - List all subnets
        - get_subnet_info() - Get subnet details
        - get_subnet_hyperparameters() - Get subnet config
        - subnet_exists() - Check subnet existence
        - get_subnet_count() - Total subnet count
        - get_subnet_tempo() - Get epoch length
        - get_subnet_emission() - Get emission rate
        - get_subnet_emissions() - Get all emission distribution
        - get_subnet_owner() - Get subnet owner address
        - get_subnet_registration_allowed() - Check registration status
        - get_subnet_network_metadata() - Get network metadata
        - get_subnetwork_n() - Get subnetwork count
        - get_total_subnets() - Total subnet count
        - get_max_subnets() - Maximum allowed subnets

    Transaction Methods:
        - register_subnet() - Register new subnet
        - set_subnet_weights() - Set subnet weights (root validators)
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def get_all_subnets(self) -> List[Dict[str, Any]]:
        """
        Get list of all subnets.

        Returns:
            List of subnet info
        """
        try:
            result = self._rpc()._call_rpc("subnet_getAll", [])
            return result if result else []
        except Exception as e:
            logger.warning(f"Failed to get subnets: {e}")
            return []

    def get_subnet_info(self, subnet_uid: int) -> Optional[Dict[str, Any]]:
        """
        Get subnet information.

        Args:
            subnet_uid: Subnet unique identifier

        Returns:
            Subnet info or None
        """
        try:
            return self._rpc()._call_rpc("subnet_getInfo", [subnet_uid])
        except Exception as e:
            logger.warning(f"Failed to get subnet {subnet_uid}: {e}")
            return None

    def get_subnet_hyperparameters(self, subnet_uid: int) -> Dict[str, Any]:
        """
        Get subnet hyperparameters.

        Args:
            subnet_uid: Subnet unique identifier

        Returns:
            Hyperparameters dict
        """
        try:
            result = self._rpc()._call_rpc("subnet_getHyperparameters", [subnet_uid])
            return result if result else {}
        except Exception as e:
            logger.warning(f"Failed to get hyperparameters for subnet {subnet_uid}: {e}")
            return {}

    def subnet_exists(self, subnet_uid: int) -> bool:
        """
        Check if subnet exists.

        Args:
            subnet_uid: Subnet unique identifier

        Returns:
            True if exists
        """
        try:
            result = self._rpc()._call_rpc("subnet_exists", [subnet_uid])
            return bool(result)
        except Exception:
            return False

    def get_subnet_count(self) -> int:
        """
        Get total number of subnets.

        Returns:
            Subnet count
        """
        try:
            result = self._rpc()._call_rpc("subnet_getCount", [])
            return int(result) if result else 0
        except Exception as e:
            logger.warning(f"Failed to get subnet count: {e}")
            return 0

    # Extended Query Methods

    def get_subnet_tempo(self, subnet_id: int) -> int:
        """
        Get tempo (epoch length) for a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Tempo in blocks
        """
        try:
            result = self._rpc()._call_rpc("query_subnetTempo", [subnet_id])
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting tempo for subnet {subnet_id}: {e}")
            return 0

    def get_subnet_emission(self, subnet_id: int) -> int:
        """
        Get emission rate for a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Emission rate
        """
        try:
            result = self._rpc()._call_rpc("query_subnetEmission", [subnet_id])
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting emission for subnet {subnet_id}: {e}")
            return 0

    def get_subnet_emissions(self, total_emission: Optional[int] = None) -> List[Dict[str, Any]]:
        """
        Get emission distribution for all subnets.

        Args:
            total_emission: Total emission amount (default: 1000 MDT in LTS)

        Returns:
            List of EmissionShare dictionaries
        """
        try:
            params = []
            if total_emission is not None:
                params.append(f"0x{total_emission:x}")
            result = self._rpc()._call_rpc("subnet_getEmissions", params)
            return result if result else []
        except Exception as e:
            logger.error(f"Failed to get subnet emissions: {e}")
            return []

    def get_subnet_owner(self, subnet_id: int) -> str:
        """
        Get owner address of a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Owner address
        """
        try:
            result = self._rpc()._call_rpc("query_subnetOwner", [subnet_id])
            return result if result else ""
        except Exception as e:
            logger.error(f"Error getting owner for subnet {subnet_id}: {e}")
            return ""

    def get_subnet_registration_allowed(self, subnet_id: int) -> bool:
        """
        Check if registration is allowed in a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            True if registration allowed, False otherwise
        """
        try:
            result = self._rpc()._call_rpc("query_subnetRegistrationAllowed", [subnet_id])
            return bool(result)
        except Exception as e:
            logger.error(f"Error checking registration allowed for subnet {subnet_id}: {e}")
            return False

    def get_subnet_network_metadata(self, subnet_id: int) -> Dict[str, Any]:
        """
        Get network metadata for a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Network metadata including URLs, descriptions, etc.
        """
        try:
            result = self._rpc()._call_rpc("query_subnetNetworkMetadata", [subnet_id])
            return result if result else {}
        except Exception as e:
            logger.error(f"Error getting network metadata for subnet {subnet_id}: {e}")
            return {}

    def get_subnetwork_n(self, subnet_id: int) -> int:
        """
        Get number of subnetworks in a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Number of subnetworks
        """
        try:
            result = self._rpc()._call_rpc("query_subnetworkN", [subnet_id])
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting subnetwork N for subnet {subnet_id}: {e}")
            return 0

    def get_total_subnets(self) -> int:
        """
        Get total number of subnets.

        Returns:
            Number of subnets
        """
        try:
            result = self._rpc()._call_rpc("query_totalSubnets")
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting total subnets: {e}")
            return 0

    def get_max_subnets(self) -> int:
        """
        Get maximum number of subnets allowed.

        Returns:
            Maximum subnets
        """
        try:
            result = self._rpc()._call_rpc("query_maxSubnets")
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting max subnets: {e}")
            return 0

    # Transaction Methods

    def register_subnet(self, name: str, owner: str) -> Dict[str, Any]:
        """
        Register a new subnet (Subnet 0 operation).

        Args:
            name: Human-readable subnet name
            owner: Owner address (0x...)

        Returns:
            Registration result with netuid if successful
        """
        try:
            result = self._rpc()._call_rpc("subnet_register", [name, owner])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to register subnet: {e}")
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
            # Convert int keys to strings for JSON
            weights_json = {str(k): v for k, v in weights.items()}
            result = self._rpc()._call_rpc("subnet_setWeights", [validator, weights_json])
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error(f"Failed to set subnet weights: {e}")
            return {"success": False, "error": str(e)}

    # Network Metrics

    def get_activity_cutoff(self, subnet_id: int) -> int:
        """
        Get activity cutoff for a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Activity cutoff in blocks
        """
        try:
            result = self._rpc()._call_rpc("query_activityCutoff", [subnet_id])
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting activity cutoff for subnet {subnet_id}: {e}")
            return 0

    def get_difficulty(self, subnet_id: int) -> int:
        """
        Get registration difficulty for a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Difficulty value
        """
        try:
            result = self._rpc()._call_rpc("query_difficulty", [subnet_id])
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting difficulty for subnet {subnet_id}: {e}")
            return 0

    def get_burn_cost(self, subnet_id: int) -> int:
        """
        Get burn cost for registration in a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Burn cost in tokens
        """
        try:
            result = self._rpc()._call_rpc("query_burnCost", [subnet_id])
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting burn cost for subnet {subnet_id}: {e}")
            return 0
