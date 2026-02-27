"""
Metagraph Mixin for LuxtensorClient

Wraps metagraph_* and lux_* RPC methods for querying the full
metagraph state from MetagraphDB (RocksDB-backed persistent storage).

This mixin provides a high-level API for reading the complete network
state: subnets, neurons, weights, emissions, and validators together
in a single coherent view.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class MetagraphMixin:
    """
    Mixin providing full metagraph state access methods.

    All methods use the `metagraph_*` or `lux_*` namespace backed by MetagraphDB.

    Methods:
        - metagraph_get_state()          — Full metagraph state snapshot (metagraph_getState)
        - metagraph_get_weights()        — Weight matrix from metagraph (metagraph_getWeights)
        - get_metagraph_snapshot()       — Alias for metagraph_get_state with caching
        - get_emissions()                — Emission info for a subnet (lux_getEmissions)
        - get_network_state()            — Combined subnet + neuron state
        - get_validator_info()           — Validator data for a subnet
        - is_validator()                 — Check if a hot-key is a validator
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    # ------------------------------------------------------------------
    # Metagraph State Methods
    # ------------------------------------------------------------------

    def metagraph_get_state(self, subnet_id: int) -> Dict[str, Any]:
        """
        Get the full metagraph state for a subnet.

        Returns a comprehensive snapshot including:
        - subnet: SubnetData (id, name, owner, emission_rate, tempo, etc.)
        - neurons: List of NeuronData entries
        - weights: Full weight matrix as list of WeightData
        - neuron_count: total number of neurons
        - weight_count: total number of weight entries

        Args:
            subnet_id: Subnet identifier

        Returns:
            Full metagraph state dict
        """
        try:
            result = self._rpc()._call_rpc("metagraph_getState", [subnet_id])
            return result if isinstance(result, dict) else {}
        except Exception as e:
            logger.error("Failed to get metagraph state for subnet %s: %s", subnet_id, e)
            return {}

    def metagraph_get_weights(self, subnet_id: int) -> List[Dict[str, Any]]:
        """
        Get weight matrix from metagraph namespace.

        Args:
            subnet_id: Subnet identifier

        Returns:
            List of WeightData dicts {from_uid, to_uid, weight, block_set}
        """
        try:
            result = self._rpc()._call_rpc("metagraph_getWeights", [subnet_id])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning("Failed to get metagraph weights for subnet %s: %s", subnet_id, e)
            return []

    def get_metagraph_snapshot(self, subnet_id: int) -> Dict[str, Any]:
        """
        Get full metagraph snapshot (alias for metagraph_get_state).

        Includes subnet, neurons list, weights matrix, counts.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Full snapshot dict
        """
        return self.metagraph_get_state(subnet_id)

    # ------------------------------------------------------------------
    # Emission Methods
    # ------------------------------------------------------------------

    def get_emissions(self, subnet_id: int) -> Dict[str, Any]:
        """
        Get emission information for a subnet.

        Returns:
            Dict with subnet_id, emission_rate, emission_rate_decimal, name
        """
        try:
            result = self._rpc()._call_rpc("lux_getEmissions", [subnet_id])
            return result if isinstance(result, dict) else {}
        except Exception as e:
            logger.warning("Failed to get emissions for subnet %s: %s", subnet_id, e)
            return {}

    def get_emissions_for_all_subnets(self) -> List[Dict[str, Any]]:
        """
        Get emission data for all subnets.

        Returns:
            List of emission dicts, one per subnet
        """
        try:
            subnets = self._rpc()._call_rpc("lux_listSubnets", [])
            if not subnets:
                return []
            results = []
            for subnet in subnets:
                sid = subnet.get("id")
                if sid is not None:
                    emission = self._rpc()._call_rpc("lux_getEmissions", [sid])
                    if emission:
                        results.append(emission)
            return results
        except Exception as e:
            logger.error("Failed to get emissions for all subnets: %s", e)
            return []

    # ------------------------------------------------------------------
    # Network State
    # ------------------------------------------------------------------

    def get_network_state(self) -> Dict[str, Any]:
        """
        Get the combined network state: all subnets with their neuron counts.

        Returns:
            Dict with subnets list and summary stats
        """
        try:
            subnets = self._rpc()._call_rpc("lux_listSubnets", [])
            if not subnets:
                return {"subnets": [], "total_subnets": 0, "total_neurons": 0}

            total_neurons = 0
            subnet_states = []
            for subnet in subnets:
                sid = subnet.get("id")
                if sid is not None:
                    count_result = self._rpc()._call_rpc("lux_getNeuronCount", [sid])
                    if isinstance(count_result, dict):
                        count = int(count_result.get("count", 0))
                    elif count_result:
                        count = int(count_result)
                    else:
                        count = 0
                    total_neurons += count
                    subnet_states.append({**subnet, "neuron_count": count})

            return {
                "subnets": subnet_states,
                "total_subnets": len(subnet_states),
                "total_neurons": total_neurons,
            }
        except Exception as e:
            logger.error("Failed to get network state: %s", e)
            return {"subnets": [], "total_subnets": 0, "total_neurons": 0}

    # ------------------------------------------------------------------
    # Validator Methods
    # ------------------------------------------------------------------

    def get_validator_info(self, subnet_id: int) -> List[Dict[str, Any]]:
        """
        Get validator neurons for a subnet (neurons with active validator=true).

        Args:
            subnet_id: Subnet identifier

        Returns:
            List of validator NeuronData dicts
        """
        try:
            all_neurons = self._rpc()._call_rpc("lux_getNeurons", [subnet_id])
            if not all_neurons:
                return []
            return [n for n in all_neurons if n.get("is_validator", False)]
        except Exception as e:
            logger.warning("Failed to get validator info for subnet %s: %s", subnet_id, e)
            return []

    def is_validator(self, subnet_id: int, hotkey: str) -> bool:
        """
        Check if a hotkey address is a registered validator in a subnet.

        Args:
            subnet_id: Subnet identifier
            hotkey: Validator hot-key address (0x...)

        Returns:
            True if the hotkey belongs to a validator neuron
        """
        try:
            validators = self.get_validator_info(subnet_id)
            return any(v.get("hotkey", "").lower() == hotkey.lower() for v in validators)
        except Exception as e:
            logger.warning("Failed to check validator status for %s: %s", hotkey, e)
            return False

    def get_system_node_roles(self) -> Dict[str, Any]:
        """
        Get system-wide node role assignments from the running node.

        Returns:
            Dict with roles list and node address
        """
        try:
            result = self._rpc()._call_rpc("system_nodeRoles", [])
            return result if isinstance(result, dict) else {}
        except Exception as e:
            logger.warning("Failed to get system node roles: %s", e)
            return {}
