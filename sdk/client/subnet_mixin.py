"""
Subnet Mixin for LuxtensorClient

Provides subnet query methods.
"""

from typing import Dict, Any, List, Optional
import logging

logger = logging.getLogger(__name__)


class SubnetMixin:
    """
    Mixin providing subnet query methods.

    Methods:
        - get_all_subnets()
        - get_subnet_info()
        - get_subnet_hyperparameters()
        - subnet_exists()
        - get_subnet_count()
    """

    def get_all_subnets(self) -> List[Dict[str, Any]]:
        """
        Get list of all subnets.

        Returns:
            List of subnet info
        """
        try:
            result = self._call_rpc("subnet_getAll", [])
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
            return self._call_rpc("subnet_getInfo", [subnet_uid])
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
            result = self._call_rpc("subnet_getHyperparameters", [subnet_uid])
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
            result = self._call_rpc("subnet_exists", [subnet_uid])
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
            result = self._call_rpc("subnet_getCount", [])
            return int(result) if result else 0
        except Exception as e:
            logger.warning(f"Failed to get subnet count: {e}")
            return 0
