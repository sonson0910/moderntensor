"""
Neuron Client
Handles neuron registration, queries, and weight management.
"""

import logging
from typing import Optional, Dict, Any, List
from .base import BaseRpcClient

logger = logging.getLogger(__name__)


class NeuronClient(BaseRpcClient):
    """
    Client for neuron operations.
    Single Responsibility: Neuron management only.
    """

    # ========================================================================
    # Neuron Queries
    # ========================================================================

    def get_neuron(self, subnet_id: int, neuron_uid: int) -> Optional[Dict[str, Any]]:
        """
        Get specific neuron by subnet and UID.

        Args:
            subnet_id: Subnet ID
            neuron_uid: Neuron UID

        Returns:
            Neuron data or None
        """
        try:
            return self._call_rpc("neuron_get", [subnet_id, neuron_uid])
        except Exception as e:
            logger.warning("Failed to get neuron %s in subnet %s: %s", neuron_uid, subnet_id, e)
            return None

    def get_neurons(self, subnet_id: int) -> List[Dict[str, Any]]:
        """
        Get all neurons in subnet.

        Args:
            subnet_id: Subnet ID

        Returns:
            List of neuron data
        """
        try:
            result = self._call_rpc("neuron_listBySubnet", [subnet_id])
            return result if result else []
        except Exception as e:
            logger.error("Failed to get neurons for subnet %s: %s", subnet_id, e)
            return []

    def get_neuron_for_hotkey(self, subnet_id: int, hotkey: str) -> Optional[Dict[str, Any]]:
        """
        Get neuron by hotkey.

        Args:
            subnet_id: Subnet ID
            hotkey: Hotkey address

        Returns:
            Neuron data or None
        """
        try:
            return self._call_rpc("neuron_getByHotkey", [subnet_id, hotkey])
        except Exception as e:
            logger.warning("Failed to get neuron for hotkey: %s", e)
            return None

    def is_hotkey_registered(self, subnet_id: int, hotkey: str) -> bool:
        """
        Check if hotkey is registered in subnet.

        Args:
            subnet_id: Subnet ID
            hotkey: Hotkey address

        Returns:
            True if registered
        """
        neuron = self.get_neuron_for_hotkey(subnet_id, hotkey)
        return neuron is not None

    def get_active_neurons(self, subnet_id: int) -> List[int]:
        """
        Get UIDs of active neurons.

        Args:
            subnet_id: Subnet ID

        Returns:
            List of active neuron UIDs
        """
        try:
            result = self._call_rpc("neuron_getActive", [subnet_id])
            return result if result else []
        except Exception as e:
            logger.warning("Failed to get active neurons: %s", e)
            return []

    def get_total_neurons(self, subnet_id: Optional[int] = None) -> int:
        """
        Get total neuron count.

        Args:
            subnet_id: Subnet ID (None for all subnets)

        Returns:
            Number of neurons
        """
        try:
            params = [subnet_id] if subnet_id is not None else []
            result = self._call_rpc("neuron_count", params)
            return result if result else 0
        except Exception as e:
            logger.warning("Failed to get neuron count: %s", e)
            return 0

    # ========================================================================
    # Neuron Registration
    # ========================================================================

    def register_neuron(
        self,
        subnet_id: int,
        hotkey: str,
        coldkey: str,
        ip: str = "",
        port: int = 0,
    ) -> Dict[str, Any]:
        """
        Register a neuron in subnet.

        Args:
            subnet_id: Target subnet
            hotkey: Hotkey address
            coldkey: Coldkey address
            ip: Optional axon IP
            port: Optional axon port

        Returns:
            Registration result
        """
        try:
            result = self._call_rpc(
                "neuron_register",
                [subnet_id, hotkey, coldkey, ip, port]
            )
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error("Failed to register neuron: %s", e)
            return {"success": False, "error": str(e)}

    # ========================================================================
    # Weight Management
    # ========================================================================

    def get_weights(self, subnet_id: int, neuron_uid: int) -> List[float]:
        """
        Get weights for neuron.

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
            logger.error("Failed to get weights: %s", e)
            return []

    def set_weights(
        self,
        subnet_id: int,
        hotkey: str,
        uids: List[int],
        weights: List[float],
    ) -> Dict[str, Any]:
        """
        Set weights for neurons.

        Args:
            subnet_id: Subnet ID
            hotkey: Validator hotkey
            uids: Target neuron UIDs
            weights: Weight values

        Returns:
            Result
        """
        try:
            result = self._call_rpc(
                "weight_setWeights",
                [subnet_id, hotkey, uids, weights]
            )
            return result if result else {"success": False, "error": "No result"}
        except Exception as e:
            logger.error("Failed to set weights: %s", e)
            return {"success": False, "error": str(e)}

    def get_weight_commits(self, subnet_id: int) -> List[Dict[str, Any]]:
        """
        Get weight commits for subnet.

        Args:
            subnet_id: Subnet ID

        Returns:
            List of weight commits
        """
        try:
            result = self._call_rpc("weight_getCommits", [subnet_id])
            return result if result else []
        except Exception as e:
            logger.warning("Failed to get weight commits: %s", e)
            return []

    # ========================================================================
    # Performance Metrics
    # ========================================================================

    def get_rank(self, subnet_id: int, neuron_uid: int) -> float:
        """Get neuron rank"""
        return self._get_metric(subnet_id, neuron_uid, "rank")

    def get_trust(self, subnet_id: int, neuron_uid: int) -> float:
        """Get neuron trust"""
        return self._get_metric(subnet_id, neuron_uid, "trust")

    def get_consensus(self, subnet_id: int, neuron_uid: int) -> float:
        """Get neuron consensus"""
        return self._get_metric(subnet_id, neuron_uid, "consensus")

    def get_incentive(self, subnet_id: int, neuron_uid: int) -> float:
        """Get neuron incentive"""
        return self._get_metric(subnet_id, neuron_uid, "incentive")

    def get_dividends(self, subnet_id: int, neuron_uid: int) -> float:
        """Get neuron dividends"""
        return self._get_metric(subnet_id, neuron_uid, "dividends")

    def get_emission(self, subnet_id: int, neuron_uid: int) -> float:
        """Get neuron emission.

        Note: There is no query_emission RPC. Emission is fetched from
        the full neuron data via neuron_get.
        """
        try:
            neuron = self._call_rpc("neuron_get", [subnet_id, neuron_uid])
            if neuron and isinstance(neuron, dict):
                return float(neuron.get("emission", 0.0))
            return 0.0
        except Exception as e:
            logger.warning("Failed to get emission: %s", e)
            return 0.0

    def _get_metric(self, subnet_id: int, neuron_uid: int, metric: str) -> float:
        """Helper to get single neuron metric via query_* RPC.

        Valid metrics: rank, trust, consensus, incentive, dividends.
        For emission, use get_emission() directly.
        """
        try:
            result = self._call_rpc(f"query_{metric}", [subnet_id, neuron_uid])
            return float(result) if result else 0.0
        except Exception as e:
            logger.warning("Failed to get %s: %s", metric, e)
            return 0.0

    # ========================================================================
    # Batch Operations
    # ========================================================================

    def batch_get_neurons(self, subnet_id: int, neuron_uids: List[int]) -> List[Dict[str, Any]]:
        """
        Batch get neurons by UIDs.

        Args:
            subnet_id: Subnet ID
            neuron_uids: List of UIDs

        Returns:
            List of neuron data
        """
        try:
            result = self._call_rpc("neuron_batchGet", [subnet_id, neuron_uids])
            return result if result else []
        except Exception as e:
            logger.warning("Failed to batch get neurons: %s", e)
            # Fallback to individual queries
            neurons = []
            for uid in neuron_uids:
                neuron = self.get_neuron(subnet_id, uid)
                if neuron:
                    neurons.append(neuron)
            return neurons
