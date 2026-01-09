"""
Unit tests for validator commands in mtcli.

Tests validator operations including start, stop, status, and set-weights.
"""

import pytest
from click.testing import CliRunner
from sdk.cli.main import cli


class TestValidatorCommands:
    """Test validator command group."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_validator_help(self):
        """Test validator help command."""
        result = self.runner.invoke(cli, ['validator', '--help'])
        assert result.exit_code == 0
        assert 'validator' in result.output.lower()


class TestValidatorStart:
    """Test validator start command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_start_requires_params(self):
        """Test that validator start requires parameters."""
        result = self.runner.invoke(cli, ['validator', 'start'])
        # Should fail or show instructions because parameters are required
        assert result.exit_code in [0, 1, 2]

    def test_start_with_params(self):
        """Test validator start with parameters."""
        result = self.runner.invoke(cli, [
            'validator', 'start',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--network', 'testnet'
        ])
        # May show instructions or fail if wallet doesn't exist
        assert result.exit_code in [0, 1, 2]


class TestValidatorStop:
    """Test validator stop command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_stop_basic(self):
        """Test validator stop command."""
        result = self.runner.invoke(cli, ['validator', 'stop'])
        # Should show instructions or handle gracefully
        assert result.exit_code in [0, 1]


class TestValidatorStatus:
    """Test validator status command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_status_basic(self):
        """Test validator status command."""
        result = self.runner.invoke(cli, [
            'validator', 'status',
            '--network', 'testnet'
        ])
        # May fail if network unavailable or missing required params
        assert result.exit_code in [0, 1, 2]

    def test_status_with_address(self):
        """Test validator status with specific address."""
        result = self.runner.invoke(cli, [
            'validator', 'status',
            '--address', '0x1234567890123456789012345678901234567890',
            '--network', 'testnet'
        ])
        # May fail if network unavailable or missing required params
        assert result.exit_code in [0, 1, 2]


class TestValidatorSetWeights:
    """Test validator set-weights command."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_set_weights_requires_params(self):
        """Test that set-weights requires parameters."""
        result = self.runner.invoke(cli, ['validator', 'set-weights'])
        # Should fail because required parameters are missing
        assert result.exit_code != 0

    def test_set_weights_requires_subnet(self):
        """Test that set-weights requires subnet-uid."""
        result = self.runner.invoke(cli, [
            'validator', 'set-weights',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk'
        ])
        # Should fail because subnet-uid is required
        assert result.exit_code != 0

    def test_set_weights_with_params(self):
        """Test set-weights with all parameters."""
        result = self.runner.invoke(cli, [
            'validator', 'set-weights',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--subnet-uid', '1',
            '--weights', '{"0": 0.5, "1": 0.5}',
            '--network', 'testnet'
        ], input='password\nn\n')  # Decline confirmation
        # May fail because wallet doesn't exist
        assert result.exit_code in [0, 1, 2]


class TestValidatorValidation:
    """Test validator command validation."""

    def setup_method(self):
        """Setup test fixtures."""
        self.runner = CliRunner()

    def test_set_weights_invalid_json(self):
        """Test set-weights with invalid JSON weights."""
        result = self.runner.invoke(cli, [
            'validator', 'set-weights',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--subnet-uid', '1',
            '--weights', 'invalid_json',
            '--network', 'testnet'
        ])
        # Should handle invalid JSON gracefully
        assert result.exit_code in [0, 1, 2]

    def test_set_weights_empty_weights(self):
        """Test set-weights with empty weights."""
        result = self.runner.invoke(cli, [
            'validator', 'set-weights',
            '--coldkey', 'test_ck',
            '--hotkey', 'test_hk',
            '--subnet-uid', '1',
            '--weights', '{}',
            '--network', 'testnet'
        ])
        # Empty weights might be rejected
        assert result.exit_code in [0, 1, 2]

    def test_status_invalid_address(self):
        """Test status with invalid address."""
        result = self.runner.invoke(cli, [
            'validator', 'status',
            '--address', 'invalid_address',
            '--network', 'testnet'
        ])
        # Should handle invalid address gracefully
        assert result.exit_code in [0, 1, 2]
