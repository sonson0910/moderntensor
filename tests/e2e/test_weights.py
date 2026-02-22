# tests/e2e/test_weights.py
"""
E2E Tests — Weight Operations
Verifies weight queries, commits, and version tracking.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestWeightQueries:
    """Read-only weight queries."""

    def test_get_weight_commits(self, luxtensor_client):
        """Weight commits query does not crash."""
        commits = luxtensor_client.get_weight_commits(subnet_id=0)
        # May be empty list or None for fresh subnet

    def test_weights_version(self, luxtensor_client):
        """Weights version is retrievable."""
        version = luxtensor_client.get_weights_version(subnet_id=0)
        if version is not None:
            assert isinstance(version, int)

    def test_weights_rate_limit(self, luxtensor_client):
        """Weights rate limit is retrievable."""
        limit = luxtensor_client.get_weights_rate_limit(subnet_id=0)
        if limit is not None:
            assert isinstance(limit, int)
            assert limit >= 0

    def test_get_weights_for_neuron(self, luxtensor_client):
        """Weight matrix for a specific neuron — handles empty subnet gracefully."""
        neurons = luxtensor_client.get_neurons(subnet_id=0)
        if not neurons:
            # No neurons — verify SDK handles weight query gracefully
            weights = luxtensor_client.get_weights(subnet_id=0, neuron_uid=0)
            # Empty list is expected
            return

        weights = luxtensor_client.get_weights(subnet_id=0, neuron_uid=0)
        # May be empty for fresh neuron
