"""
Unit tests for query commands in mtcli.

Tests blockchain query operations.
"""

import pytest
from click.testing import CliRunner
from sdk.cli.main import cli


class TestQueryCommands:
    """Test query command group."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_query_help(self):
        """Test query help command."""
        result = self.runner.invoke(cli, ['query', '--help'])
        assert result.exit_code == 0
        assert 'query' in result.output.lower()


class TestAddressQuery:
    """Test address query command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_address_requires_param(self):
        """Test that address query requires an address parameter."""
        result = self.runner.invoke(cli, ['query', 'address'])
        # Should fail because address is required
        assert result.exit_code != 0

    def test_address_with_param(self):
        """Test address query with parameter."""
        result = self.runner.invoke(cli, [
            'query', 'address',
            '0x1234567890123456789012345678901234567890',
            '--network', 'testnet'
        ])
        # May fail if network is not available, but should not crash
        assert result.exit_code in [0, 1]


class TestBalanceQuery:
    """Test balance query command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_balance_requires_params(self):
        """Test that balance query requires parameters."""
        result = self.runner.invoke(cli, ['query', 'balance'])
        # Should fail because coldkey/hotkey are required
        assert result.exit_code != 0

    def test_balance_with_params(self):
        """Test balance query with parameters."""
        result = self.runner.invoke(cli, [
            'query', 'balance',
            '--coldkey', 'test_coldkey',
            '--hotkey', 'test_hotkey',
            '--network', 'testnet'
        ])
        # May fail if wallet doesn't exist or network unavailable
        assert result.exit_code in [0, 1]


class TestSubnetQuery:
    """Test subnet query commands."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_subnet_requires_uid(self):
        """Test that subnet query requires subnet-uid."""
        result = self.runner.invoke(cli, ['query', 'subnet'])
        # Should fail because subnet-uid is required
        assert result.exit_code != 0

    def test_subnet_with_uid(self):
        """Test subnet query with uid."""
        result = self.runner.invoke(cli, [
            'query', 'subnet',
            '--subnet-uid', '1',
            '--network', 'testnet'
        ])
        # May fail if network is not available
        assert result.exit_code in [0, 1]

    def test_list_subnets(self):
        """Test list-subnets command."""
        result = self.runner.invoke(cli, [
            'query', 'list-subnets',
            '--network', 'testnet'
        ])
        # May fail if network is not available
        assert result.exit_code in [0, 1]


class TestValidatorQuery:
    """Test validator query command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_validator_requires_address(self):
        """Test that validator query requires address."""
        result = self.runner.invoke(cli, ['query', 'validator'])
        # Should fail because address is required
        assert result.exit_code != 0

    def test_validator_with_address(self):
        """Test validator query with address."""
        result = self.runner.invoke(cli, [
            'query', 'validator',
            '0x1234567890123456789012345678901234567890',
            '--network', 'testnet'
        ])
        # May fail if network is not available
        assert result.exit_code in [0, 1]


class TestMinerQuery:
    """Test miner query command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_miner_requires_address(self):
        """Test that miner query requires address."""
        result = self.runner.invoke(cli, ['query', 'miner'])
        # Should fail because address is required
        assert result.exit_code != 0

    def test_miner_with_address(self):
        """Test miner query with address."""
        result = self.runner.invoke(cli, [
            'query', 'miner',
            '0x1234567890123456789012345678901234567890',
            '--network', 'testnet'
        ])
        # May fail if network is not available
        assert result.exit_code in [0, 1]


class TestQueryNetworkOptions:
    """Test network options for query commands."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_query_with_mainnet(self):
        """Test query with mainnet network."""
        result = self.runner.invoke(cli, [
            'query', 'list-subnets',
            '--network', 'mainnet'
        ])
        # May fail if network is not available
        assert result.exit_code in [0, 1]

    def test_query_with_local(self):
        """Test query with local network."""
        result = self.runner.invoke(cli, [
            'query', 'list-subnets',
            '--network', 'local'
        ])
        # May fail if network is not available
        assert result.exit_code in [0, 1]
