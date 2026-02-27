"""
Dispute Resolution Mixin for LuxtensorClient

Wraps L1 dispute_* RPC methods for submitting fraud proofs
and querying dispute statuses.
"""

import logging
import time
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class DisputeMixin:
    """
    Mixin providing dispute resolution methods.

    Methods:
        - dispute_submit()     — Submit a fraud proof / dispute
        - dispute_get_status() — Get status of a dispute
        - dispute_get_stats()  — Get overall dispute statistics
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    def dispute_submit(
        self,
        disputer: str,
        target: str,
        evidence: str,
        dispute_type: str,
        signature: str,
        description: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Submit a fraud proof / dispute against a target address.

        Args:
            disputer: Disputer address (0x...)
            target: Target address being disputed (0x...)
            evidence: Hex-encoded evidence data
            dispute_type: Type of dispute (e.g. "invalid_weight", "gradient_fraud")
            signature: Secp256k1 signature over `dispute_submit:{target}:{timestamp}`
            description: Optional human-readable description

        Returns:
            Dispute result with dispute_id and initial status
        """
        try:
            timestamp = int(time.time())
            params: Dict[str, Any] = {
                "disputer": disputer,
                "target": target,
                "evidence": evidence,
                "dispute_type": dispute_type,
                "signature": signature,
                "timestamp": timestamp,
            }
            if description:
                params["description"] = description
            return self._rpc()._call_rpc("dispute_submit", [params]) or {}
        except Exception as e:
            logger.error("Failed to submit dispute against %s: %s", target, e)
            raise

    def dispute_get_status(self, dispute_id: str) -> Optional[Dict[str, Any]]:
        """
        Get the current status of a dispute.

        Args:
            dispute_id: Dispute ID returned by dispute_submit

        Returns:
            Dispute status dict or None if not found
        """
        try:
            return self._rpc()._call_rpc("dispute_getStatus", [dispute_id])
        except Exception as e:
            logger.warning("Failed to get dispute status %s: %s", dispute_id, e)
            return None

    def dispute_get_stats(self) -> Dict[str, Any]:
        """
        Get overall dispute statistics (total, resolved, pending).

        Returns:
            Stats dict
        """
        try:
            return self._rpc()._call_rpc("dispute_stats", []) or {}
        except Exception as e:
            logger.warning("Failed to get dispute stats: %s", e)
            return {}
