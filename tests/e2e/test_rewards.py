# tests/e2e/test_rewards.py
"""
E2E Tests — Reward & Tokenomics Queries
Verifies reward stats, burn stats, DAO balance, and reward history.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestRewardQueries:
    """Read-only reward and tokenomics queries."""

    def test_reward_stats(self, luxtensor_client):
        """Reward executor stats are retrievable."""
        stats = luxtensor_client.get_reward_stats()
        assert stats is not None

    def test_burn_stats(self, luxtensor_client):
        """Burn statistics are retrievable."""
        stats = luxtensor_client.get_burn_stats()
        assert stats is not None

    def test_dao_balance(self, luxtensor_client):
        """DAO treasury balance is non-negative."""
        balance = luxtensor_client.get_dao_balance()
        assert balance is not None
        assert balance >= 0

    def test_pending_rewards(self, luxtensor_client, test_address):
        """Pending rewards for address is non-negative."""
        rewards = luxtensor_client.get_pending_rewards(address=test_address)
        if rewards is not None:
            assert rewards >= 0

    def test_reward_balance(self, luxtensor_client, test_address):
        """Full reward balance info is retrievable."""
        balance = luxtensor_client.get_reward_balance(address=test_address)
        assert balance is not None

    def test_reward_history(self, luxtensor_client, test_address):
        """Reward history returns a list."""
        history = luxtensor_client.get_reward_history(address=test_address, limit=5)
        assert history is not None
        assert isinstance(history, list)
