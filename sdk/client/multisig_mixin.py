"""
Multisig Wallet Mixin for LuxtensorClient

Wraps L1 multisig_* RPC methods for creating and managing
multi-signature wallets and their transactions.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class MultisigMixin:
    """
    Mixin providing multi-signature wallet methods.

    Methods:
        Wallet Management:
            - multisig_create_wallet()           — Create a new multisig wallet
            - multisig_get_wallet()              — Get wallet details

        Transaction Lifecycle:
            - multisig_propose_transaction()     — Propose a new transaction
            - multisig_approve_transaction()     — Approve a pending transaction
            - multisig_get_transaction()         — Get transaction details
            - multisig_get_pending_for_wallet()  — List pending transactions for a wallet
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    # ---------------------------------------------------------------
    # Wallet Management
    # ---------------------------------------------------------------

    def multisig_create_wallet(
        self,
        signers: List[str],
        threshold: int,
        name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Create a new multi-signature wallet.

        Args:
            signers: List of signer addresses (0x...)
            threshold: Minimum approvals required to execute a transaction
            name: Optional human-readable wallet name

        Returns:
            Dict with wallet_id, threshold, signers, created_at, name
        """
        try:
            params: Dict[str, Any] = {
                "signers": signers,
                "threshold": threshold,
            }
            if name:
                params["name"] = name
            return self._rpc()._call_rpc("multisig_createWallet", [params]) or {}
        except Exception as e:
            logger.error(f"Failed to create multisig wallet: {e}")
            raise

    def multisig_get_wallet(self, wallet_id: str) -> Optional[Dict[str, Any]]:
        """
        Get details of a multisig wallet.

        Args:
            wallet_id: Wallet ID

        Returns:
            Wallet info dict or None if not found
        """
        try:
            return self._rpc()._call_rpc("multisig_getWallet", [wallet_id])
        except Exception as e:
            logger.warning(f"Failed to get multisig wallet {wallet_id}: {e}")
            return None

    # ---------------------------------------------------------------
    # Transaction Lifecycle
    # ---------------------------------------------------------------

    def multisig_propose_transaction(
        self,
        wallet_id: str,
        proposer: str,
        to: str,
        value: str = "0",
        data: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Propose a new transaction from a multisig wallet.

        Args:
            wallet_id: Wallet ID
            proposer: Proposer address (must be a signer, 0x...)
            to: Recipient address (0x...)
            value: Amount to send (u128 as decimal string, default "0")
            data: Optional hex-encoded call data

        Returns:
            Dict with tx_id, wallet_id, to, value, approval_count, proposed_at
        """
        try:
            params: Dict[str, Any] = {
                "wallet_id": wallet_id,
                "proposer": proposer,
                "to": to,
                "value": value,
            }
            if data:
                params["data"] = data
            return self._rpc()._call_rpc("multisig_proposeTransaction", [params]) or {}
        except Exception as e:
            logger.error(
                f"Failed to propose multisig transaction in wallet {wallet_id}: {e}"
            )
            raise

    def multisig_approve_transaction(
        self,
        tx_id: str,
        signer: str,
    ) -> Dict[str, Any]:
        """
        Approve a pending multisig transaction.

        Args:
            tx_id: Transaction ID
            signer: Approving signer address (must be a wallet signer, 0x...)

        Returns:
            Dict with tx_id, approval_count, executed, approvals list
        """
        try:
            params: Dict[str, Any] = {
                "tx_id": tx_id,
                "signer": signer,
            }
            return (
                self._rpc()._call_rpc("multisig_approveTransaction", [params]) or {}
            )
        except Exception as e:
            logger.error(f"Failed to approve multisig transaction {tx_id}: {e}")
            return {"success": False, "error": str(e)}

    def multisig_get_transaction(self, tx_id: str) -> Optional[Dict[str, Any]]:
        """
        Get details of a multisig transaction.

        Args:
            tx_id: Transaction ID

        Returns:
            Transaction info dict or None if not found
        """
        try:
            return self._rpc()._call_rpc("multisig_getTransaction", [tx_id])
        except Exception as e:
            logger.warning(f"Failed to get multisig transaction {tx_id}: {e}")
            return None

    def multisig_get_pending_for_wallet(
        self, wallet_id: str
    ) -> List[Dict[str, Any]]:
        """
        List all pending (unexpired, unexecuted) transactions for a wallet.

        Args:
            wallet_id: Wallet ID

        Returns:
            List of pending transaction dicts
        """
        try:
            params: Dict[str, Any] = {"wallet_id": wallet_id}
            result = self._rpc()._call_rpc(
                "multisig_getPendingForWallet", [params]
            )
            if isinstance(result, dict):
                return result.get("pending_transactions", [])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning(
                f"Failed to get pending transactions for wallet {wallet_id}: {e}"
            )
            return []
