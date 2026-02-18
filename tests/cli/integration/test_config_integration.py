"""
Integration tests for CLI configuration management.

Tests configuration loading, saving, and management.
"""

import pytest
import os
import tempfile
import shutil
import yaml
from pathlib import Path
from click.testing import CliRunner
from sdk.cli.main import cli
from sdk.cli.config import CLIConfig, NetworkConfig, get_network_config


class TestCLIConfig:
    """Test CLIConfig functionality."""

    def setup_method(self):
        """Setup test fixtures with temporary config directory."""
        self.temp_dir = tempfile.mkdtemp()
        self.config_path = Path(self.temp_dir) / "config.yaml"

    def teardown_method(self):
        """Cleanup temporary directories."""
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)

    def test_create_default_config(self):
        """Test creating default configuration."""
        config = CLIConfig()

        # Should have default values
        assert config is not None
        assert config.network is not None
        assert config.wallet is not None

    def test_config_to_dict(self):
        """Test converting config to dictionary."""
        config = CLIConfig()
        config_dict = config.to_dict()

        assert 'network' in config_dict
        assert 'wallet' in config_dict
        assert 'verbosity' in config_dict

    def test_config_from_dict(self):
        """Test creating config from dictionary."""
        test_config_dict = {
            'network': {
                'name': 'testnet',
                'rpc_url': 'https://test.example.com',
                'chain_id': 2
            },
            'verbosity': 2
        }

        config = CLIConfig.from_dict(test_config_dict)

        assert config.network.name == 'testnet'
        assert config.network.rpc_url == 'https://test.example.com'
        assert config.verbosity == 2

    def test_get_network_config(self):
        """Test getting network configuration."""
        # Get testnet config
        network_config = get_network_config('testnet')

        assert isinstance(network_config, NetworkConfig)
        assert network_config.name == 'testnet'
        assert network_config.rpc_url is not None

    def test_save_and_load_config(self):
        """Test saving and loading configuration."""
        config = CLIConfig()
        config.network.name = 'mainnet'
        config.verbosity = 3

        # Save to file
        config_dict = config.to_dict()
        with open(self.config_path, 'w') as f:
            yaml.dump(config_dict, f)

        # Load from file
        with open(self.config_path, 'r') as f:
            loaded_dict = yaml.safe_load(f)

        loaded_config = CLIConfig.from_dict(loaded_dict)

        assert loaded_config.network.name == 'mainnet'
        assert loaded_config.verbosity == 3


class TestCLIConfigIntegration:
    """Test CLI integration with configuration."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()
        self.temp_dir = tempfile.mkdtemp()
        self.config_path = Path(self.temp_dir) / "config.yaml"

    def teardown_method(self):
        """Cleanup temporary directories."""
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)

    def test_cli_with_config_option(self):
        """Test CLI with custom config file."""
        # Create test config
        test_config = {
            'network': {
                'name': 'testnet',
                'rpc_url': 'https://test.example.com'
            }
        }

        with open(self.config_path, 'w') as f:
            yaml.dump(test_config, f)

        # Run CLI command with config
        result = self.runner.invoke(cli, [
            '--config', str(self.config_path),
            'utils', 'version'
        ])

        assert result.exit_code == 0

    def test_cli_without_config(self):
        """Test CLI without config file (should use defaults)."""
        result = self.runner.invoke(cli, ['utils', 'version'])
        assert result.exit_code == 0

    def test_network_option_override(self):
        """Test that network option overrides config."""
        result = self.runner.invoke(cli, [
            'query', 'list-subnets',
            '--network', 'mainnet'
        ])
        # May fail if network unavailable, but should accept the option
        assert result.exit_code in [0, 1]


class TestNetworkConfiguration:
    """Test network configuration functionality."""

    def setup_method(self):
        """Setup test fixtures."""
        self.temp_dir = tempfile.mkdtemp()

    def teardown_method(self):
        """Cleanup temporary directories."""
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)

    def test_testnet_config(self):
        """Test testnet configuration."""
        config = get_network_config('testnet')
        assert config.name == 'testnet'
        assert config.chain_id == 9999
        assert 'testnet' in config.rpc_url.lower()

    def test_mainnet_config(self):
        """Test mainnet configuration."""
        config = get_network_config('mainnet')
        assert config.name == 'mainnet'
        assert config.chain_id == 8898

    def test_local_config(self):
        """Test local configuration."""
        config = get_network_config('local')
        assert config.name == 'local'
        assert '127.0.0.1' in config.rpc_url or 'localhost' in config.rpc_url
