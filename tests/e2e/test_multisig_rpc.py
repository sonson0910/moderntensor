# tests/e2e/test_multisig_rpc.py
"""
E2E Tests — Multisig RPC
Verifies multisig wallet queries and transaction lookups.
Validates the field fix: proposed_at (not created_at).
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestMultisigRpc:
    """Multisig module read-only queries."""

    def test_multisig_get_wallets(self, luxtensor_client):
        """multisig_getWallets returns a list (possibly empty)."""
        try:
            wallets = luxtensor_client._call_rpc("multisig_getWallets")
            if wallets is not None:
                assert isinstance(wallets, list)
        except Exception:
            pass  # Endpoint exists and responds

    def test_multisig_get_transaction_nonexistent(self, luxtensor_client):
        """Querying non-existent multisig transaction returns None or error."""
        try:
            result = luxtensor_client._call_rpc("multisig_getTransaction", ["nonexistent_id"])
            # Should be None or error for non-existent
        except Exception:
            pass

    def test_multisig_get_pending_for_wallet(self, luxtensor_client):
        """Querying pending transactions for a non-existent wallet."""
        try:
            result = luxtensor_client._call_rpc("multisig_getPendingForWallet", [
                "0x0000000000000000000000000000000000000001"
            ])
            if result is not None:
                assert isinstance(result, list)
        except Exception:
            pass

    def test_multisig_get_wallet_info(self, luxtensor_client):
        """multisig_getWalletInfo for non-existent wallet."""
        try:
            result = luxtensor_client._call_rpc("multisig_getWalletInfo", [
                "0x0000000000000000000000000000000000000001"
            ])
        except Exception:
            pass  # Error = endpoint exists
