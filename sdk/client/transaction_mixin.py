"""
Transaction Mixin for LuxtensorClient

Provides transaction submission and query methods.
"""

from typing import Optional, Dict, Any, List, TYPE_CHECKING
from .base import TransactionResult

if TYPE_CHECKING:
    from .base import BaseClient


class TransactionMixin:
    """
    Mixin providing transaction methods.

    Methods:
        - submit_transaction()
        - get_transaction()
        - get_transaction_receipt()
        - wait_for_transaction()
    """

    _call_rpc: callable

    def submit_transaction(self, signed_tx: str) -> TransactionResult:
        """
        Submit signed transaction to Luxtensor.

        Args:
            signed_tx: Signed transaction (hex encoded, 0x prefix)

        Returns:
            TransactionResult with tx_hash and status
        """
        tx_hash = self._call_rpc("eth_sendRawTransaction", [signed_tx])
        return TransactionResult(
            tx_hash=tx_hash,
            status="pending",
            block_number=None,
            error=None
        )

    def get_transaction(self, tx_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get transaction by hash.

        Args:
            tx_hash: Transaction hash

        Returns:
            Transaction data or None if not found
        """
        return self._call_rpc("eth_getTransactionByHash", [tx_hash])

    def get_transaction_receipt(self, tx_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get transaction receipt.

        Args:
            tx_hash: Transaction hash

        Returns:
            Receipt data or None if not yet mined
        """
        return self._call_rpc("eth_getTransactionReceipt", [tx_hash])

    def wait_for_transaction(
        self,
        tx_hash: str,
        timeout: int = 60,
        poll_interval: float = 1.0
    ) -> Optional[Dict[str, Any]]:
        """
        Wait for transaction to be mined.

        Args:
            tx_hash: Transaction hash
            timeout: Max seconds to wait
            poll_interval: Seconds between polls

        Returns:
            Transaction receipt or None if timeout
        """
        import time
        start = time.time()

        while time.time() - start < timeout:
            receipt = self.get_transaction_receipt(tx_hash)
            if receipt is not None:
                return receipt
            time.sleep(poll_interval)

        return None

    def estimate_gas(self, tx: Dict[str, Any]) -> int:
        """
        Estimate gas for transaction.

        Args:
            tx: Transaction dict (from, to, value, data, etc.)

        Returns:
            Estimated gas
        """
        result = self._call_rpc("eth_estimateGas", [tx])
        return int(result, 16) if isinstance(result, str) else result

    def send_raw_transaction(self, raw_tx: str) -> str:
        """
        Send raw transaction.

        Args:
            raw_tx: Raw transaction hex (0x prefix)

        Returns:
            Transaction hash
        """
        return self._call_rpc("eth_sendRawTransaction", [raw_tx])
