"""
Neuron Mixin for LuxtensorClient

Wraps lux_* RPC methods for querying neuron/miner/validator data
stored in MetagraphDB (RocksDB-backed persistent storage).
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class NeuronMixin:
    """
    Mixin providing neuron query methods.

    All query methods use the `lux_*` RPC namespace backed by MetagraphDB.

    Methods:
        - get_neuron()                  — Single neuron info (lux_getNeuron)
        - get_neurons()                 — All neurons in subnet (lux_getNeurons)
        - get_neuron_count()            — Neuron count for subnet (lux_getNeuronCount)
        - neuron_exists()               — Check if neuron registered
        - get_neuron_weights()          — Weights set by a neuron (lux_getWeights)
        - get_neurons_batch()           — Multiple neurons by uid list
        - get_active_neurons()          — Neurons with active=true
        - get_axon_info()               — Axon endpoint info
        - get_prometheus_info()         — Prometheus endpoint info
        - get_total_neurons()           — Total neurons across all subnets
        - get_neuron_consensus()        — Consensus score
        - get_neuron_emissions()        — Emission value
        - get_neuron_rewards()          — Reward value
        - get_neuron_rank()             — Rank position
        - get_neuron_trust()            — Trust score
        - get_neuron_stake()            — Staked amount
        - get_all_neuron_weights()      — Full weight matrix (lux_getAllWeights)
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    # ------------------------------------------------------------------
    # Primary Methods
    # ------------------------------------------------------------------

    def get_neuron(self, subnet_uid: int, uid: int) -> Optional[Dict[str, Any]]:
        """
        Get a single neuron's information from MetagraphDB.

        Args:
            subnet_uid: Subnet identifier
            uid: Neuron UID within the subnet

        Returns:
            Neuron dict or None if not found
        """
        try:
            return self._rpc()._call_rpc("lux_getNeuron", [subnet_uid, uid])
        except Exception as e:
            logger.warning("Failed to get neuron %s in subnet %s: %s", uid, subnet_uid, e)
            return None

    def get_neurons(self, subnet_uid: int) -> List[Dict[str, Any]]:
        """
        Get all neurons registered in a subnet from MetagraphDB.

        Args:
            subnet_uid: Subnet identifier

        Returns:
            List of NeuronData dicts
        """
        try:
            result = self._rpc()._call_rpc("lux_getNeurons", [subnet_uid])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning("Failed to get neurons for subnet %s: %s", subnet_uid, e)
            return []

    def get_neuron_count(self, subnet_uid: int) -> int:
        """
        Get number of neurons registered in a subnet.

        Args:
            subnet_uid: Subnet identifier

        Returns:
            Neuron count
        """
        try:
            result = self._rpc()._call_rpc("lux_getNeuronCount", [subnet_uid])
            if isinstance(result, dict):
                return int(result.get("count", 0))
            return int(result) if result else 0
        except Exception as e:
            logger.warning("Failed to get neuron count for subnet %s: %s", subnet_uid, e)
            return 0

    def neuron_exists(self, subnet_uid: int, uid: int) -> bool:
        """
        Check if a neuron with the given UID is registered in the subnet.

        Args:
            subnet_uid: Subnet identifier
            uid: Neuron UID

        Returns:
            True if found in MetagraphDB
        """
        try:
            result = self._rpc()._call_rpc("lux_getNeuron", [subnet_uid, uid])
            return result is not None
        except Exception:
            return False

    def get_neuron_weights(self, subnet_uid: int, uid: int) -> List[Dict[str, Any]]:
        """
        Get the weights that a neuron has set (its outgoing weight assignments).

        Args:
            subnet_uid: Subnet identifier
            uid: Source neuron UID

        Returns:
            List of WeightData dicts {from_uid, to_uid, weight, block_set}
        """
        try:
            result = self._rpc()._call_rpc("lux_getWeights", [subnet_uid, uid])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning("Failed to get weights for neuron %s in subnet %s: %s", uid, subnet_uid, e)
            return []

    def get_all_neuron_weights(self, subnet_uid: int) -> List[Dict[str, Any]]:
        """
        Get the full weight matrix for a subnet (all weight entries).

        Args:
            subnet_uid: Subnet identifier

        Returns:
            List of all WeightData entries {from_uid, to_uid, weight, block_set}
        """
        try:
            result = self._rpc()._call_rpc("lux_getAllWeights", [subnet_uid])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning("Failed to get all weights for subnet %s: %s", subnet_uid, e)
            return []

    # ------------------------------------------------------------------
    # Batch / Filtering Methods
    # ------------------------------------------------------------------

    def get_neurons_batch(self, subnet_uid: int, uids: List[int]) -> List[Dict[str, Any]]:
        """
        Get multiple neurons by UID list (client-side filtering from full list).

        Args:
            subnet_uid: Subnet identifier
            uids: List of UIDs to retrieve

        Returns:
            List of matching NeuronData dicts
        """
        try:
            all_neurons = self._rpc()._call_rpc("lux_getNeurons", [subnet_uid])
            if not all_neurons:
                return []
            uid_set = set(uids)
            return [n for n in all_neurons if n.get("uid") in uid_set]
        except Exception as e:
            logger.warning("Failed to get neurons batch for subnet %s: %s", subnet_uid, e)
            return []

    def get_active_neurons(self, subnet_uid: int) -> List[Dict[str, Any]]:
        """
        Get all neurons with active=true in a subnet.

        Args:
            subnet_uid: Subnet identifier

        Returns:
            List of active NeuronData dicts
        """
        try:
            all_neurons = self._rpc()._call_rpc("lux_getNeurons", [subnet_uid])
            if not all_neurons:
                return []
            return [n for n in all_neurons if n.get("active", False)]
        except Exception as e:
            logger.warning("Failed to get active neurons for subnet %s: %s", subnet_uid, e)
            return []

    # ------------------------------------------------------------------
    # Endpoint / Score Accessors (derived from NeuronData fields)
    # ------------------------------------------------------------------

    def get_axon_info(self, subnet_uid: int, uid: int) -> Optional[Dict[str, Any]]:
        """
        Get axon endpoint info for a neuron.

        Returns:
            Dict with ip, port, protocol, wallet fields or None
        """
        try:
            neuron = self._rpc()._call_rpc("lux_getNeuron", [subnet_uid, uid])
            if neuron:
                endpoint = neuron.get("endpoint") or ""
                ip, port, protocol = "", 0, ""
                if endpoint:
                    # Parse endpoint like "http://1.2.3.4:8080" or "1.2.3.4:8080"
                    from urllib.parse import urlparse
                    parsed = urlparse(endpoint if "://" in endpoint else f"tcp://{endpoint}")
                    ip = parsed.hostname or ""
                    port = parsed.port or 0
                    protocol = parsed.scheme if parsed.scheme != "tcp" else "http"
                return {
                    "ip": ip,
                    "port": port,
                    "protocol": protocol,
                    "wallet": neuron.get("hotkey", ""),
                    "endpoint": endpoint,
                }
            return None
        except Exception as e:
            logger.warning("Failed to get axon info for neuron %s: %s", uid, e)
            return None

    def get_prometheus_info(self, subnet_uid: int, uid: int) -> Optional[Dict[str, Any]]:
        """
        Get prometheus endpoint info for a neuron (stored in neuron data).

        Returns:
            Dict with ip, port fields or None
        """
        try:
            neuron = self._rpc()._call_rpc("lux_getNeuron", [subnet_uid, uid])
            if neuron:
                # Prometheus fields are stored alongside neuron data
                return {
                    "ip": neuron.get("prometheus_ip", neuron.get("ip", "")),
                    "port": neuron.get("prometheus_port", 0),
                }
            return None
        except Exception as e:
            logger.warning("Failed to get prometheus info for neuron %s: %s", uid, e)
            return None

    def get_total_neurons(self) -> int:
        """
        Get total neuron count across all subnets.

        Streams all subnets and sums neuron counts.
        """
        try:
            subnets = self._rpc()._call_rpc("lux_listSubnets", [])
            if not subnets:
                return 0
            total = 0
            for subnet in subnets:
                sid = subnet.get("id")
                if sid is not None:
                    result = self._rpc()._call_rpc("lux_getNeuronCount", [sid])
                    if isinstance(result, dict):
                        total += int(result.get("count", 0))
                    elif result:
                        total += int(result)
            return total
        except Exception as e:
            logger.warning("Failed to get total neurons: %s", e)
            return 0

    # ------------------------------------------------------------------
    # Score Accessors
    # ------------------------------------------------------------------

    def _get_neuron_field(self, subnet_uid: int, uid: int, field: str, default=0):
        """Helper to retrieve a specific field from NeuronData."""
        try:
            neuron = self._rpc()._call_rpc("lux_getNeuron", [subnet_uid, uid])
            if neuron:
                raw = neuron.get(field, default)
                return float(raw) if raw is not None else float(default)
            return float(default)
        except Exception:
            return float(default)

    def get_neuron_consensus(self, subnet_uid: int, uid: int) -> float:
        """Get consensus score for a neuron."""
        return self._get_neuron_field(subnet_uid, uid, "consensus")

    def get_neuron_emissions(self, subnet_uid: int, uid: int) -> float:
        """Get emission value assigned to a neuron."""
        return self._get_neuron_field(subnet_uid, uid, "emission")

    def get_neuron_rewards(self, subnet_uid: int, uid: int) -> float:
        """Get reward value for a neuron."""
        return self._get_neuron_field(subnet_uid, uid, "reward")

    def get_neuron_rank(self, subnet_uid: int, uid: int) -> float:
        """Get rank position for a neuron."""
        return self._get_neuron_field(subnet_uid, uid, "rank")

    def get_neuron_trust(self, subnet_uid: int, uid: int) -> float:
        """Get trust score for a neuron."""
        return self._get_neuron_field(subnet_uid, uid, "trust")

    def get_neuron_stake(self, subnet_uid: int, uid: int) -> str:
        """
        Get stake amount for a neuron.

        Returns:
            Stake as decimal string (u128)
        """
        try:
            neuron = self._rpc()._call_rpc("lux_getNeuron", [subnet_uid, uid])
            if neuron:
                return str(neuron.get("stake_decimal", neuron.get("stake", "0")))
            return "0"
        except Exception as e:
            logger.warning("Failed to get stake for neuron %s: %s", uid, e)
            return "0"
