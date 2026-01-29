"""
Neuron Mixin for LuxtensorClient

Provides neuron query methods.
"""

from typing import Dict, Any, List, Optional
import logging

logger = logging.getLogger(__name__)


class NeuronMixin:
    """
    Mixin providing neuron query methods.

    Methods:
        - get_neuron()
        - get_neurons()
        - get_neuron_count()
        - neuron_exists()
    """

    def get_neuron(self, subnet_uid: int, neuron_uid: int) -> Optional[Dict[str, Any]]:
        """
        Get neuron information.

        Args:
            subnet_uid: Subnet unique identifier
            neuron_uid: Neuron unique identifier

        Returns:
            Neuron info or None
        """
        try:
            return self._call_rpc("neuron_get", [subnet_uid, neuron_uid])
        except Exception as e:
            logger.warning(f"Failed to get neuron {neuron_uid} in subnet {subnet_uid}: {e}")
            return None

    def get_neurons(self, subnet_uid: int) -> List[Dict[str, Any]]:
        """
        Get all neurons in a subnet.

        Args:
            subnet_uid: Subnet unique identifier

        Returns:
            List of neuron info
        """
        try:
            result = self._call_rpc("neuron_getAll", [subnet_uid])
            return result if result else []
        except Exception as e:
            logger.warning(f"Failed to get neurons for subnet {subnet_uid}: {e}")
            return []

    def get_neuron_count(self, subnet_uid: int) -> int:
        """
        Get number of neurons in subnet.

        Args:
            subnet_uid: Subnet unique identifier

        Returns:
            Neuron count
        """
        try:
            result = self._call_rpc("neuron_getCount", [subnet_uid])
            return int(result) if result else 0
        except Exception as e:
            logger.warning(f"Failed to get neuron count: {e}")
            return 0

    def neuron_exists(self, subnet_uid: int, neuron_uid: int) -> bool:
        """
        Check if neuron exists.

        Args:
            subnet_uid: Subnet unique identifier
            neuron_uid: Neuron unique identifier

        Returns:
            True if exists
        """
        try:
            result = self._call_rpc("neuron_exists", [subnet_uid, neuron_uid])
            return bool(result)
        except Exception:
            return False

    def get_weights(self, subnet_uid: int, neuron_uid: int) -> List[tuple]:
        """
        Get weights set by a neuron.

        Args:
            subnet_uid: Subnet unique identifier
            neuron_uid: Neuron unique identifier

        Returns:
            List of (target_uid, weight) tuples
        """
        try:
            result = self._call_rpc("neuron_getWeights", [subnet_uid, neuron_uid])
            if not result:
                return []
            return [(w["uid"], w["weight"]) for w in result]
        except Exception as e:
            logger.warning(f"Failed to get weights: {e}")
            return []
