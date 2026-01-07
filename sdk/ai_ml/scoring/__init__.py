"""
Advanced scoring and consensus - Surpassing Bittensor.

Features that Bittensor doesn't have:
- Multi-criteria scoring
- Ensemble scoring methods
- Advanced consensus algorithms
- Reward model integration
"""

from .advanced_scorer import AdvancedScorer, ScoringMethod
from .consensus import ConsensusAggregator, ConsensusMethod, ValidatorScore

__all__ = [
    "AdvancedScorer",
    "ScoringMethod",
    "ConsensusAggregator",
    "ConsensusMethod",
    "ValidatorScore",
]
