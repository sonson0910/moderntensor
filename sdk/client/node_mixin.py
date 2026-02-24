"""
Node Tier Management Mixin for LuxtensorClient

Wraps L1 node_* RPC methods for progressive staking node registration,
tier queries, and network info.
"""

import logging
import time
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class NodeMixin:
    """
    Mixin providing node tier management methods.

    Methods:
        Registration:
            - node_register()                — Register a node with stake (signed)
            - node_update_stake()            — Update a node's stake (signed)
            - node_unregister()              — Unregister a node (signed)

        Queries:
            - node_get_tier()               — Get a node's tier info
            - node_get_info()               — Get detailed node info
            - node_get_validators()          — List all validator-tier nodes
            - node_get_infrastructure_nodes() — List infrastructure nodes
            - node_get_stats()              — Get network-wide node stats
            - node_get_tier_requirements()  — Get tier stake requirements
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    # ---------------------------------------------------------------
    # Registration
    # ---------------------------------------------------------------

    def node_register(
        self,
        address: str,
        stake: str,
        signature: str,
        block_height: Optional[int] = None,
    ) -> Dict[str, Any]:
        """
        Register a node with its stake amount.

        The node's tier (LightNode / FullNode / Validator / SuperValidator)
        is determined automatically by the stake level.

        Args:
            address: Node address (0x...)
            stake: Stake amount (u128 as decimal or hex string)
            signature: Secp256k1 signature over `node_register:{address}:{stake}:{timestamp}`
            block_height: Optional current block height

        Returns:
            Dict with success, address, tier, tier_name, can_produce_blocks
        """
        try:
            timestamp = int(time.time())
            params: Dict[str, Any] = {
                "address": address,
                "stake": stake,
                "signature": signature,
                "timestamp": timestamp,
            }
            if block_height is not None:
                params["block_height"] = block_height
            return self._rpc()._call_rpc("node_register", [params]) or {}
        except Exception as e:
            logger.error(f"Failed to register node {address}: {e}")
            raise

    def node_update_stake(
        self,
        address: str,
        new_stake: str,
        signature: str,
    ) -> Dict[str, Any]:
        """
        Update the stake for an already-registered node.

        The tier may change automatically based on the new stake.

        Args:
            address: Node address (0x...)
            new_stake: New stake amount (u128 as decimal or hex string)
            signature: Secp256k1 signature over `node_updateStake:{address}:{new_stake}:{timestamp}`

        Returns:
            Dict with success, address, new_stake, new_tier, tier_name
        """
        try:
            timestamp = int(time.time())
            params: Dict[str, Any] = {
                "address": address,
                "new_stake": new_stake,
                "signature": signature,
                "timestamp": timestamp,
            }
            return self._rpc()._call_rpc("node_updateStake", [params]) or {}
        except Exception as e:
            logger.error(f"Failed to update stake for node {address}: {e}")
            return {"success": False, "error": str(e)}

    def node_unregister(
        self,
        address: str,
        signature: str,
    ) -> Dict[str, Any]:
        """
        Unregister a node and return its staked tokens.

        Args:
            address: Node address (0x...)
            signature: Secp256k1 signature over `node_unregister:{address}`

        Returns:
            Dict with success, address, stake_returned
        """
        try:
            timestamp = int(time.time())
            params: Dict[str, Any] = {
                "address": address,
                "signature": signature,
                "timestamp": str(timestamp),
            }
            return self._rpc()._call_rpc("node_unregister", [params]) or {}
        except Exception as e:
            logger.error(f"Failed to unregister node {address}: {e}")
            return {"success": False, "error": str(e)}

    # ---------------------------------------------------------------
    # Queries
    # ---------------------------------------------------------------

    def node_get_tier(self, address: str) -> Optional[Dict[str, Any]]:
        """
        Get the tier classification of a node.

        Args:
            address: Node address (0x...)

        Returns:
            Dict with tier, tier_name, emission_share, can_produce_blocks
            or None if not found
        """
        try:
            return self._rpc()._call_rpc("node_getTier", [{"address": address}])
        except Exception as e:
            logger.warning(f"Failed to get tier for node {address}: {e}")
            return None

    def node_get_info(self, address: str) -> Optional[Dict[str, Any]]:
        """
        Get detailed information about a registered node.

        Args:
            address: Node address (0x...)

        Returns:
            Dict with stake, tier, registered_at, last_active,
            uptime_score, blocks_produced, tx_relayed — or None if not found
        """
        try:
            return self._rpc()._call_rpc("node_getInfo", [{"address": address}])
        except Exception as e:
            logger.warning(f"Failed to get node info for {address}: {e}")
            return None

    def node_get_validators(self) -> List[Dict[str, Any]]:
        """
        List all nodes that are in the Validator or SuperValidator tier.

        Returns:
            List of validator info dicts (address, tier, stake, uptime_score, blocks_produced)
        """
        try:
            result = self._rpc()._call_rpc("node_getValidators", [])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning(f"Failed to get validators: {e}")
            return []

    def node_get_infrastructure_nodes(self) -> List[Dict[str, Any]]:
        """
        List all infrastructure nodes (FullNode and above).

        Returns:
            List of node info dicts (address, tier, stake)
        """
        try:
            result = self._rpc()._call_rpc("node_getInfrastructureNodes", [])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning(f"Failed to get infrastructure nodes: {e}")
            return []

    def node_get_stats(self) -> Dict[str, Any]:
        """
        Get network-wide node statistics.

        Returns:
            Dict with total_nodes, total_stake, and per-tier counts
        """
        try:
            return self._rpc()._call_rpc("node_getStats", []) or {}
        except Exception as e:
            logger.warning(f"Failed to get node stats: {e}")
            return {}

    def node_get_tier_requirements(self) -> Dict[str, Any]:
        """
        Get the stake requirements for each node tier.

        Returns:
            Dict with a `tiers` list (tier name, min_stake, emission_share, can_produce_blocks)
        """
        try:
            return self._rpc()._call_rpc("node_getTierRequirements", []) or {}
        except Exception as e:
            logger.warning(f"Failed to get tier requirements: {e}")
            return {}
