"""
Unit tests for EmissionController.
"""

import pytest
from sdk.tokenomics.emission_controller import EmissionController
from sdk.tokenomics.config import TokenomicsConfig


class TestEmissionController:
    """Test cases for EmissionController."""
    
    def test_initialization(self):
        """Test controller initialization."""
        controller = EmissionController()
        
        assert controller.current_supply == 0
        assert controller.config.max_supply == 21_000_000
    
    def test_calculate_epoch_emission_full_utility(self):
        """Test emission calculation with full utility."""
        controller = EmissionController()
        
        # Epoch 0, full utility
        emission = controller.calculate_epoch_emission(
            utility_score=1.0,
            epoch=0
        )
        
        assert emission == 1000  # base_reward * 1.0 * 1.0
    
    def test_calculate_epoch_emission_half_utility(self):
        """Test emission calculation with half utility."""
        controller = EmissionController()
        
        # Epoch 0, half utility
        emission = controller.calculate_epoch_emission(
            utility_score=0.5,
            epoch=0
        )
        
        assert emission == 500  # base_reward * 0.5 * 1.0
    
    def test_calculate_epoch_emission_with_halving(self):
        """Test emission calculation with halving."""
        controller = EmissionController()
        
        # First halving at epoch 210,000
        emission = controller.calculate_epoch_emission(
            utility_score=1.0,
            epoch=210_000
        )
        
        assert emission == 500  # base_reward * 1.0 * 0.5
    
    def test_calculate_epoch_emission_multiple_halvings(self):
        """Test emission calculation with multiple halvings."""
        controller = EmissionController()
        
        # Second halving at epoch 420,000
        emission = controller.calculate_epoch_emission(
            utility_score=1.0,
            epoch=420_000
        )
        
        assert emission == 250  # base_reward * 1.0 * 0.25
    
    def test_calculate_epoch_emission_max_supply_cap(self):
        """Test that emission is capped at max supply."""
        config = TokenomicsConfig(max_supply=1000)
        controller = EmissionController(config)
        controller.current_supply = 900
        
        emission = controller.calculate_epoch_emission(
            utility_score=1.0,
            epoch=0
        )
        
        assert emission == 100  # Capped at remaining supply
    
    def test_calculate_epoch_emission_invalid_utility(self):
        """Test that invalid utility scores raise error."""
        controller = EmissionController()
        
        with pytest.raises(ValueError, match="Utility score must be between 0.0 and 1.0"):
            controller.calculate_epoch_emission(utility_score=1.5, epoch=0)
        
        with pytest.raises(ValueError, match="Utility score must be between 0.0 and 1.0"):
            controller.calculate_epoch_emission(utility_score=-0.1, epoch=0)
    
    def test_calculate_utility_score(self):
        """Test utility score calculation."""
        controller = EmissionController()
        
        utility = controller.calculate_utility_score(
            task_volume=5000,
            avg_task_difficulty=0.8,
            validator_participation=0.9
        )
        
        # w1=0.5, w2=0.3, w3=0.2
        # (5000/10000)*0.5 + 0.8*0.3 + 0.9*0.2
        expected = 0.5 * 0.5 + 0.3 * 0.8 + 0.2 * 0.9
        assert abs(utility - expected) < 0.001
    
    def test_calculate_utility_score_max_tasks(self):
        """Test utility score with more than max tasks."""
        controller = EmissionController()
        
        utility = controller.calculate_utility_score(
            task_volume=20000,  # More than max_expected_tasks
            avg_task_difficulty=0.5,
            validator_participation=0.5
        )
        
        # Task score should be capped at 1.0
        expected = 0.5 * 1.0 + 0.3 * 0.5 + 0.2 * 0.5
        assert abs(utility - expected) < 0.001
    
    def test_calculate_utility_score_invalid_inputs(self):
        """Test that invalid inputs raise errors."""
        controller = EmissionController()
        
        with pytest.raises(ValueError, match="Task difficulty must be between 0.0 and 1.0"):
            controller.calculate_utility_score(
                task_volume=100,
                avg_task_difficulty=1.5,
                validator_participation=0.5
            )
        
        with pytest.raises(ValueError, match="Validator participation must be between 0.0 and 1.0"):
            controller.calculate_utility_score(
                task_volume=100,
                avg_task_difficulty=0.5,
                validator_participation=1.5
            )
    
    def test_update_supply(self):
        """Test supply update."""
        controller = EmissionController()
        
        controller.update_supply(1000)
        assert controller.current_supply == 1000
        
        controller.update_supply(500)
        assert controller.current_supply == 1500
    
    def test_update_supply_exceeds_max(self):
        """Test that supply cannot exceed max."""
        config = TokenomicsConfig(max_supply=1000)
        controller = EmissionController(config)
        
        controller.update_supply(1500)
        assert controller.current_supply == 1000
    
    def test_get_supply_info(self):
        """Test getting supply information."""
        controller = EmissionController()
        controller.current_supply = 5_000_000
        
        info = controller.get_supply_info()
        
        assert info['current_supply'] == 5_000_000
        assert info['max_supply'] == 21_000_000
        assert info['remaining_supply'] == 16_000_000
        assert abs(info['supply_percentage'] - 23.809523809523807) < 0.001
