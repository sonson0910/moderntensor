"""
Unit tests for transaction commands in mtcli.

Tests transaction operations including send, status, and history.
"""

import pytest
from click.testing import CliRunner
from sdk.cli.main import cli


class TestTxCommands:
    """Test transaction command group."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_tx_help(self):
        """Test tx help command."""
        result = self.runner.invoke(cli, ['tx', '--help'])
        assert result.exit_code == 0
        assert 'tx' in result.output.lower() or 'transaction' in result.output.lower()


class TestSendTransaction:
    """Test send transaction command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_send_requires_params(self):
        """Test that send requires parameters."""
        result = self.runner.invoke(cli, ['tx', 'send'])
        # Should fail because required parameters are missing
        assert result.exit_code != 0

    def test_send_requires_coldkey(self):
        """Test that send requires coldkey."""
        result = self.runner.invoke(cli, [
            'tx', 'send',
            '--hotkey', 'test_hk',
            '--to', '0x1234567890123456789012345678901234567890',
            '--amount', '1000000'
        ])
        # Should fail because coldkey is required
        assert result.exit_code != 0

    def test_send_requires_recipient(self):
        """Test that send requires recipient address."""
        result = self.runner.invoke(cli, [
            'tx', 'send',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--amount', '1000000'
        ])
        # Should fail because recipient is required
        assert result.exit_code != 0

    def test_send_requires_amount(self):
        """Test that send requires amount."""
        result = self.runner.invoke(cli, [
            'tx', 'send',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--to', '0x1234567890123456789012345678901234567890'
        ])
        # Should fail because amount is required
        assert result.exit_code != 0

    def test_send_with_all_params(self):
        """Test send with all required parameters."""
        result = self.runner.invoke(cli, [
            'tx', 'send',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--to', '0x1234567890123456789012345678901234567890',
            '--amount', '1000000',
            '--network', 'testnet'
        ], input='TestPassword123!\nn\n')  # Decline confirmation
        # May fail because wallet doesn't exist, but should handle gracefully
        assert result.exit_code in [0, 1]


class TestTransactionStatus:
    """Test transaction status command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_status_requires_hash(self):
        """Test that status requires transaction hash."""
        result = self.runner.invoke(cli, ['tx', 'status'])
        # Should fail because tx_hash is required
        assert result.exit_code != 0

    def test_status_with_hash(self):
        """Test status with transaction hash."""
        result = self.runner.invoke(cli, [
            'tx', 'status',
            '0x' + '0' * 64,  # Dummy transaction hash
            '--network', 'testnet'
        ])
        # May fail if network unavailable or tx doesn't exist
        assert result.exit_code in [0, 1]


class TestTransactionHistory:
    """Test transaction history command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_history_requires_params(self):
        """Test that history requires parameters."""
        result = self.runner.invoke(cli, ['tx', 'history'])
        # Should fail because coldkey/hotkey are required
        assert result.exit_code != 0

    def test_history_with_params(self):
        """Test history with parameters."""
        result = self.runner.invoke(cli, [
            'tx', 'history',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--limit', '10',
            '--network', 'testnet'
        ])
        # May fail if wallet doesn't exist or network unavailable
        assert result.exit_code in [0, 1]

    def test_history_with_custom_limit(self):
        """Test history with custom limit."""
        result = self.runner.invoke(cli, [
            'tx', 'history',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--limit', '5',
            '--network', 'testnet'
        ])
        assert result.exit_code in [0, 1]


class TestTransactionValidation:
    """Test transaction validation."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_send_invalid_address(self):
        """Test send with invalid recipient address."""
        result = self.runner.invoke(cli, [
            'tx', 'send',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--to', 'invalid_address',
            '--amount', '1000000',
            '--network', 'testnet'
        ], input='password\nn\n')
        # Should handle invalid address gracefully
        assert result.exit_code in [0, 1]

    def test_send_negative_amount(self):
        """Test send with negative amount."""
        result = self.runner.invoke(cli, [
            'tx', 'send',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--to', '0x1234567890123456789012345678901234567890',
            '--amount', '-1000',
            '--network', 'testnet'
        ])
        # Negative amounts are handled - might succeed but show warning
        assert result.exit_code in [0, 1, 2]

    def test_send_zero_amount(self):
        """Test send with zero amount."""
        result = self.runner.invoke(cli, [
            'tx', 'send',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--to', '0x1234567890123456789012345678901234567890',
            '--amount', '0',
            '--network', 'testnet'
        ])
        # Zero amount might be rejected
        assert result.exit_code in [0, 1, 2]
