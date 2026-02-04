"""
AI & Oracle Mixin for LuxtensorClient

Provides AI task submission and oracle methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class AIMixin:
    """
    Mixin providing AI task and oracle methods.

    Methods:
        - submit_ai_task() - Submit AI computation task
        - get_ai_result() - Get AI task result
        - claim_rewards() - Claim pending rewards
        - get_burn_stats() - Get token burn statistics
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def submit_ai_task(self, task_data: Dict[str, Any]) -> str:
        """
        Submit AI computation task to Luxtensor.

        Args:
            task_data: Task parameters (model_hash, input_data, requester, reward)

        Returns:
            Task ID
        """
        try:
            return self._rpc()._call_rpc("lux_submitAITask", [task_data])
        except Exception as e:
            logger.error(f"Failed to submit AI task: {e}")
            raise

    def get_ai_result(self, task_id: str) -> Optional[Dict[str, Any]]:
        """
        Get AI task result.

        Args:
            task_id: Task ID from submit_ai_task

        Returns:
            Task result if available, None otherwise
        """
        try:
            return self._rpc()._call_rpc("lux_getAIResult", [task_id])
        except Exception as e:
            logger.warning(f"Failed to get AI result for task {task_id}: {e}")
            return None

    def claim_rewards(self, address: str) -> Dict[str, Any]:
        """
        Claim pending rewards for an address.

        Args:
            address: Account address (0x...)

        Returns:
            Claim result with success, claimed amount, and new balance
        """
        try:
            result = self._rpc()._call_rpc("rewards_claim", [address])
            return result if result else {"success": False, "claimed": 0}
        except Exception as e:
            logger.error(f"Failed to claim rewards for {address}: {e}")
            return {"success": False, "error": str(e)}

    def get_burn_stats(self) -> Dict[str, Any]:
        """
        Get token burn statistics.

        Returns:
            Burn stats including total burned, tx fee burned, slashing burned, etc.
        """
        try:
            result = self._rpc()._call_rpc("rewards_getBurnStats", [])
            return result if result else {}
        except Exception as e:
            logger.warning(f"Failed to get burn stats: {e}")
            return {}
