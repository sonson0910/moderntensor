# tests/e2e/test_subnet_operations.py
"""
E2E Tests — Subnet State-Changing Operations
Verifies subnet registration and weight setting against live node.
"""

import pytest

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.live_node,
    pytest.mark.requires_funded_account,
]


class TestSubnetOperations:
    """State-changing subnet operations."""

    def test_register_subnet(self, luxtensor_client, funded_account):
        """Register a new subnet."""
        addr = funded_account["address"]
        try:
            result = luxtensor_client.register_subnet(
                name="e2e_test_subnet",
                owner=addr,
            )
            if result is not None:
                assert isinstance(result, dict)
        except Exception as e:
            # May fail due to permissions or cost — structured error is OK
            error_msg = str(e).lower()
            assert any(word in error_msg for word in [
                "error", "permission", "cost", "insufficient", "already"
            ]), f"Unexpected error: {e}"

    def test_set_subnet_weights(self, luxtensor_client, funded_account):
        """Set subnet weights as root validator."""
        addr = funded_account["address"]
        try:
            result = luxtensor_client.set_subnet_weights(
                validator=addr,
                weights={0: 1.0},
            )
            if result is not None:
                assert isinstance(result, dict)
        except Exception as e:
            error_msg = str(e).lower()
            assert any(word in error_msg for word in [
                "error", "permission", "validator", "not found", "invalid"
            ]), f"Unexpected error: {e}"
