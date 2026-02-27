"""
ModernTensor Adaptive Tokenomics Module.

This module implements adaptive tokenomics for ModernTensor,
providing superior features compared to Bittensor's fixed emission model.

Key Features:
- Adaptive emission based on network utility
- Token recycling from fees and penalties
- Burn mechanisms for deflationary pressure
- Fair reward distribution with Merkle proofs
- Tight integration with Layer 1 blockchain

Components:
- EmissionController: Manages adaptive token emission
- RecyclingPool: Handles token recycling
- BurnManager: Manages token burning
- RewardDistributor: Distributes rewards to miners/validators/DAO
- ClaimManager: Manages reward claims with Merkle proofs
- TokenomicsIntegration: Integrates with Layer 1 consensus
"""

from .config import TokenomicsConfig, DistributionConfig
from .emission_controller import EmissionController
from .recycling_pool import RecyclingPool
from .burn_manager import BurnManager
from .reward_distributor import RewardDistributor, DistributionResult
from .claim_manager import ClaimManager
from .integration import TokenomicsIntegration, EpochTokenomics, ConsensusData
from .metrics_collector import NetworkMetricsCollector, NetworkMetrics

__all__ = [
    'TokenomicsConfig',
    'DistributionConfig',
    'EmissionController',
    'RecyclingPool',
    'BurnManager',
    'RewardDistributor',
    'DistributionResult',
    'ClaimManager',
    'TokenomicsIntegration',
    'EpochTokenomics',
    'ConsensusData',
    'NetworkMetricsCollector',
    'NetworkMetrics',
]
