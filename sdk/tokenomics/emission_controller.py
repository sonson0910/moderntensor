"""
Emission Controller for adaptive token emission.

Implements ModernTensor's adaptive emission model:
- Utility-based minting (vs fixed emission)
- Minimum emission floor (prevents death spiral)
- Quality multiplier
- Halving schedule

This provides a superior alternative to Bittensor's fixed 7,200 TAO/day.
"""

from typing import Optional
from sdk.tokenomics.config import TokenomicsConfig, DEFAULT_TOKENOMICS_CONFIG
import math


class EmissionController:
    """
    Manages adaptive token emission based on network utility.

    Core Formula:
        MintAmount = max(
            BaseReward × UtilityScore × QualityMultiplier × HalvingFactor,
            MinEmissionFloor × HalvingFactor
        )

    Where:
        - BaseReward: Base reward per epoch (1000 MDT, decreases via halving)
        - UtilityScore: 0.0 to 1.0 based on network activity
        - QualityMultiplier: 0.6 to 1.4 based on consensus quality
        - HalvingFactor: 0.5^n where n = number of halvings
        - MinEmissionFloor: 100 MDT/day minimum (also decreases via halving)
    """

    def __init__(self, config: Optional[TokenomicsConfig] = None):
        """
        Initialize emission controller.

        Args:
            config: Tokenomics configuration (uses defaults if not provided)
        """
        self.config = config or DEFAULT_TOKENOMICS_CONFIG
        self.current_supply = 0
        self.total_burned = 0

    def calculate_epoch_emission(
        self,
        utility_score: float,
        epoch: int,
        quality_score: float = 1.0
    ) -> int:
        """
        Calculate emission for current epoch with minimum floor.

        Args:
            utility_score: Network utility score (0.0 to 1.0)
            epoch: Current epoch number
            quality_score: Consensus quality score (0.0 to 1.0)

        Returns:
            Token amount to mint this epoch
        """
        # Validate inputs
        if not 0.0 <= utility_score <= 1.0:
            raise ValueError(f"Utility score must be between 0.0 and 1.0, got {utility_score}")
        if not 0.0 <= quality_score <= 1.0:
            raise ValueError(f"Quality score must be between 0.0 and 1.0, got {quality_score}")

        # Calculate halving multiplier
        halvings = epoch // self.config.halving_interval
        halving_factor = 0.5 ** halvings

        # Calculate quality multiplier (0.6 to 1.4)
        quality_multiplier = self._calculate_quality_multiplier(quality_score)

        # Calculate adaptive emission
        calculated_emission = (
            self.config.base_reward *
            utility_score *
            quality_multiplier *
            halving_factor
        )

        # Apply minimum emission floor (also subject to halving)
        min_emission = self.config.min_daily_emission * halving_factor

        # Final emission = max(calculated, floor)
        mint_amount = max(calculated_emission, min_emission)

        # Cap at max supply
        remaining = self.config.max_supply - self.current_supply
        if mint_amount > remaining:
            mint_amount = max(0, remaining)

        return int(mint_amount)

    def _calculate_quality_multiplier(self, quality_score: float) -> float:
        """
        Calculate quality multiplier using sigmoid function.

        Args:
            quality_score: Consensus quality (0-1)

        Returns:
            Multiplier between 0.6 and 1.4
        """
        # Sigmoid centered at 0.5, k=10 for steepness
        k = 10.0
        x = quality_score - 0.5
        sigmoid = 1 / (1 + math.exp(-k * x))

        # Scale to 0.6 - 1.4
        return 0.6 + 0.8 * sigmoid

    def calculate_utility_score(
        self,
        task_volume: int,
        avg_task_difficulty: float,
        validator_participation: float
    ) -> float:
        """
        Calculate network utility score.

        Formula:
            U = α₁ × TaskVolumeScore +
                α₂ × DifficultyScore +
                α₃ × ParticipationScore

        Where α₁ + α₂ + α₃ = 1.0 (default: 0.4, 0.3, 0.3)

        Args:
            task_volume: Number of tasks completed in epoch
            avg_task_difficulty: Average difficulty (0.0 to 1.0)
            validator_participation: Validator participation ratio (0.0 to 1.0)

        Returns:
            Utility score (0.0 to 1.0)
        """
        # Get weights from config
        w1, w2, w3 = self.config.utility_weights

        # Normalize task volume (0-1 scale)
        max_tasks = self.config.max_expected_tasks
        task_score = min(task_volume / max_tasks, 1.0) if max_tasks > 0 else 0.0

        # Validate inputs
        if not 0.0 <= avg_task_difficulty <= 1.0:
            raise ValueError(f"Task difficulty must be between 0.0 and 1.0, got {avg_task_difficulty}")
        if not 0.0 <= validator_participation <= 1.0:
            raise ValueError(f"Validator participation must be between 0.0 and 1.0, got {validator_participation}")

        # Calculate weighted utility
        utility = (
            w1 * task_score +
            w2 * avg_task_difficulty +
            w3 * validator_participation
        )

        return min(utility, 1.0)

    def update_supply(self, amount: int) -> None:
        """
        Update current supply after minting.

        Args:
            amount: Amount minted
        """
        if amount < 0:
            raise ValueError(f"Amount must be non-negative, got {amount}")

        self.current_supply += amount
        if self.current_supply > self.config.max_supply:
            self.current_supply = self.config.max_supply

    def record_burn(self, amount: int) -> None:
        """
        Record tokens burned.

        Args:
            amount: Amount burned
        """
        if amount < 0:
            raise ValueError(f"Amount must be non-negative, got {amount}")

        self.total_burned += amount
        # Note: burned tokens reduce circulating supply but not max supply

    def get_supply_info(self) -> dict:
        """
        Get comprehensive supply information.

        Returns:
            Dictionary with supply metrics
        """
        circulating = self.current_supply - self.total_burned
        remaining = self.config.max_supply - self.current_supply

        return {
            'current_supply': self.current_supply,
            'max_supply': self.config.max_supply,
            'remaining_supply': remaining,
            'total_burned': self.total_burned,
            'circulating_supply': max(0, circulating),
            'supply_percentage': (self.current_supply / self.config.max_supply * 100) if self.config.max_supply > 0 else 0,
            'burn_percentage': (self.total_burned / self.current_supply * 100) if self.current_supply > 0 else 0
        }

    def get_emission_projection(self, epochs: int = 365 * 6 * 24) -> dict:
        """
        Project emission over time.

        Args:
            epochs: Number of epochs to project (default: 1 year at 10min/epoch)

        Returns:
            Projection data
        """
        projections = []\n        _ = self.current_supply  # Reference current supply for future use

        # Sample key epochs
        sample_epochs = [0, epochs // 4, epochs // 2, epochs * 3 // 4, epochs]

        for epoch in sample_epochs:
            halvings = epoch // self.config.halving_interval
            halving_factor = 0.5 ** halvings

            max_emission = self.config.base_reward * halving_factor
            min_emission = self.config.min_daily_emission * halving_factor

            projections.append({
                'epoch': epoch,
                'halvings': halvings,
                'max_daily_emission': max_emission,
                'min_daily_emission': min_emission,
            })

        return {
            'projections': projections,
            'total_epochs': epochs,
            'halving_interval': self.config.halving_interval
        }

    def compare_with_bittensor(self, epoch: int, utility_score: float) -> dict:
        """
        Compare ModernTensor emission with Bittensor's fixed model.

        Args:
            epoch: Current epoch
            utility_score: Current utility score

        Returns:
            Comparison data
        """
        mdt_emission = self.calculate_epoch_emission(utility_score, epoch)
        tao_daily = 7200  # Bittensor's fixed daily emission

        # Account for Bittensor halvings (at 10.5M supply)
        # Simplified: assume first halving reduces to 3600
        if epoch > 210_000 * 2:  # ~4 years
            tao_daily = 3600

        reduction = ((tao_daily - mdt_emission) / tao_daily) * 100 if tao_daily > 0 else 0

        return {
            'mdt_emission': mdt_emission,
            'tao_emission': tao_daily,
            'reduction_percentage': max(0, reduction),
            'mdt_is_lower': mdt_emission < tao_daily
        }
