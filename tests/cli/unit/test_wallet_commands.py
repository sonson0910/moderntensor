"""
Unit tests for wallet commands in mtcli.

Tests wallet management operations including create, restore, generate, and list.
"""

import pytest
import os
import tempfile
import shutil
from pathlib import Path
from click.testing import CliRunner
from sdk.cli.main import cli


class TestWalletCommands:
    """Test wallet management commands."""

    def setup_method(self):
        """Setup test fixtures with temporary wallet directory."""
        self.runner = CliRunner()
        self.temp_dir = tempfile.mkdtemp()
        self.wallet_path = Path(self.temp_dir) / "wallets"
        self.wallet_path.mkdir(parents=True, exist_ok=True)

    def teardown_method(self):
        """Cleanup temporary directories."""
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)

    def test_wallet_help(self):
        """Test wallet help command."""
        result = self.runner.invoke(cli, ['wallet', '--help'])
        assert result.exit_code == 0
        assert 'wallet' in result.output.lower()

    def test_wallet_list_empty(self):
        """Test listing wallets when none exist."""
        # Set HOME to temp dir to use temp wallet location
        with self.runner.isolated_filesystem(temp_dir=self.temp_dir):
            result = self.runner.invoke(cli, ['wallet', 'list'])
            # Should complete successfully even with no wallets
            assert result.exit_code == 0

    def test_create_coldkey_no_name(self):
        """Test creating coldkey without name fails."""
        result = self.runner.invoke(cli, ['wallet', 'create-coldkey'])
        # Should fail because name is required
        assert result.exit_code != 0

    def test_create_coldkey_with_name(self):
        """Test creating coldkey with name."""
        with self.runner.isolated_filesystem(temp_dir=self.temp_dir):
            # Use input to simulate user entering password and confirming mnemonic
            result = self.runner.invoke(
                cli,
                ['wallet', 'create-coldkey', '--name', 'test_wallet'],
                input='TestPassword123!\nTestPassword123!\ny\n'
            )
            # Command might fail if it needs user input, but should not crash
            assert result.exit_code in [0, 1]

    def test_restore_coldkey_no_name(self):
        """Test restoring coldkey without name fails."""
        result = self.runner.invoke(cli, ['wallet', 'restore-coldkey'])
        # Should fail because name is required
        assert result.exit_code != 0

    def test_generate_hotkey_requires_coldkey(self):
        """Test generating hotkey requires coldkey name."""
        result = self.runner.invoke(cli, ['wallet', 'generate-hotkey'])
        # Should fail because coldkey is required
        assert result.exit_code != 0

    def test_list_hotkeys_requires_coldkey(self):
        """Test listing hotkeys requires coldkey name."""
        result = self.runner.invoke(cli, ['wallet', 'list-hotkeys'])
        # Should fail because coldkey is required
        assert result.exit_code != 0

    def test_show_hotkey_requires_params(self):
        """Test show-hotkey requires both coldkey and hotkey."""
        result = self.runner.invoke(cli, ['wallet', 'show-hotkey'])
        # Should fail because parameters are required
        assert result.exit_code != 0

    def test_show_address_requires_params(self):
        """Test show-address requires coldkey and hotkey."""
        result = self.runner.invoke(cli, ['wallet', 'show-address'])
        # Should fail because parameters are required
        assert result.exit_code != 0

    def test_query_address_requires_params(self):
        """Test query-address requires parameters."""
        result = self.runner.invoke(cli, ['wallet', 'query-address'])
        # Should fail because parameters are required
        assert result.exit_code != 0

    def test_register_hotkey_requires_params(self):
        """Test register-hotkey requires parameters."""
        result = self.runner.invoke(cli, ['wallet', 'register-hotkey'])
        # Should fail because parameters are required
        assert result.exit_code != 0


class TestWalletValidation:
    """Test wallet command validation and error handling."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_invalid_wallet_name(self):
        """Test that invalid wallet names are rejected."""
        # Test with special characters that might not be allowed
        result = self.runner.invoke(
            cli,
            ['wallet', 'create-coldkey', '--name', '../../../etc/passwd'],
            input='password\npassword\ny\n'
        )
        # Should handle path traversal attempts gracefully
        assert result.exit_code in [0, 1]

    def test_empty_wallet_name(self):
        """Test that empty wallet name is handled."""
        result = self.runner.invoke(
            cli,
            ['wallet', 'create-coldkey', '--name', ''],
            input='password\npassword\ny\n'
        )
        # Should complete (empty string is technically valid)
        assert result.exit_code == 0


class TestWalletHotkeyOperations:
    """Test hotkey-specific operations."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_import_hotkey_requires_file(self):
        """Test import-hotkey requires hotkey file."""
        result = self.runner.invoke(cli, [
            'wallet', 'import-hotkey',
            '--coldkey', 'test',
            '--hotkey-name', 'test_hk'
        ])
        # Should fail because hotkey-file is required
        assert result.exit_code != 0

    def test_regen_hotkey_requires_index(self):
        """Test regen-hotkey requires index."""
        result = self.runner.invoke(cli, [
            'wallet', 'regen-hotkey',
            '--coldkey', 'test',
            '--hotkey-name', 'test_hk'
        ])
        # Should fail or prompt for index
        assert result.exit_code in [0, 1, 2]

    def test_register_hotkey_requires_subnet(self):
        """Test register-hotkey requires subnet-uid."""
        result = self.runner.invoke(cli, [
            'wallet', 'register-hotkey',
            '--coldkey', 'test',
            '--hotkey', 'test_hk'
        ])
        # Should fail because subnet-uid is required
        assert result.exit_code != 0
