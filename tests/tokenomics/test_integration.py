"""
Integration test for complete tokenomics cycle.

This test demonstrates the full tokenomics workflow integrated with
Layer 1 consensus.
"""

import pytest
from sdk.tokenomics import (
    TokenomicsIntegration,
    ConsensusData,
    NetworkMetrics,
    TokenomicsConfig,
    DistributionConfig
)


class TestTokenomicsIntegration:
    """Integration tests for complete tokenomics system."""
    
    def test_full_epoch_cycle(self):
        """Test complete epoch tokenomics processing."""
        # Setup
        tokenomics = TokenomicsIntegration()
        
        # Simulate epoch 0 with good network activity
        consensus_data = ConsensusData(
            miner_scores={
                'miner1': 0.8,
                'miner2': 0.6,
                'miner3': 0.4
            },
            validator_stakes={
                'val1': 10000,
                'val2': 5000
            },
            quality_score=0.9
        )
        
        network_metrics = NetworkMetrics(
            task_count=5000,
            total_difficulty=4000,
            avg_difficulty=0.8,
            active_validators=2,
            total_validators=2,
            validator_ratio=1.0
        )
        
        # Process epoch
        result = tokenomics.process_epoch_tokenomics(
            epoch=0,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        # Verify results
        assert result.epoch == 0
        assert 0 < result.utility_score <= 1.0
        assert result.emission_amount > 0
        assert result.burned_amount == 0  # Good quality, no burn
        assert result.miner_pool > 0
        assert result.validator_pool > 0
        assert result.dao_allocation > 0
        assert result.claim_root != b'\x00' * 32
        
        # Verify distribution adds up (allow for rounding differences)
        total = result.miner_pool + result.validator_pool + result.dao_allocation
        assert abs(total - result.emission_amount) <= 1  # Allow 1 token rounding difference
    
    def test_multiple_epochs(self):
        """Test processing multiple epochs."""
        tokenomics = TokenomicsIntegration()
        
        for epoch in range(5):
            consensus_data = ConsensusData(
                miner_scores={'miner1': 0.8},
                validator_stakes={'val1': 10000},
                quality_score=0.9
            )
            
            network_metrics = NetworkMetrics(
                task_count=1000 * (epoch + 1),
                avg_difficulty=0.5,
                validator_ratio=1.0
            )
            
            result = tokenomics.process_epoch_tokenomics(
                epoch=epoch,
                consensus_data=consensus_data,
                network_metrics=network_metrics
            )
            
            assert result.epoch == epoch
            assert result.emission_amount > 0
    
    def test_low_quality_burns_tokens(self):
        """Test that low quality triggers token burning."""
        tokenomics = TokenomicsIntegration()
        
        # Low quality consensus
        consensus_data = ConsensusData(
            miner_scores={'miner1': 0.2},
            validator_stakes={'val1': 10000},
            quality_score=0.3  # Below 0.5 threshold
        )
        
        network_metrics = NetworkMetrics(
            task_count=1000,
            avg_difficulty=0.5,
            validator_ratio=1.0
        )
        
        result = tokenomics.process_epoch_tokenomics(
            epoch=0,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        # Should have burned tokens
        assert result.burned_amount > 0
    
    def test_recycling_pool_usage(self):
        """Test that recycling pool is used before minting."""
        tokenomics = TokenomicsIntegration()
        
        # Add tokens to recycling pool
        tokenomics.add_to_recycling_pool(5000, 'registration_fees')
        
        consensus_data = ConsensusData(
            miner_scores={'miner1': 0.8},
            validator_stakes={'val1': 10000},
            quality_score=0.9
        )
        
        network_metrics = NetworkMetrics(
            task_count=1000,
            avg_difficulty=0.5,
            validator_ratio=1.0
        )
        
        result = tokenomics.process_epoch_tokenomics(
            epoch=0,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        # Should use pool first
        assert result.from_pool > 0
        # May or may not need minting depending on emission amount
    
    def test_claim_proof_generation(self):
        """Test claim proof generation and verification."""
        tokenomics = TokenomicsIntegration()
        
        consensus_data = ConsensusData(
            miner_scores={
                'miner1': 0.8,
                'miner2': 0.6
            },
            validator_stakes={
                'val1': 10000
            },
            quality_score=0.9
        )
        
        network_metrics = NetworkMetrics(
            task_count=1000,
            avg_difficulty=0.5,
            validator_ratio=1.0
        )
        
        result = tokenomics.process_epoch_tokenomics(
            epoch=0,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        # Get proof for miner1
        proof = tokenomics.get_claim_proof(0, 'miner1')
        assert proof is not None
        
        # Verify we can check claim status
        status = tokenomics.claims.get_claim_status(0, 'miner1')
        assert status['exists'] is True
        assert status['reward'] > 0
        assert status['claimed'] is False
    
    def test_custom_config(self):
        """Test with custom tokenomics configuration."""
        config = TokenomicsConfig(
            max_supply=100_000,
            base_reward=100,
            halving_interval=1000
        )
        
        dist_config = DistributionConfig(
            miner_share=0.50,
            validator_share=0.30,
            dao_share=0.20
        )
        
        tokenomics = TokenomicsIntegration(
            tokenomics_config=config,
            distribution_config=dist_config
        )
        
        consensus_data = ConsensusData(
            miner_scores={'miner1': 1.0},
            validator_stakes={'val1': 10000},
            quality_score=1.0
        )
        
        network_metrics = NetworkMetrics(
            task_count=10000,
            avg_difficulty=1.0,
            validator_ratio=1.0
        )
        
        result = tokenomics.process_epoch_tokenomics(
            epoch=0,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        # Custom base reward with full utility
        assert result.emission_amount == 100
        
        # Custom distribution split
        assert result.miner_pool == 50
        assert result.validator_pool == 30
        assert result.dao_allocation == 20
    
    def test_get_stats(self):
        """Test getting comprehensive statistics."""
        tokenomics = TokenomicsIntegration()
        
        # Process an epoch
        consensus_data = ConsensusData(
            miner_scores={'miner1': 0.8},
            validator_stakes={'val1': 10000},
            quality_score=0.9
        )
        
        network_metrics = NetworkMetrics(
            task_count=1000,
            avg_difficulty=0.5,
            validator_ratio=1.0
        )
        
        tokenomics.process_epoch_tokenomics(
            epoch=0,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        # Get stats
        stats = tokenomics.get_stats()
        
        assert 'supply' in stats
        assert 'recycling_pool' in stats
        assert 'burns' in stats
        
        # Verify supply info
        assert stats['supply']['current_supply'] >= 0
        assert stats['supply']['max_supply'] == 21_000_000
