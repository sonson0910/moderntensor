# tests/e2e/test_network_info.py
"""
E2E Tests — Network Information
Verifies global network statistics and issuance data.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestNetworkInfo:
    """Global network information queries."""

    def test_total_issuance(self, luxtensor_client):
        """Total token issuance is non-negative."""
        total = luxtensor_client.get_total_issuance()
        assert isinstance(total, int)
        assert total >= 0

    def test_network_info(self, luxtensor_client):
        """Network info returns structured data."""
        info = luxtensor_client.get_network_info()
        assert info is not None
        if isinstance(info, dict):
            assert len(info) > 0

    def test_total_neurons(self, luxtensor_client):
        """Total neurons count is non-negative."""
        total = luxtensor_client.get_total_neurons()
        assert isinstance(total, int)
        assert total >= 0

    def test_total_subnets(self, luxtensor_client):
        """Total subnets is non-negative."""
        total = luxtensor_client.get_total_subnets()
        assert isinstance(total, int)
        assert total >= 0

    def test_max_subnets(self, luxtensor_client):
        """Max subnets is a positive number."""
        max_s = luxtensor_client.get_max_subnets()
        assert isinstance(max_s, int)
        assert max_s > 0
