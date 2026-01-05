# tests/metagraph/test_aggregated_state.py
"""
Tests for SubnetAggregatedDatum and SubnetAggregatedStateManager.
"""

import pytest
from sdk.metagraph.aggregated_state import (
    SubnetAggregatedDatum,
    SubnetAggregatedStateManager,
    DATUM_INT_DIVISOR
)


class TestSubnetAggregatedDatum:
    """Test SubnetAggregatedDatum structure."""
    
    def test_create_empty_state(self):
        """Test creating an empty aggregated state."""
        subnet_uid = 1
        current_slot = 1000
        
        state = SubnetAggregatedDatum.create_empty(subnet_uid, current_slot)
        
        assert state.subnet_uid == subnet_uid
        assert state.current_epoch == 0
        assert state.total_miners == 0
        assert state.total_validators == 0
        assert state.active_miners == 0
        assert state.active_validators == 0
        assert state.total_stake == 0
        assert state.last_update_slot == current_slot
    
    def test_performance_properties(self):
        """Test performance property conversions."""
        state = SubnetAggregatedDatum.create_empty(1, 1000)
        
        # Set scaled values
        state.scaled_avg_miner_performance = int(0.85 * DATUM_INT_DIVISOR)
        state.scaled_avg_validator_performance = int(0.92 * DATUM_INT_DIVISOR)
        state.scaled_subnet_performance = int(0.88 * DATUM_INT_DIVISOR)
        
        # Check float conversions
        assert abs(state.avg_miner_performance - 0.85) < 0.01
        assert abs(state.avg_validator_performance - 0.92) < 0.01
        assert abs(state.subnet_performance - 0.88) < 0.01
    
    def test_to_dict(self):
        """Test conversion to dictionary."""
        state = SubnetAggregatedDatum.create_empty(1, 1000)
        state.total_miners = 10
        state.total_validators = 5
        state.total_stake = 1000000
        
        state_dict = state.to_dict()
        
        assert isinstance(state_dict, dict)
        assert state_dict['subnet_uid'] == 1
        assert state_dict['total_miners'] == 10
        assert state_dict['total_validators'] == 5
        assert state_dict['total_stake'] == 1000000
        assert 'weight_matrix_hash' in state_dict
        assert 'consensus_scores_root' in state_dict


class TestSubnetAggregatedStateManager:
    """Test SubnetAggregatedStateManager."""
    
    def test_create_subnet_state(self):
        """Test creating a subnet state."""
        manager = SubnetAggregatedStateManager()
        
        state = manager.create_subnet_state(subnet_uid=1, current_slot=1000)
        
        assert state.subnet_uid == 1
        assert state.current_epoch == 0
        assert state.last_update_slot == 1000
        assert manager.get_state(1) == state
    
    def test_update_participant_counts(self):
        """Test updating participant counts."""
        manager = SubnetAggregatedStateManager()
        manager.create_subnet_state(subnet_uid=1, current_slot=1000)
        
        manager.update_participant_counts(
            subnet_uid=1,
            total_miners=20,
            total_validators=10,
            active_miners=18,
            active_validators=9
        )
        
        state = manager.get_state(1)
        assert state.total_miners == 20
        assert state.total_validators == 10
        assert state.active_miners == 18
        assert state.active_validators == 9
    
    def test_update_economic_metrics(self):
        """Test updating economic metrics."""
        manager = SubnetAggregatedStateManager()
        manager.create_subnet_state(subnet_uid=1, current_slot=1000)
        
        manager.update_economic_metrics(
            subnet_uid=1,
            total_stake=5000000,
            miner_stake=3000000,
            validator_stake=2000000
        )
        
        state = manager.get_state(1)
        assert state.total_stake == 5000000
        assert state.total_miner_stake == 3000000
        assert state.total_validator_stake == 2000000
    
    def test_update_consensus_data(self):
        """Test updating consensus data."""
        manager = SubnetAggregatedStateManager()
        manager.create_subnet_state(subnet_uid=1, current_slot=1000)
        
        weight_hash = b'\x01' * 32
        consensus_root = b'\x02' * 32
        emission_root = b'\x03' * 32
        
        manager.update_consensus_data(
            subnet_uid=1,
            weight_matrix_hash=weight_hash,
            consensus_scores_root=consensus_root,
            emission_schedule_root=emission_root,
            current_slot=1100
        )
        
        state = manager.get_state(1)
        assert state.weight_matrix_hash == weight_hash
        assert state.consensus_scores_root == consensus_root
        assert state.emission_schedule_root == emission_root
        assert state.last_consensus_slot == 1100
    
    def test_update_performance_metrics(self):
        """Test updating performance metrics."""
        manager = SubnetAggregatedStateManager()
        manager.create_subnet_state(subnet_uid=1, current_slot=1000)
        
        manager.update_performance_metrics(
            subnet_uid=1,
            avg_miner_performance=0.87,
            avg_validator_performance=0.93,
            subnet_performance=0.90
        )
        
        state = manager.get_state(1)
        assert abs(state.avg_miner_performance - 0.87) < 0.01
        assert abs(state.avg_validator_performance - 0.93) < 0.01
        assert abs(state.subnet_performance - 0.90) < 0.01
    
    def test_update_emission_data(self):
        """Test updating emission data."""
        manager = SubnetAggregatedStateManager()
        manager.create_subnet_state(subnet_uid=1, current_slot=1000)
        
        manager.update_emission_data(
            subnet_uid=1,
            total_emission=100000,
            miner_pool=60000,
            validator_pool=40000,
            current_slot=1200
        )
        
        state = manager.get_state(1)
        assert state.total_emission_this_epoch == 100000
        assert state.miner_reward_pool == 60000
        assert state.validator_reward_pool == 40000
        assert state.last_emission_slot == 1200
    
    def test_increment_epoch(self):
        """Test incrementing epoch."""
        manager = SubnetAggregatedStateManager()
        manager.create_subnet_state(subnet_uid=1, current_slot=1000)
        
        initial_epoch = manager.get_state(1).current_epoch
        
        manager.increment_epoch(subnet_uid=1, current_slot=1500)
        
        state = manager.get_state(1)
        assert state.current_epoch == initial_epoch + 1
        assert state.last_update_slot == 1500
    
    def test_calculate_aggregated_metrics(self):
        """Test calculating aggregated metrics from participants."""
        manager = SubnetAggregatedStateManager()
        
        # Mock miners and validators with performance
        class MockParticipant:
            def __init__(self, perf):
                self.last_performance = perf
        
        miners = [MockParticipant(0.8), MockParticipant(0.9), MockParticipant(0.85)]
        validators = [MockParticipant(0.95), MockParticipant(0.90)]
        
        avg_miner, avg_validator, subnet = manager.calculate_aggregated_metrics(
            miners, validators
        )
        
        # Check averages
        assert abs(avg_miner - 0.85) < 0.01  # (0.8 + 0.9 + 0.85) / 3
        assert abs(avg_validator - 0.925) < 0.01  # (0.95 + 0.90) / 2
        # Subnet should be weighted average
        expected_subnet = (3 * 0.85 + 2 * 0.925) / 5
        assert abs(subnet - expected_subnet) < 0.01
    
    def test_calculate_aggregated_metrics_empty(self):
        """Test calculating metrics with no participants."""
        manager = SubnetAggregatedStateManager()
        
        avg_miner, avg_validator, subnet = manager.calculate_aggregated_metrics(
            [], []
        )
        
        assert avg_miner == 0.0
        assert avg_validator == 0.0
        assert subnet == 0.0
    
    def test_invalid_subnet_uid(self):
        """Test operations with invalid subnet UID."""
        manager = SubnetAggregatedStateManager()
        manager.create_subnet_state(subnet_uid=1, current_slot=1000)
        
        with pytest.raises(ValueError):
            manager.update_participant_counts(
                subnet_uid=999,  # Non-existent
                total_miners=10,
                total_validators=5,
                active_miners=10,
                active_validators=5
            )
    
    def test_get_nonexistent_state(self):
        """Test getting a non-existent state."""
        manager = SubnetAggregatedStateManager()
        
        state = manager.get_state(999)
        assert state is None


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
