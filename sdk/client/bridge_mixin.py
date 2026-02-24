"""
Cross-Chain Bridge Mixin for LuxtensorClient

Wraps L1 bridge_* RPC methods for querying cross-chain bridge
configuration and message status.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class BridgeMixin:
    """
    Mixin providing cross-chain bridge query methods.

    Methods:
        - bridge_get_config()    — Get bridge configuration
        - bridge_get_message()   — Get a specific bridge message status
        - bridge_list_messages() — List bridge messages with optional filters
        - bridge_get_stats()     — Get bridge statistics
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    def bridge_get_config(self) -> Dict[str, Any]:
        """
        Get the cross-chain bridge configuration.

        Returns:
            Bridge config dict (supported chains, fees, limits, etc.)
        """
        try:
            return self._rpc()._call_rpc("bridge_getConfig", []) or {}
        except Exception as e:
            logger.warning(f"Failed to get bridge config: {e}")
            return {}

    def bridge_get_message(self, message_id: str) -> Optional[Dict[str, Any]]:
        """
        Get the status and details of a bridge message.

        Args:
            message_id: Bridge message ID

        Returns:
            Message details dict or None if not found
        """
        try:
            return self._rpc()._call_rpc("bridge_getMessage", [message_id])
        except Exception as e:
            logger.warning(f"Failed to get bridge message {message_id}: {e}")
            return None

    def bridge_list_messages(
        self,
        sender: Optional[str] = None,
        status: Optional[str] = None,
        limit: int = 50,
    ) -> List[Dict[str, Any]]:
        """
        List bridge messages with optional filtering.

        Args:
            sender: Optional sender address to filter by (0x...)
            status: Optional status filter ("pending", "completed", "failed")
            limit: Maximum number of messages to return

        Returns:
            List of bridge message dicts
        """
        try:
            params: Dict[str, Any] = {"limit": limit}
            if sender:
                params["sender"] = sender
            if status:
                params["status"] = status
            result = self._rpc()._call_rpc("bridge_listMessages", [params])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning(f"Failed to list bridge messages: {e}")
            return []

    def bridge_get_stats(self) -> Dict[str, Any]:
        """
        Get bridge statistics (total messages, volume, fees collected).

        Returns:
            Bridge stats dict
        """
        try:
            return self._rpc()._call_rpc("bridge_getStats", []) or {}
        except Exception as e:
            logger.warning(f"Failed to get bridge stats: {e}")
            return {}
