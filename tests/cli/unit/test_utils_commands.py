"""
Unit tests for utility commands in mtcli.

Tests the utility commands including convert, version, and generate-keypair.
"""

import pytest
from click.testing import CliRunner
from sdk.cli.main import cli


class TestConvertCommand:
    """Test the convert utility command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_convert_from_mdt(self):
        """Test converting from MDT to base units."""
        result = self.runner.invoke(cli, ['utils', 'convert', '--from-mdt', '1.5'])
        assert result.exit_code == 0
        assert 'MDT' in result.output
        # 1.5 MDT = 1.5 * 10^9 base units (9 decimals)
        expected_base = 1500000000
        assert str(expected_base) in result.output

    def test_convert_from_base(self):
        """Test converting from base units to MDT."""
        result = self.runner.invoke(cli, ['utils', 'convert', '--from-base', '1000000000'])
        assert result.exit_code == 0
        assert 'MDT' in result.output
        assert '1.0' in result.output or '1 ' in result.output

    def test_convert_no_args(self):
        """Test convert without arguments shows error."""
        result = self.runner.invoke(cli, ['utils', 'convert'])
        assert '❌' in result.output or 'Specify' in result.output

    def test_convert_both_args(self):
        """Test convert with both arguments."""
        result = self.runner.invoke(cli, ['utils', 'convert', '--from-mdt', '1', '--from-base', '1000000000'])
        assert result.exit_code == 0
        # Should display both conversions


class TestVersionCommand:
    """Test the version utility command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_version_flag(self):
        """Test version flag on main CLI."""
        result = self.runner.invoke(cli, ['--version'])
        assert result.exit_code == 0
        assert 'mtcli version' in result.output
        assert 'ModernTensor' in result.output

    def test_version_command(self):
        """Test version subcommand."""
        result = self.runner.invoke(cli, ['utils', 'version'])
        assert result.exit_code == 0
        assert 'mtcli' in result.output or 'version' in result.output


class TestGenerateKeypairCommand:
    """Test the generate-keypair utility command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_generate_keypair(self):
        """Test keypair generation."""
        result = self.runner.invoke(cli, ['utils', 'generate-keypair'])
        assert result.exit_code == 0
        # Should show public and private keys or addresses
        assert 'key' in result.output.lower() or 'address' in result.output.lower()


class TestLatencyCommand:
    """Test the latency utility command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_latency_basic(self):
        """Test basic latency command."""
        # This might fail if network is not available, but should not crash
        result = self.runner.invoke(cli, ['utils', 'latency', '--count', '1'])
        # Command should exit gracefully even if network fails
        assert result.exit_code in [0, 1]

    def test_latency_with_network(self):
        """Test latency with specific network."""
        result = self.runner.invoke(cli, ['utils', 'latency', '--network', 'testnet', '--count', '1'])
        assert result.exit_code in [0, 1]


class TestUtilsHelp:
    """Test utils command help."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_utils_help(self):
        """Test utils help command."""
        result = self.runner.invoke(cli, ['utils', '--help'])
        assert result.exit_code == 0
        assert 'Utility commands' in result.output or 'commands' in result.output
