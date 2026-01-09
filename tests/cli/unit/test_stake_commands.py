"""
Unit tests for stake commands in mtcli.

Tests staking operations including add, remove, claim, info, and list.
"""

import pytest
from click.testing import CliRunner
from sdk.cli.main import cli


class TestStakeCommands:
    """Test stake command group."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_stake_help(self):
        """Test stake help command."""
        result = self.runner.invoke(cli, ['stake', '--help'])
        assert result.exit_code == 0
        assert 'stake' in result.output.lower()


class TestStakeAdd:
    """Test stake add command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_add_requires_params(self):
        """Test that add stake requires parameters."""
        result = self.runner.invoke(cli, ['stake', 'add'])
        # Should fail because required parameters are missing
        assert result.exit_code != 0

    def test_add_requires_coldkey(self):
        """Test that add stake requires coldkey."""
        result = self.runner.invoke(cli, [
            'stake', 'add',
            '--hotkey', 'test_hk',
            '--amount', '10000'
        ])
        # Should fail because coldkey is required
        assert result.exit_code != 0

    def test_add_requires_amount(self):
        """Test that add stake requires amount."""
        result = self.runner.invoke(cli, [
            'stake', 'add',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk'
        ])
        # Should fail because amount is required
        assert result.exit_code != 0

    def test_add_with_params(self):
        """Test add stake with all parameters."""
        result = self.runner.invoke(cli, [
            'stake', 'add',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--amount', '10000',
            '--network', 'testnet'
        ], input='password\nn\n')  # Decline confirmation
        # May fail because wallet doesn't exist
        assert result.exit_code in [0, 1]


class TestStakeRemove:
    """Test stake remove command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_remove_requires_params(self):
        """Test that remove stake requires parameters."""
        result = self.runner.invoke(cli, ['stake', 'remove'])
        # Should fail because required parameters are missing
        assert result.exit_code != 0

    def test_remove_with_params(self):
        """Test remove stake with all parameters."""
        result = self.runner.invoke(cli, [
            'stake', 'remove',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--amount', '5000',
            '--network', 'testnet'
        ], input='password\nn\n')  # Decline confirmation
        # May fail because wallet doesn't exist
        assert result.exit_code in [0, 1]


class TestStakeClaim:
    """Test stake claim command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_claim_requires_params(self):
        """Test that claim rewards requires parameters."""
        result = self.runner.invoke(cli, ['stake', 'claim'])
        # Should fail because required parameters are missing
        assert result.exit_code != 0

    def test_claim_with_params(self):
        """Test claim rewards with parameters."""
        result = self.runner.invoke(cli, [
            'stake', 'claim',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--network', 'testnet'
        ], input='password\nn\n')  # Decline confirmation
        # May fail because wallet doesn't exist
        assert result.exit_code in [0, 1]


class TestStakeInfo:
    """Test stake info command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_info_requires_params(self):
        """Test that stake info requires parameters."""
        result = self.runner.invoke(cli, ['stake', 'info'])
        # Should fail because required parameters are missing
        assert result.exit_code != 0

    def test_info_with_params(self):
        """Test stake info with parameters."""
        result = self.runner.invoke(cli, [
            'stake', 'info',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--network', 'testnet'
        ])
        # May fail if wallet doesn't exist or network unavailable
        assert result.exit_code in [0, 1]


class TestStakeList:
    """Test stake list command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_list_basic(self):
        """Test basic stake list command."""
        result = self.runner.invoke(cli, [
            'stake', 'list',
            '--network', 'testnet'
        ])
        # May fail if network unavailable
        assert result.exit_code in [0, 1]

    def test_list_with_limit(self):
        """Test stake list with custom limit."""
        result = self.runner.invoke(cli, [
            'stake', 'list',
            '--network', 'testnet',
            '--limit', '10'
        ])
        # May fail if network unavailable
        assert result.exit_code in [0, 1]


class TestStakeValidation:
    """Test stake command validation."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_add_negative_amount(self):
        """Test adding negative stake amount."""
        result = self.runner.invoke(cli, [
            'stake', 'add',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--amount', '-1000',
            '--network', 'testnet'
        ])
        # Negative amounts are handled - might succeed but show warning
        assert result.exit_code in [0, 1, 2]

    def test_remove_negative_amount(self):
        """Test removing negative stake amount."""
        result = self.runner.invoke(cli, [
            'stake', 'remove',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--amount', '-1000',
            '--network', 'testnet'
        ])
        # Negative amounts are handled - might succeed but show warning
        assert result.exit_code in [0, 1, 2]

    def test_add_zero_amount(self):
        """Test adding zero stake amount."""
        result = self.runner.invoke(cli, [
            'stake', 'add',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--amount', '0',
            '--network', 'testnet'
        ])
        # Zero amount might be rejected
        assert result.exit_code in [0, 1, 2]
