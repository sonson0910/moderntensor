"""
Subnet 0 (Root Subnet) Mixin for LuxtensorClient

Provides Root Subnet operations.
Synced with luxtensor-rpc/src/subnet_rpc.rs
"""

from typing import Dict, Any, List, TYPE_CHECKING
import logging

if TYPE_CHECKING:
    from .base import BaseClient

logger = logging.getLogger(__name__)


class Subnet0Mixin:
    """
    Mixin providing Subnet 0 (Root Subnet) methods.

    These methods synchronize with the Rust backend:
    - luxtensor-rpc/src/subnet_rpc.rs

    Methods:
        - get_all_subnets()
        - register_subnet()
        - get_root_validators()
        - is_root_validator()
        - set_root_weights()
        - get_root_config()
        - get_emission_distribution()
    """

    _call_rpc: callable

    def get_all_subnets(self) -> List[Dict[str, Any]]:
        """
        Get all registered subnets.

        Returns:
            List of SubnetInfo dictionaries
        """
        try:
            result = self._call_rpc("subnet_getAll", [])
            return result if result else []
        except Exception as e:
            logger.error(f"Failed to get all subnets: {e}")
            return []

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
            result = self._call_rpc("subnet_register", [name, owner])
            return result
        except Exception as e:
            logger.error(f"Failed to register subnet: {e}")
            return {"success": False, "error": str(e)}

    def get_root_validators(self) -> List[Dict[str, Any]]:
        """
        Get list of root validators (top stakers in Subnet 0).

        Returns:
            List of RootValidatorInfo dictionaries
        """
        try:
            result = self._call_rpc("subnet_getRootValidators", [])
            return result if result else []
        except Exception as e:
            logger.error(f"Failed to get root validators: {e}")
            return []

    def is_root_validator(self, address: str) -> bool:
        """
        Check if address is a root validator.

        Args:
            address: Address to check (0x...)

        Returns:
            True if root validator
        """
        try:
            result = self._call_rpc("subnet_isRootValidator", [address])
            return bool(result)
        except Exception as e:
            logger.error(f"Failed to check root validator: {e}")
            return False

    def set_root_weights(
        self,
        validator: str,
        weights: Dict[int, float]
    ) -> bool:
        """
        Set root weights for subnets (root validator only).

        Args:
            validator: Validator address
            weights: Dict of netuid -> weight

        Returns:
            True if successful
        """
        try:
            result = self._call_rpc("subnet_setWeights", [validator, weights])
            return result.get("success", False) if result else False
        except Exception as e:
            logger.error(f"Failed to set root weights: {e}")
            return False

    def get_root_config(self) -> Dict[str, Any]:
        """
        Get Subnet 0 configuration.

        Returns:
            RootConfig dictionary
        """
        try:
            result = self._call_rpc("subnet_getConfig", [])
            return result if result else {}
        except Exception as e:
            logger.error(f"Failed to get root config: {e}")
            return {}

    def get_emission_distribution(self) -> Dict[int, float]:
        """
        Get emission distribution for all subnets.

        Returns:
            Dict of netuid -> emission share
        """
        try:
            result = self._call_rpc("subnet_getEmissions", [])
            return result if result else {}
        except Exception as e:
            logger.error(f"Failed to get emission distribution: {e}")
            return {}

    def get_subnet(self, netuid: int) -> Dict[str, Any]:
        """
        Get single subnet info.

        Args:
            netuid: Subnet ID

        Returns:
            SubnetInfo dictionary or empty dict
        """
        try:
            result = self._call_rpc("subnet_get", [netuid])
            return result if result else {}
        except Exception as e:
            logger.error(f"Failed to get subnet {netuid}: {e}")
            return {}
