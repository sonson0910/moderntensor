# tests/e2e/test_staking_queries.py
"""
E2E Tests — Staking Query Operations
Verifies staking queries: validators, stake amounts, config, and minimums.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestStakingQueries:
    """Read-only staking queries."""

    def test_get_validators(self, luxtensor_client):
        """Validator list is retrievable."""
        validators = luxtensor_client.get_validators()
        assert validators is not None
        assert isinstance(validators, list)

    def test_get_total_stake(self, luxtensor_client):
        """Total stake is non-negative."""
        total = luxtensor_client.get_total_stake()
        assert total is not None
        assert total >= 0

    def test_get_stake_for_address(self, luxtensor_client, test_address):
        """Stake query for an address returns a value."""
        stake = luxtensor_client.get_stake(address=test_address)
        assert stake is not None
        assert stake >= 0

    def test_staking_minimums(self, luxtensor_client):
        """Staking minimums returns structured data."""
        mins = luxtensor_client.get_staking_minimums()
        assert mins is not None
        if isinstance(mins, dict):
            # Should have minimum stake amounts
            assert len(mins) > 0

    def test_staking_config(self, luxtensor_client):
        """staking_getConfig returns config data."""
        config = luxtensor_client._call_rpc("staking_getConfig")
        assert config is not None

    def test_active_validators(self, luxtensor_client):
        """staking_getActiveValidators returns a list."""
        active = luxtensor_client._call_rpc("staking_getActiveValidators")
        assert active is not None

    def test_get_validator_status(self, luxtensor_client, test_address):
        """Validator status query does not crash."""
        status = luxtensor_client.get_validator_status(address=test_address)
        # May be None for non-validator, but should not crash

    def test_stake_for_coldkey_hotkey(self, luxtensor_client, coldkey, hotkey):
        """Coldkey-hotkey stake pair query returns a value."""
        stake = luxtensor_client.get_stake_for_coldkey_and_hotkey(
            coldkey=coldkey, hotkey=hotkey
        )
        assert stake is not None
        assert stake >= 0

    def test_all_stake_for_coldkey(self, luxtensor_client, coldkey):
        """All stakes for coldkey returns a dict."""
        stakes = luxtensor_client.get_all_stake_for_coldkey(coldkey=coldkey)
        assert stakes is not None

    def test_total_stake_for_coldkey(self, luxtensor_client, coldkey):
        """Total stake for coldkey is non-negative."""
        total = luxtensor_client.get_total_stake_for_coldkey(coldkey=coldkey)
        assert total is not None
        assert total >= 0
