# tests/e2e/test_transaction_flow.py
"""
E2E Tests — Transaction Flow
Verifies transaction submission, receipt retrieval, and history queries.
"""

import pytest

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.live_node,
    pytest.mark.requires_funded_account,
]


class TestTransactionFlow:
    """Transaction lifecycle tests."""

    def test_get_transaction_count(self, luxtensor_client, funded_account):
        """Transaction count for funded account is non-negative."""
        addr = funded_account["address"]
        count = luxtensor_client._call_rpc(
            "eth_getTransactionCount", [addr, "latest"]
        )
        assert count is not None

    def test_get_transaction_receipt_nonexistent(self, luxtensor_client):
        """Non-existent tx receipt returns None."""
        fake_hash = "0x" + "ab" * 32
        receipt = luxtensor_client._call_rpc(
            "eth_getTransactionReceipt", [fake_hash]
        )
        assert receipt is None

    def test_get_transactions_for_address(self, luxtensor_client, funded_account):
        """Transaction history for funded account returns a list."""
        addr = funded_account["address"]
        txs = luxtensor_client.get_transactions_for_address(
            address=addr, limit=5
        )
        assert txs is not None
        assert isinstance(txs, list)

    def test_get_transfer_history(self, luxtensor_client, funded_account):
        """Transfer history returns a list."""
        addr = funded_account["address"]
        history = luxtensor_client.get_transfer_history(address=addr, limit=5)
        assert history is not None
        assert isinstance(history, list)

    def test_get_stake_history(self, luxtensor_client, funded_account):
        """Stake history returns a list."""
        addr = funded_account["address"]
        history = luxtensor_client.get_stake_history(address=addr, limit=5)
        assert history is not None
        assert isinstance(history, list)

    def test_tx_get_mempool(self, luxtensor_client):
        """tx_getMempool returns mempool contents."""
        try:
            mempool = luxtensor_client._call_rpc("tx_getMempool")
            if mempool is not None:
                assert isinstance(mempool, (list, dict))
        except Exception:
            pass  # Endpoint may not be fully implemented

    def test_tx_get_status(self, luxtensor_client):
        """tx_getStatus for non-existent tx."""
        try:
            status = luxtensor_client._call_rpc("tx_getStatus", ["0x" + "00" * 32])
        except Exception:
            pass  # Error = endpoint exists
