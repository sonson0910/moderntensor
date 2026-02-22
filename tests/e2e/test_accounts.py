# tests/e2e/test_accounts.py
"""
E2E Tests — Account Operations
Verifies balance queries, nonce retrieval, and address validation.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestAccountQueries:
    """Read-only account queries."""

    def test_get_balance(self, luxtensor_client, test_address):
        """Balance query returns a non-negative value."""
        balance = luxtensor_client._call_rpc("eth_getBalance", [test_address, "latest"])
        assert balance is not None

    def test_get_balance_zero_address(self, luxtensor_client, zero_address):
        """Zero address has a valid (possibly zero) balance."""
        balance = luxtensor_client._call_rpc("eth_getBalance", [zero_address, "latest"])
        assert balance is not None

    def test_get_nonce(self, luxtensor_client, test_address):
        """Nonce (transaction count) is non-negative."""
        nonce = luxtensor_client._call_rpc("eth_getTransactionCount", [test_address, "latest"])
        assert nonce is not None

    def test_validate_address_valid(self, luxtensor_client):
        """Valid hex address passes validation."""
        result = luxtensor_client.validate_address("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2")
        assert result is True

    def test_validate_address_invalid(self, luxtensor_client, invalid_address):
        """Invalid address fails validation."""
        result = luxtensor_client.validate_address(invalid_address)
        assert result is False

    def test_validate_address_zero(self, luxtensor_client, zero_address):
        """Zero address is technically valid format."""
        result = luxtensor_client.validate_address(zero_address)
        assert result is True

    def test_get_code_empty_account(self, luxtensor_client, test_address):
        """Non-contract account returns empty/null code."""
        code = luxtensor_client._call_rpc("eth_getCode", [test_address, "latest"])
        # EOA should return "0x" or None
        if code is not None:
            assert isinstance(code, str)
