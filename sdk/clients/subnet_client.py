"""
Subnet Client
Handles subnet queries and management.
"""

import logging
from typing import Optional, Dict, Any, List
from .base import BaseRpcClient

logger = logging.getLogger(__name__)


class SubnetClient(BaseRpcClient):
    """
    Client for subnet operations.
    Single Responsibility: Subnet management only.
    """

    def get_subnet_info(self, subnet_id: int) -> Dict[str, Any]:
        """
        Get subnet information.

        Args:
            subnet_id: Subnet ID

        Returns:
            Subnet metadata and configuration
        """
        try:
            result = self._call_rpc("subnet_getInfo", [subnet_id])
            return result if result else {}
        except Exception as e:
            logger.error(f"Failed to get subnet info for {subnet_id}: {e}")
            raise

    def get_all_subnets(self) -> List[Dict[str, Any]]:
        """
        Get all subnets.

        Returns:
            List of subnet info
        """
        try:
            result = self._call_rpc("subnet_list", [])
            return result if result else []
        except Exception as e:
            logger.warning(f"Failed to get subnets: {e}")
            return []

    def subnet_exists(self, subnet_id: int) -> bool:
        """
        Check if subnet exists.

        Args:
            subnet_id: Subnet ID

        Returns:
            True if exists
        """
        try:
            result = self._call_rpc("subnet_exists", [subnet_id])
            return bool(result)
        except Exception:
            return False

    def get_total_subnets(self) -> int:
        """
        Get total subnet count.

        Returns:
            Number of subnets
        """
        try:
            result = self._call_rpc("subnet_count", [])
            return result if result else 0
        except Exception as e:
            logger.warning(f"Failed to get subnet count: {e}")
            return 0

    # ========================================================================
    # Hyperparameters
    # ========================================================================

    def get_subnet_hyperparameters(self, subnet_id: int) -> Dict[str, Any]:
        """
        Get subnet hyperparameters.

        Args:
            subnet_id: Subnet ID

        Returns:
            Hyperparameter dict
        """
        try:
            result = self._call_rpc("subnet_getHyperparameters", [subnet_id])
            return result if result else {}
        except Exception as e:
            logger.warning(f"Failed to get hyperparameters: {e}")
            return {}

    def get_subnet_tempo(self, subnet_id: int) -> int:
        """Get subnet tempo (blocks per epoch)"""
        return self._get_param(subnet_id, "tempo", 0)

    def get_subnet_emission(self, subnet_id: int) -> float:
        """Get subnet emission rate"""
        return self._get_param(subnet_id, "emission", 0.0)

    def get_difficulty(self, subnet_id: int) -> int:
        """Get PoW difficulty"""
        return self._get_param(subnet_id, "difficulty", 0)

    def get_immunity_period(self, subnet_id: int) -> int:
        """Get immunity period in blocks"""
        return self._get_param(subnet_id, "immunityPeriod", 0)

    def get_burn_cost(self, subnet_id: int) -> int:
        """Get registration burn cost"""
        return self._get_param(subnet_id, "burnCost", 0)

    def get_max_allowed_validators(self, subnet_id: int) -> int:
        """Get max validators for subnet"""
        return self._get_param(subnet_id, "maxAllowedValidators", 0)

    def get_rho(self, subnet_id: int) -> float:
        """Get rho parameter"""
        return self._get_param(subnet_id, "rho", 0.0)

    def get_kappa(self, subnet_id: int) -> float:
        """Get kappa parameter"""
        return self._get_param(subnet_id, "kappa", 0.0)

    def _get_param(self, subnet_id: int, param: str, default: Any) -> Any:
        """Helper to get single hyperparameter"""
        try:
            result = self._call_rpc(f"subnet_get{param.capitalize()}", [subnet_id])
            return result if result is not None else default
        except Exception as e:
            logger.warning(f"Failed to get {param}: {e}")
            return default
