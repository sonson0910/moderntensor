"""
Neuron Mixin for LuxtensorClient

Provides neuron query methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class NeuronMixin:
    """
    Mixin providing neuron query methods.

    Methods:
        - get_neuron() - Get single neuron info
        - get_neurons() - Get all neurons in subnet
        - get_neuron_count() - Count neurons in subnet
        - neuron_exists() - Check neuron existence
        - get_weights() - Get neuron weights
        - batch_get_neurons() - Batch query multiple neurons
        - get_active_neurons() - Get active neuron UIDs
        - get_neuron_axon() - Get axon information
        - get_neuron_prometheus() - Get prometheus endpoint
        - get_total_neurons() - Total neurons across all subnets
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

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
            return self._rpc()._call_rpc("neuron_get", [subnet_uid, neuron_uid])
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
            result = self._rpc()._call_rpc("neuron_getAll", [subnet_uid])
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
            result = self._rpc()._call_rpc("neuron_getCount", [subnet_uid])
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
            result = self._rpc()._call_rpc("neuron_exists", [subnet_uid, neuron_uid])
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
            result = self._rpc()._call_rpc("neuron_getWeights", [subnet_uid, neuron_uid])
            if not result:
                return []
            return [(w["uid"], w["weight"]) for w in result]
        except Exception as e:
            logger.warning(f"Failed to get weights: {e}")
            return []

    # Extended Neuron Methods

    def get_neurons_batch(
        self, subnet_id: int, neuron_uids: List[int]
    ) -> List[Optional[Dict[str, Any]]]:
        """
        Batch get multiple neurons.

        Args:
            subnet_id: Subnet identifier
            neuron_uids: List of neuron UIDs

        Returns:
            List of neuron data (None for failed fetches)
        """
        results = []
        for uid in neuron_uids:
            try:
                neuron = self.get_neuron(subnet_id, uid)
                results.append(neuron)
            except Exception as e:
                logger.warning(f"Error getting neuron {uid}: {e}")
                results.append(None)
        return results

    def get_active_neurons(self, subnet_id: int) -> List[int]:
        """
        Get list of active neuron UIDs in a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            List of active neuron UIDs
        """
        try:
            result = self._rpc()._call_rpc("query_activeNeurons", [subnet_id])
            return result if result else []
        except Exception as e:
            logger.error(f"Error getting active neurons in subnet {subnet_id}: {e}")
            return []

    def get_neuron_axon(self, subnet_id: int, neuron_uid: int) -> Dict[str, Any]:
        """
        Get axon information for a specific neuron.

        Args:
            subnet_id: Subnet identifier
            neuron_uid: Neuron unique identifier

        Returns:
            Axon information (ip, port, protocol)
        """
        try:
            result = self._rpc()._call_rpc("query_neuronAxon", [subnet_id, neuron_uid])
            return result if result else {}
        except Exception as e:
            logger.error(f"Error getting axon for neuron {neuron_uid}: {e}")
            return {}

    def get_neuron_prometheus(self, subnet_id: int, neuron_uid: int) -> Dict[str, Any]:
        """
        Get prometheus information for a specific neuron.

        Args:
            subnet_id: Subnet identifier
            neuron_uid: Neuron unique identifier

        Returns:
            Prometheus endpoint information
        """
        try:
            result = self._rpc()._call_rpc("query_neuronPrometheus", [subnet_id, neuron_uid])
            return result if result else {}
        except Exception as e:
            logger.error(f"Error getting prometheus for neuron {neuron_uid}: {e}")
            return {}

    def get_total_neurons(self) -> int:
        """
        Get total number of neurons across all subnets.

        Returns:
            Total neuron count
        """
        try:
            result = self._rpc()._call_rpc("query_totalNeurons")
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting total neurons: {e}")
            return 0

    def get_consensus(self, subnet_id: int, neuron_uid: int) -> float:
        """
        Get consensus weight for a neuron.

        Args:
            subnet_id: Subnet identifier
            neuron_uid: Neuron UID

        Returns:
            Consensus weight (0-1)
        """
        try:
            result = self._rpc()._call_rpc("query_consensus", [subnet_id, neuron_uid])
            return float(result) if result else 0.0
        except Exception as e:
            logger.error(f"Error getting consensus for neuron {neuron_uid}: {e}")
            return 0.0

    def get_emission(self, subnet_id: int, neuron_uid: int) -> float:
        """
        Get emission rate for a neuron.

        Args:
            subnet_id: Subnet identifier
            neuron_uid: Neuron UID

        Returns:
            Emission rate
        """
        try:
            result = self._rpc()._call_rpc("query_emission", [subnet_id, neuron_uid])
            return float(result) if result else 0.0
        except Exception as e:
            logger.error(f"Error getting emission for neuron {neuron_uid}: {e}")
            return 0.0

    # Neuron Scoring Metrics

    def get_dividends(self, subnet_id: int, neuron_uid: int) -> float:
        """Get dividends for a neuron."""
        try:
            result = self._rpc()._call_rpc("query_dividends", [subnet_id, neuron_uid])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting dividends for neuron {neuron_uid}: {e}")
            raise

    def get_incentive(self, subnet_id: int, neuron_uid: int) -> float:
        """Get incentive score for a neuron."""
        try:
            result = self._rpc()._call_rpc("query_incentive", [subnet_id, neuron_uid])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting incentive for neuron {neuron_uid}: {e}")
            raise

    def get_rank(self, subnet_id: int, neuron_uid: int) -> float:
        """Get rank score for a neuron."""
        try:
            result = self._rpc()._call_rpc("query_rank", [subnet_id, neuron_uid])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting rank for neuron {neuron_uid}: {e}")
            raise

    def get_trust(self, subnet_id: int, neuron_uid: int) -> float:
        """Get trust score for a neuron."""
        try:
            result = self._rpc()._call_rpc("query_trust", [subnet_id, neuron_uid])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting trust for neuron {neuron_uid}: {e}")
            raise

    def get_validator_trust(self, subnet_id: int, neuron_uid: int) -> float:
        """Get validator trust score for a neuron."""
        try:
            result = self._rpc()._call_rpc("query_validatorTrust", [subnet_id, neuron_uid])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting validator trust for neuron {neuron_uid}: {e}")
            raise
