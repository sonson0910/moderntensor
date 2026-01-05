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

from sdk.tokenomics.config import TokenomicsConfig, DistributionConfig
from sdk.tokenomics.emission_controller import EmissionController
from sdk.tokenomics.recycling_pool import RecyclingPool
from sdk.tokenomics.burn_manager import BurnManager
from sdk.tokenomics.reward_distributor import RewardDistributor, DistributionResult
from sdk.tokenomics.claim_manager import ClaimManager
from sdk.tokenomics.integration import TokenomicsIntegration, EpochTokenomics, ConsensusData
from sdk.tokenomics.metrics_collector import NetworkMetricsCollector, NetworkMetrics

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
