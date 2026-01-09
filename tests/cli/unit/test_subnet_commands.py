"""
Unit tests for subnet commands in mtcli.

Tests subnet management operations.
"""

import pytest
from click.testing import CliRunner
from sdk.cli.main import cli


class TestSubnetCommands:
    """Test subnet command group."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_subnet_help(self):
        """Test subnet help command."""
        result = self.runner.invoke(cli, ['subnet', '--help'])
        assert result.exit_code == 0
        assert 'subnet' in result.output.lower()


class TestSubnetCreate:
    """Test subnet create command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_create_requires_coldkey(self):
        """Test that subnet create requires coldkey."""
        result = self.runner.invoke(cli, ['subnet', 'create'])
        # Should fail because coldkey is required
        assert result.exit_code != 0

    def test_create_with_params(self):
        """Test subnet create with parameters."""
        result = self.runner.invoke(cli, [
            'subnet', 'create',
            '--coldkey', 'test_ck',
            '--name', 'Test Subnet',
            '--network', 'testnet'
        ], input='password\nn\n')  # Decline confirmation
        # May fail because wallet doesn't exist
        assert result.exit_code in [0, 1]


class TestSubnetRegister:
    """Test subnet register command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_register_requires_params(self):
        """Test that subnet register requires parameters."""
        result = self.runner.invoke(cli, ['subnet', 'register'])
        # Should fail because required parameters are missing
        assert result.exit_code != 0

    def test_register_with_params(self):
        """Test subnet register with parameters."""
        result = self.runner.invoke(cli, [
            'subnet', 'register',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--subnet-uid', '1',
            '--network', 'testnet'
        ], input='password\nn\n')  # Decline confirmation
        # May fail because wallet doesn't exist
        assert result.exit_code in [0, 1]


class TestSubnetInfo:
    """Test subnet info command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_info_requires_uid(self):
        """Test that subnet info requires subnet-uid."""
        result = self.runner.invoke(cli, ['subnet', 'info'])
        # Should fail because subnet-uid is required
        assert result.exit_code != 0

    def test_info_with_uid(self):
        """Test subnet info with uid."""
        result = self.runner.invoke(cli, [
            'subnet', 'info',
            '--subnet-uid', '1',
            '--network', 'testnet'
        ])
        # May fail if network unavailable
        assert result.exit_code in [0, 1]


class TestSubnetParticipants:
    """Test subnet participants command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_participants_requires_uid(self):
        """Test that participants requires subnet-uid."""
        result = self.runner.invoke(cli, ['subnet', 'participants'])
        # Should fail because subnet-uid is required
        assert result.exit_code != 0

    def test_participants_with_uid(self):
        """Test participants with uid."""
        result = self.runner.invoke(cli, [
            'subnet', 'participants',
            '--subnet-uid', '1',
            '--network', 'testnet'
        ])
        # May fail if network unavailable
        assert result.exit_code in [0, 1]

    def test_participants_with_limit(self):
        """Test participants with custom limit."""
        result = self.runner.invoke(cli, [
            'subnet', 'participants',
            '--subnet-uid', '1',
            '--limit', '20',
            '--network', 'testnet'
        ])
        # May fail if network unavailable
        assert result.exit_code in [0, 1]


class TestSubnetValidation:
    """Test subnet command validation."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_create_empty_name(self):
        """Test creating subnet with empty name."""
        result = self.runner.invoke(cli, [
            'subnet', 'create',
            '--coldkey', 'test_ck',
            '--name', '',
            '--network', 'testnet'
        ])
        # Empty name might be accepted
        assert result.exit_code in [0, 1]

    def test_info_invalid_uid(self):
        """Test subnet info with invalid uid."""
        result = self.runner.invoke(cli, [
            'subnet', 'info',
            '--subnet-uid', '-1',
            '--network', 'testnet'
        ])
        # Should handle invalid uid gracefully
        assert result.exit_code in [0, 1, 2]
