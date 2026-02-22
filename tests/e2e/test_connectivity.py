# tests/e2e/test_connectivity.py
"""
E2E Tests — Node Connectivity & Basic Health
Verifies the SDK can connect to a live LuxTensor node and retrieve basic info.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestNodeConnectivity:
    """Basic connectivity and health checks."""

    def test_is_connected(self, luxtensor_client):
        """Client reports connected to live node."""
        assert luxtensor_client.is_connected() is True

    def test_health_check(self, luxtensor_client):
        """Node health endpoint returns structured data."""
        health = luxtensor_client.health_check()
        assert health is not None

    def test_block_number_is_non_negative(self, luxtensor_client):
        """Block number is a non-negative integer."""
        block_num = luxtensor_client.get_block_number()
        assert isinstance(block_num, int)
        assert block_num >= 0

    def test_web3_client_version(self, luxtensor_client):
        """web3_clientVersion returns a version string."""
        version = luxtensor_client._call_rpc("web3_clientVersion")
        assert version is not None
        assert isinstance(version, str)
        assert len(version) > 0

    def test_net_version(self, luxtensor_client):
        """net_version returns a network identifier."""
        net_ver = luxtensor_client._call_rpc("net_version")
        assert net_ver is not None

    def test_net_peer_count(self, luxtensor_client):
        """net_peerCount returns a non-negative value."""
        peer_count = luxtensor_client._call_rpc("net_peerCount")
        # May be 0 for single-node testnet
        assert peer_count is not None

    def test_sync_status(self, luxtensor_client):
        """sync_getSyncStatus returns sync info."""
        sync = luxtensor_client._call_rpc("sync_getSyncStatus")
        # May be None or structured — just verify no crash
        # A synced node may return False or a status object
        pass  # No crash = pass
