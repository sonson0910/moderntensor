"""
Unit tests for tokenomics configuration.
"""

import pytest
from sdk.tokenomics.config import TokenomicsConfig, DistributionConfig


class TestTokenomicsConfig:
    """Test cases for TokenomicsConfig."""
    
    def test_default_config(self):
        """Test default configuration values."""
        config = TokenomicsConfig()
        
        assert config.max_supply == 21_000_000
        assert config.base_reward == 1000
        assert config.halving_interval == 210_000
        assert config.max_expected_tasks == 10_000
        assert config.utility_weights == (0.5, 0.3, 0.2)
    
    def test_custom_config(self):
        """Test custom configuration values."""
        config = TokenomicsConfig(
            max_supply=100_000_000,
            base_reward=500,
            halving_interval=100_000,
            max_expected_tasks=5_000,
            utility_weights=(0.4, 0.4, 0.2)
        )
        
        assert config.max_supply == 100_000_000
        assert config.base_reward == 500
        assert config.halving_interval == 100_000
        assert config.max_expected_tasks == 5_000
        assert config.utility_weights == (0.4, 0.4, 0.2)
    
    def test_utility_weights_validation(self):
        """Test that utility weights must sum to 1.0."""
        with pytest.raises(ValueError, match="Utility weights must sum to 1.0"):
            TokenomicsConfig(utility_weights=(0.5, 0.3, 0.3))


class TestDistributionConfig:
    """Test cases for DistributionConfig."""
    
    def test_default_config(self):
        """Test default distribution values."""
        config = DistributionConfig()
        
        assert config.miner_share == 0.40
        assert config.validator_share == 0.40
        assert config.dao_share == 0.20
    
    def test_custom_config(self):
        """Test custom distribution values."""
        config = DistributionConfig(
            miner_share=0.50,
            validator_share=0.30,
            dao_share=0.20
        )
        
        assert config.miner_share == 0.50
        assert config.validator_share == 0.30
        assert config.dao_share == 0.20
    
    def test_distribution_validation(self):
        """Test that distribution shares must sum to 1.0."""
        with pytest.raises(ValueError, match="Distribution shares must sum to 1.0"):
            DistributionConfig(
                miner_share=0.50,
                validator_share=0.40,
                dao_share=0.30
            )
