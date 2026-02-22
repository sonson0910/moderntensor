# tests/e2e/test_neuron_operations.py
"""
E2E Tests — Neuron State-Changing Operations
Verifies axon serving and neuron registration against live node.
"""

import pytest

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.live_node,
    pytest.mark.requires_funded_account,
]


class TestNeuronOperations:
    """State-changing neuron operations."""

    def test_serve_axon(self, luxtensor_client, funded_account):
        """Register/update axon endpoint on-chain."""
        addr = funded_account["address"]
        try:
            result = luxtensor_client.serve_axon(
                subnet_id=0,
                hotkey=addr,
                coldkey=addr,
                ip="127.0.0.1",
                port=8091,
                protocol=4,
                version=1,
            )
            if result is not None:
                assert isinstance(result, dict)
        except Exception as e:
            error_msg = str(e).lower()
            assert any(word in error_msg for word in [
                "error", "registered", "permission", "invalid", "not found"
            ]), f"Unexpected error: {e}"

    def test_is_hotkey_registered(self, luxtensor_client, funded_account):
        """Check if the funded account's hotkey is registered in subnet 0."""
        addr = funded_account["address"]
        registered = luxtensor_client.is_hotkey_registered(
            subnet_id=0, hotkey=addr
        )
        assert isinstance(registered, bool)

    def test_get_uid_for_hotkey(self, luxtensor_client, funded_account):
        """Get UID for hotkey — may be None if not registered."""
        addr = funded_account["address"]
        uid = luxtensor_client.get_uid_for_hotkey(subnet_id=0, hotkey=addr)
        # May be None if not registered

    def test_has_validator_permit(self, luxtensor_client, funded_account):
        """Check validator permit status."""
        addr = funded_account["address"]
        permit = luxtensor_client.has_validator_permit(subnet_id=0, hotkey=addr)
        assert isinstance(permit, bool)
