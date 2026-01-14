"""
Neuron Mixin for LuxtensorClient

Provides neuron and weight query methods.
"""

from typing import Dict, Any, List, Optional, TYPE_CHECKING
import logging

if TYPE_CHECKING:
    from .base import BaseClient

logger = logging.getLogger(__name__)


class NeuronMixin:
    """
    Mixin providing neuron and weight query methods.

    Methods:
        - get_neurons()
        - get_neuron()
        - get_weights()
        - get_subnet_info()
        - get_validators()
    """

    _call_rpc: callable

    def get_neurons(self, subnet_id: int) -> List[Dict[str, Any]]:
        """
        Get neurons (miners/validators) in subnet.

        Args:
            subnet_id: Subnet ID

        Returns:
            List of neuron information
        """
        try:
            result = self._call_rpc("neuron_listBySubnet", [subnet_id])
            return result if result else []
        except Exception as e:
            logger.error(f"Failed to get neurons for subnet {subnet_id}: {e}")
            return []

    def get_neuron(self, subnet_id: int, uid: int) -> Optional[Dict[str, Any]]:
        """
        Get single neuron by UID.

        Args:
            subnet_id: Subnet ID
            uid: Neuron UID

        Returns:
            Neuron info or None
        """
        try:
            result = self._call_rpc("neuron_get", [subnet_id, uid])
            return result
        except Exception as e:
            logger.error(f"Failed to get neuron {uid} in subnet {subnet_id}: {e}")
            return None

    def get_weights(self, subnet_id: int, neuron_uid: int) -> List[float]:
        """
        Get weight matrix for neuron.

        Args:
            subnet_id: Subnet ID
            neuron_uid: Neuron UID

        Returns:
            Weight values
        """
        try:
            result = self._call_rpc("weight_getWeights", [subnet_id, neuron_uid])
            return result if result else []
        except Exception as e:
            logger.error(f"Failed to get weights for neuron {neuron_uid}: {e}")
            return []

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
            return {}

    def get_validators(self) -> List[Dict[str, Any]]:
        """
        Get list of active validators.

        Returns:
            List of validator information
        """
        try:
            result = self._call_rpc("staking_getValidators", [])
            return result if result else []
        except Exception as e:
            logger.warning(f"Failed to get validators: {e}")
            return []

    def get_neuron_count(self, subnet_id: int) -> int:
        """
        Get total neuron count in subnet.

        Args:
            subnet_id: Subnet ID

        Returns:
            Number of neurons
        """
        neurons = self.get_neurons(subnet_id)
        return len(neurons)

    def get_weight_commits(self, subnet_id: int) -> List[Dict[str, Any]]:
        """
        Get weight commit hashes for subnet.

        Args:
            subnet_id: Subnet ID

        Returns:
            List of weight commits
        """
        try:
            result = self._call_rpc("weight_getCommits", [subnet_id])
            return result if result else []
        except Exception as e:
            logger.error(f"Failed to get weight commits: {e}")
            return []

    def submit_ai_task(self, task_data: Dict[str, Any]) -> str:
        """
        Submit AI computation task to Luxtensor.

        Args:
            task_data: Task parameters (model_hash, input_data, etc.)

        Returns:
            Task ID
        """
        return self._call_rpc("lux_submitAITask", [task_data])

    def get_ai_result(self, task_id: str) -> Optional[Dict[str, Any]]:
        """
        Get AI task result.

        Args:
            task_id: Task ID from submit_ai_task

        Returns:
            Task result if available
        """
        return self._call_rpc("lux_getAIResult", [task_id])
