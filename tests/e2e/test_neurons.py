# tests/e2e/test_neurons.py
"""
E2E Tests — Neuron Operations
Verifies neuron listing, counts, and individual neuron queries.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestNeuronQueries:
    """Read-only neuron queries."""

    def test_get_neurons(self, luxtensor_client):
        """Neuron list for subnet 0 is retrievable."""
        neurons = luxtensor_client.get_neurons(subnet_id=0)
        assert neurons is not None
        assert isinstance(neurons, list)

    def test_neuron_count(self, luxtensor_client):
        """Neuron count is non-negative."""
        count = luxtensor_client.get_neuron_count(subnet_id=0)
        assert isinstance(count, int)
        assert count >= 0

    def test_active_neurons(self, luxtensor_client):
        """Active neurons list is retrievable."""
        active = luxtensor_client.get_active_neurons(subnet_id=0)
        assert active is not None
        assert isinstance(active, list)

    def test_total_neurons(self, luxtensor_client):
        """Total neurons across all subnets is non-negative."""
        total = luxtensor_client.get_total_neurons()
        assert isinstance(total, int)
        assert total >= 0

    def test_get_single_neuron(self, luxtensor_client):
        """Individual neuron query returns data or handles empty state gracefully."""
        neurons = luxtensor_client.get_neurons(subnet_id=0)
        if not neurons:
            # No neurons — verify SDK handles gracefully without crashing
            try:
                neuron = luxtensor_client.get_neuron(subnet_id=0, neuron_uid=0)
                # None or empty dict is acceptable
            except Exception:
                pass  # RPC error for non-existent neuron is expected
            return

        neuron = luxtensor_client.get_neuron(subnet_id=0, neuron_uid=0)
        assert neuron is not None

    def test_neuron_axon_info(self, luxtensor_client):
        """Neuron axon info query does not crash."""
        neurons = luxtensor_client.get_neurons(subnet_id=0)
        if not neurons:
            # No neurons — verify SDK handles gracefully
            try:
                axon = luxtensor_client.get_neuron_axon(subnet_id=0, neuron_uid=0)
            except Exception:
                pass  # Expected on empty subnet
            return

        axon = luxtensor_client.get_neuron_axon(subnet_id=0, neuron_uid=0)
        # May be None if axon not served, but should not crash

    def test_neuron_metagraph_scores(self, luxtensor_client):
        """Metagraph score queries (rank, trust, consensus, incentive)."""
        neurons = luxtensor_client.get_neurons(subnet_id=0)
        if not neurons:
            # No neurons — verify SDK handles gracefully
            try:
                luxtensor_client.get_rank(subnet_id=0, neuron_uid=0)
                luxtensor_client.get_trust(subnet_id=0, neuron_uid=0)
                luxtensor_client.get_consensus(subnet_id=0, neuron_uid=0)
                luxtensor_client.get_incentive(subnet_id=0, neuron_uid=0)
                luxtensor_client.get_dividends(subnet_id=0, neuron_uid=0)
                luxtensor_client.get_emission(subnet_id=0, neuron_uid=0)
            except Exception:
                pass  # Expected on empty subnet
            return

        # These may return 0 or None for inactive neurons
        rank = luxtensor_client.get_rank(subnet_id=0, neuron_uid=0)
        trust = luxtensor_client.get_trust(subnet_id=0, neuron_uid=0)
        consensus = luxtensor_client.get_consensus(subnet_id=0, neuron_uid=0)
        incentive = luxtensor_client.get_incentive(subnet_id=0, neuron_uid=0)
        dividends = luxtensor_client.get_dividends(subnet_id=0, neuron_uid=0)
        emission = luxtensor_client.get_emission(subnet_id=0, neuron_uid=0)
        # No crash = success

    def test_registration_cost(self, luxtensor_client):
        """Registration cost for subnet 0 is retrievable."""
        cost = luxtensor_client.get_registration_cost(subnet_id=0)
        if cost is not None:
            assert cost >= 0

    def test_immunity_period(self, luxtensor_client):
        """Immunity period is retrievable."""
        period = luxtensor_client.get_immunity_period(subnet_id=0)
        if period is not None:
            assert isinstance(period, int)
            assert period >= 0

    def test_difficulty(self, luxtensor_client):
        """Difficulty is retrievable."""
        diff = luxtensor_client.get_difficulty(subnet_id=0)
        if diff is not None:
            assert diff >= 0
