"""
Transaction Mixin for LuxtensorClient

Provides transaction submission and query methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class TransactionMixin:
    """
    Mixin providing transaction methods.

    Methods:
        - submit_transaction()
        - get_transaction()
        - get_transaction_receipt()
        - wait_for_transaction()
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def submit_transaction(self, signed_tx: str):
        """
        Submit signed transaction to Luxtensor.

        Args:
            signed_tx: Signed transaction (hex encoded, with 0x prefix)

        Returns:
            TransactionResult with tx_hash and status
        """
        from .base import TransactionResult

        try:
            tx_hash = self._rpc()._call_rpc("eth_sendRawTransaction", [signed_tx])
            return TransactionResult(tx_hash=tx_hash, status="pending", block_number=None, error=None)
        except Exception as e:
            logger.error(f"Failed to submit transaction: {e}")
            return TransactionResult(tx_hash="", status="failed", block_number=None, error=str(e))

    def get_transaction(self, tx_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get transaction by hash.

        Args:
            tx_hash: Transaction hash (with 0x prefix)

        Returns:
            Transaction data or None if not found
        """
        return self._rpc()._call_rpc("eth_getTransactionByHash", [tx_hash])

    def get_transaction_receipt(self, tx_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get transaction receipt.

        Args:
            tx_hash: Transaction hash

        Returns:
            Transaction receipt or None if not mined
        """
        return self._rpc()._call_rpc("eth_getTransactionReceipt", [tx_hash])

    def wait_for_transaction(
        self, tx_hash: str, timeout: int = 60, poll_interval: float = 1.0
    ) -> Optional[Dict[str, Any]]:
        """
        Wait for transaction to be mined.

        Args:
            tx_hash: Transaction hash
            timeout: Maximum wait time in seconds
            poll_interval: Poll interval in seconds

        Returns:
            Transaction receipt or None if timed out
        """
        import time

        start = time.time()
        while time.time() - start < timeout:
            receipt = self.get_transaction_receipt(tx_hash)
            if receipt:
                return receipt
            time.sleep(poll_interval)

        logger.warning(f"Transaction {tx_hash} not mined after {timeout}s")
        return None
