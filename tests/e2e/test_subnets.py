# tests/e2e/test_subnets.py
"""
E2E Tests — Subnet Operations
Verifies subnet listing, existence checks, hyperparameters, and metadata.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestSubnetQueries:
    """Read-only subnet queries."""

    def test_get_all_subnets(self, luxtensor_client):
        """Subnet list is retrievable (may be empty on dev node)."""
        subnets = luxtensor_client.get_all_subnets()
        assert subnets is not None
        assert isinstance(subnets, list)

    def test_subnet_exists_default(self, luxtensor_client):
        """Subnet 0 existence check does not crash."""
        exists = luxtensor_client.subnet_exists(subnet_id=0)
        assert isinstance(exists, bool)

    def test_subnet_not_exists(self, luxtensor_client):
        """Non-existent subnet returns False."""
        exists = luxtensor_client.subnet_exists(subnet_id=99999)
        assert exists is False

    def test_get_subnet_info(self, luxtensor_client):
        """Subnet 0 info is retrievable."""
        info = luxtensor_client.get_subnet_info(subnet_id=0)
        assert info is not None

    def test_subnet_hyperparameters(self, luxtensor_client):
        """Subnet hyperparameters query does not crash."""
        params = luxtensor_client.get_subnet_hyperparameters(subnet_id=0)
        # May be None if subnet 0 doesn't exist on dev node
        if params is not None:
            assert isinstance(params, dict)

    def test_subnet_count(self, luxtensor_client):
        """Subnet count is non-negative."""
        count = luxtensor_client.get_subnet_count()
        assert isinstance(count, int)
        assert count >= 0

    def test_subnet_tempo(self, luxtensor_client):
        """Subnet tempo is a non-negative integer."""
        tempo = luxtensor_client.get_subnet_tempo(subnet_id=0)
        assert isinstance(tempo, int)
        assert tempo >= 0

    def test_subnet_emission(self, luxtensor_client):
        """Subnet emission is a non-negative integer."""
        emission = luxtensor_client.get_subnet_emission(subnet_id=0)
        assert isinstance(emission, int)
        assert emission >= 0

    def test_subnet_owner(self, luxtensor_client):
        """Subnet 0 has an owner address."""
        owner = luxtensor_client.get_subnet_owner(subnet_id=0)
        if owner is not None:
            assert isinstance(owner, str)
            assert owner.startswith("0x")

    def test_total_subnets(self, luxtensor_client):
        """Total subnets is non-negative."""
        total = luxtensor_client.get_total_subnets()
        assert isinstance(total, int)
        assert total >= 0

    def test_max_subnets(self, luxtensor_client):
        """Max subnets is a positive integer."""
        max_s = luxtensor_client.get_max_subnets()
        assert isinstance(max_s, int)
        assert max_s > 0

    def test_root_validators(self, luxtensor_client):
        """Root validators query returns a list."""
        validators = luxtensor_client.get_root_validators()
        assert validators is not None
        assert isinstance(validators, list)

    def test_root_config(self, luxtensor_client):
        """Root config is retrievable."""
        config = luxtensor_client.get_root_config()
        assert config is not None

    def test_subnet_emissions_distribution(self, luxtensor_client):
        """Subnet emission distribution returns data."""
        emissions = luxtensor_client.get_subnet_emissions()
        assert emissions is not None
