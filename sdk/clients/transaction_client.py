"""
Transaction Client
Handles transaction submission and queries.
"""

import logging
from typing import Optional, Dict, Any, List
from dataclasses import dataclass
from .base import BaseRpcClient

logger = logging.getLogger(__name__)


@dataclass
class TransactionResult:
    """Transaction submission result"""
    tx_hash: str
    status: str
    block_number: Optional[int] = None
    error: Optional[str] = None


class TransactionClient(BaseRpcClient):
    """
    Client for transaction operations.
    Single Responsibility: Transaction handling only.
    """

    def submit_transaction(self, signed_tx: str) -> TransactionResult:
        """
        Submit signed transaction.

        Args:
            signed_tx: Signed transaction (hex, 0x prefix)

        Returns:
            TransactionResult
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
            Transaction data
        """
        return self._call_rpc("eth_getTransactionByHash", [tx_hash])

    def get_transaction_receipt(self, tx_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get transaction receipt.

        Args:
            tx_hash: Transaction hash

        Returns:
            Receipt with execution result
        """
        return self._call_rpc("tx_getReceipt", [tx_hash])

    def wait_for_transaction(
        self, tx_hash: str, timeout: int = 60
    ) -> Optional[Dict[str, Any]]:
        """
        Wait for transaction to be mined.

        Args:
            tx_hash: Transaction hash
            timeout: Max wait time in seconds

        Returns:
            Receipt when mined, None if timeout
        """
        import time
        start = time.time()
        while time.time() - start < timeout:
            receipt = self.get_transaction_receipt(tx_hash)
            if receipt:
                return receipt
            time.sleep(1)
        return None

    def get_transactions_for_address(
        self, address: str, limit: int = 10
    ) -> List[Dict[str, Any]]:
        """
        Get transaction history for address.

        Args:
            address: Account address
            limit: Max transactions

        Returns:
            List of transactions
        """
        try:
            result = self._call_rpc("tx_getByAddress", [address, limit])
            return result if result else []
        except Exception as e:
            logger.warning(f"Failed to get transactions: {e}")
            return []

    def get_pending_transactions(self) -> List[Dict[str, Any]]:
        """
        Get pending transactions in mempool.

        Returns:
            List of pending transactions
        """
        try:
            result = self._call_rpc("tx_pendingTransactions", [])
            return result if result else []
        except Exception as e:
            logger.warning(f"Failed to get pending transactions: {e}")
            return []

    def estimate_gas(self, tx_params: Dict[str, Any]) -> int:
        """
        Estimate gas for transaction.

        Args:
            tx_params: Transaction parameters

        Returns:
            Estimated gas
        """
        try:
            result = self._call_rpc("eth_estimateGas", [tx_params])
            return self._hex_to_int(result)
        except Exception as e:
            logger.warning(f"Failed to estimate gas: {e}")
            return 21000  # Default gas

    def get_gas_price(self) -> int:
        """
        Get current gas price.

        Returns:
            Gas price in wei
        """
        try:
            result = self._call_rpc("eth_gasPrice", [])
            return self._hex_to_int(result)
        except Exception as e:
            logger.warning(f"Failed to get gas price: {e}")
            return 0
