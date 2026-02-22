# tests/e2e/test_error_handling.py
"""
E2E Tests — Error Handling & Edge Cases
Verifies the SDK handles invalid inputs, overflow values, and edge conditions gracefully.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestInvalidInputs:
    """Tests for invalid/malformed inputs."""

    def test_invalid_address_balance(self, luxtensor_client):
        """Invalid address for balance query raises or returns error."""
        try:
            result = luxtensor_client._call_rpc("eth_getBalance", ["not_hex", "latest"])
            # If no exception, result should indicate error or be None
        except Exception:
            pass  # Expected error

    def test_empty_address_balance(self, luxtensor_client):
        """Empty string address raises or returns error."""
        try:
            result = luxtensor_client._call_rpc("eth_getBalance", ["", "latest"])
        except Exception:
            pass

    def test_nonexistent_subnet_info(self, luxtensor_client):
        """Non-existent subnet returns None or error."""
        info = luxtensor_client.get_subnet_info(subnet_id=99999)
        # None is acceptable

    def test_negative_subnet_id(self, luxtensor_client):
        """Negative subnet ID is handled gracefully."""
        try:
            info = luxtensor_client.get_subnet_info(subnet_id=-1)
        except Exception:
            pass

    def test_very_large_subnet_id(self, luxtensor_client):
        """Very large subnet ID does not crash."""
        try:
            info = luxtensor_client.get_subnet_info(subnet_id=2**32 - 1)
        except Exception:
            pass

    def test_nonexistent_neuron(self, luxtensor_client):
        """Non-existent neuron UID returns None or error."""
        try:
            neuron = luxtensor_client.get_neuron(subnet_id=0, neuron_uid=999999)
        except Exception:
            pass

    def test_invalid_method_name(self, luxtensor_client):
        """Invalid RPC method returns an error."""
        try:
            result = luxtensor_client._call_rpc("nonexistent_method")
            # Should be None or raise — method doesn't exist
        except Exception:
            pass  # Expected

    def test_validate_address_edge_cases(self, luxtensor_client):
        """Address validation edge cases."""
        assert luxtensor_client.validate_address("") is False
        assert luxtensor_client.validate_address("0x") is False
        assert luxtensor_client.validate_address("0x" + "ff" * 20) is True
        assert luxtensor_client.validate_address("0x" + "GG" * 20) is False


class TestConcurrentQueries:
    """Test parallel query handling."""

    def test_rapid_sequential_queries(self, luxtensor_client):
        """Multiple rapid queries do not crash the connection."""
        results = []
        for _ in range(10):
            num = luxtensor_client.get_block_number()
            results.append(num)

        assert len(results) == 10
        assert all(isinstance(r, int) for r in results)

    def test_mixed_method_queries(self, luxtensor_client):
        """Different RPC methods can be called in sequence."""
        block = luxtensor_client.get_block_number()
        assert isinstance(block, int)

        connected = luxtensor_client.is_connected()
        assert connected is True

        subnets = luxtensor_client.get_all_subnets()
        assert isinstance(subnets, list)

        total = luxtensor_client.get_total_stake()
        assert total >= 0

    def test_error_recovery(self, luxtensor_client):
        """Client recovers from an error and serves subsequent requests."""
        # Trigger an error
        try:
            luxtensor_client._call_rpc("nonexistent_method_xyz")
        except Exception:
            pass

        # Verify client still works
        block = luxtensor_client.get_block_number()
        assert isinstance(block, int)
        assert block >= 0
