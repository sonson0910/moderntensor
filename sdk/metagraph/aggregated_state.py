# sdk/metagraph/aggregated_state.py
"""
Aggregated Subnet State for Layer 1 blockchain.
This module implements the SubnetAggregatedDatum structure that stores
subnet-level aggregated data in a single UTXO instead of individual UTXOs
per miner/validator.

Key benefits:
- Query entire subnet with 1 UTXO instead of scanning N UTXOs
- Reduce gas costs when updating multiple miners simultaneously
- Equivalent to Bittensor's Metagraph but on UTXO model
"""

from pycardano import PlutusData
from dataclasses import dataclass
from typing import List, Optional
import hashlib

# Import settings for divisor
try:
    from sdk.config.settings import settings
    DATUM_INT_DIVISOR = settings.METAGRAPH_DATUM_INT_DIVISOR
except ImportError:
    import logging
    logging.warning("Could not import settings for DATUM_INT_DIVISOR. Using default 1_000_000.0")
    DATUM_INT_DIVISOR = 1_000_000.0


@dataclass
class SubnetAggregatedDatum(PlutusData):
    """
    Aggregated state của cả subnet (1 UTXO).
    
    This structure stores aggregated metrics for an entire subnet,
    reducing the need to query individual miner/validator UTXOs.
    
    Storage strategy:
    - Small aggregated metrics: on-chain (this datum)
    - Large data (weight matrices, detailed scores): off-chain with hash on-chain
    - Historical data: IPFS/Arweave with hash reference
    """
    
    CONSTR_ID = 0  # type: ignore
    
    # Basic subnet identification
    subnet_uid: int
    current_epoch: int
    
    # Aggregated participant counts
    total_miners: int
    total_validators: int
    active_miners: int  # Currently active miners
    active_validators: int  # Currently active validators
    
    # Aggregated economic metrics
    total_stake: int  # Total stake across all participants
    total_miner_stake: int  # Total stake from miners
    total_validator_stake: int  # Total stake from validators
    
    # Consensus data (stored off-chain, hash on-chain)
    weight_matrix_hash: bytes  # Hash of N x M weight matrix (32 bytes)
    consensus_scores_root: bytes  # Merkle root of consensus scores (32 bytes)
    emission_schedule_root: bytes  # Merkle root of emission schedule (32 bytes)
    
    # Economic data for current epoch
    total_emission_this_epoch: int
    miner_reward_pool: int
    validator_reward_pool: int
    
    # Performance metrics (scaled by DATUM_INT_DIVISOR)
    scaled_avg_miner_performance: int  # Average miner performance
    scaled_avg_validator_performance: int  # Average validator performance
    scaled_subnet_performance: int  # Overall subnet performance
    
    # Update tracking
    last_update_slot: int  # Last slot when this datum was updated
    last_consensus_slot: int  # Last slot when consensus was run
    last_emission_slot: int  # Last slot when emission was distributed
    
    # Off-chain storage references (IPFS/Arweave)
    detailed_state_ipfs_hash: bytes  # Full detailed state (64 bytes for IPFS CID)
    historical_data_ipfs_hash: bytes  # Historical data archive (64 bytes)
    
    # Tokenomics fields (Adaptive emission system)
    utility_score_scaled: int  # Current utility score (scaled by DATUM_INT_DIVISOR)
    epoch_emission: int  # Total emission this epoch
    total_burned: int  # Total tokens burned to date
    recycling_pool_balance: int  # Current recycling pool balance
    claim_root: bytes  # Merkle root for reward claims (32 bytes)
    
    # Emission breakdown for current epoch
    dao_allocation_this_epoch: int  # DAO treasury allocation
    emission_from_pool: int  # Amount sourced from recycling pool
    emission_from_mint: int  # Amount minted this epoch
    
    @property
    def utility_score(self) -> float:
        """Return utility score as float."""
        return self.utility_score_scaled / DATUM_INT_DIVISOR
    
    @property
    def avg_miner_performance(self) -> float:
        """Return average miner performance as float."""
        return self.scaled_avg_miner_performance / DATUM_INT_DIVISOR
    
    @property
    def avg_validator_performance(self) -> float:
        """Return average validator performance as float."""
        return self.scaled_avg_validator_performance / DATUM_INT_DIVISOR
    
    @property
    def subnet_performance(self) -> float:
        """Return overall subnet performance as float."""
        return self.scaled_subnet_performance / DATUM_INT_DIVISOR
    
    def to_dict(self) -> dict:
        """Convert to dictionary for serialization."""
        return {
            'subnet_uid': self.subnet_uid,
            'current_epoch': self.current_epoch,
            'total_miners': self.total_miners,
            'total_validators': self.total_validators,
            'active_miners': self.active_miners,
            'active_validators': self.active_validators,
            'total_stake': self.total_stake,
            'total_miner_stake': self.total_miner_stake,
            'total_validator_stake': self.total_validator_stake,
            'weight_matrix_hash': self.weight_matrix_hash.hex(),
            'consensus_scores_root': self.consensus_scores_root.hex(),
            'emission_schedule_root': self.emission_schedule_root.hex(),
            'total_emission_this_epoch': self.total_emission_this_epoch,
            'miner_reward_pool': self.miner_reward_pool,
            'validator_reward_pool': self.validator_reward_pool,
            'avg_miner_performance': self.avg_miner_performance,
            'avg_validator_performance': self.avg_validator_performance,
            'subnet_performance': self.subnet_performance,
            'last_update_slot': self.last_update_slot,
            'last_consensus_slot': self.last_consensus_slot,
            'last_emission_slot': self.last_emission_slot,
            'detailed_state_ipfs_hash': self.detailed_state_ipfs_hash.hex(),
            'historical_data_ipfs_hash': self.historical_data_ipfs_hash.hex(),
            # Tokenomics fields
            'utility_score': self.utility_score,
            'epoch_emission': self.epoch_emission,
            'total_burned': self.total_burned,
            'recycling_pool_balance': self.recycling_pool_balance,
            'claim_root': self.claim_root.hex(),
            'dao_allocation_this_epoch': self.dao_allocation_this_epoch,
            'emission_from_pool': self.emission_from_pool,
            'emission_from_mint': self.emission_from_mint,
        }
    
    @classmethod
    def create_empty(cls, subnet_uid: int, current_slot: int) -> 'SubnetAggregatedDatum':
        """Create an empty aggregated state for a new subnet."""
        return cls(
            subnet_uid=subnet_uid,
            current_epoch=0,
            total_miners=0,
            total_validators=0,
            active_miners=0,
            active_validators=0,
            total_stake=0,
            total_miner_stake=0,
            total_validator_stake=0,
            weight_matrix_hash=b'\x00' * 32,
            consensus_scores_root=b'\x00' * 32,
            emission_schedule_root=b'\x00' * 32,
            total_emission_this_epoch=0,
            miner_reward_pool=0,
            validator_reward_pool=0,
            scaled_avg_miner_performance=0,
            scaled_avg_validator_performance=0,
            scaled_subnet_performance=0,
            last_update_slot=current_slot,
            last_consensus_slot=current_slot,
            last_emission_slot=current_slot,
            detailed_state_ipfs_hash=b'\x00' * 64,
            historical_data_ipfs_hash=b'\x00' * 64,
            # Tokenomics fields
            utility_score_scaled=0,
            epoch_emission=0,
            total_burned=0,
            recycling_pool_balance=0,
            claim_root=b'\x00' * 32,
            dao_allocation_this_epoch=0,
            emission_from_pool=0,
            emission_from_mint=0,
        )


@dataclass
class SubnetAggregatedStateManager:
    """
    Manager for SubnetAggregatedDatum.
    Provides utilities to create, update, and query aggregated state.
    """
    
    def __init__(self):
        """Initialize the manager."""
        self.states: dict[int, SubnetAggregatedDatum] = {}
    
    def create_subnet_state(
        self,
        subnet_uid: int,
        current_slot: int
    ) -> SubnetAggregatedDatum:
        """
        Create a new aggregated state for a subnet.
        
        Args:
            subnet_uid: Unique identifier for the subnet
            current_slot: Current blockchain slot
            
        Returns:
            New SubnetAggregatedDatum instance
        """
        state = SubnetAggregatedDatum.create_empty(subnet_uid, current_slot)
        self.states[subnet_uid] = state
        return state
    
    def update_participant_counts(
        self,
        subnet_uid: int,
        total_miners: int,
        total_validators: int,
        active_miners: int,
        active_validators: int
    ) -> None:
        """Update participant counts for a subnet."""
        if subnet_uid not in self.states:
            raise ValueError(f"Subnet {subnet_uid} not found")
        
        state = self.states[subnet_uid]
        state.total_miners = total_miners
        state.total_validators = total_validators
        state.active_miners = active_miners
        state.active_validators = active_validators
    
    def update_economic_metrics(
        self,
        subnet_uid: int,
        total_stake: int,
        miner_stake: int,
        validator_stake: int
    ) -> None:
        """Update economic metrics for a subnet."""
        if subnet_uid not in self.states:
            raise ValueError(f"Subnet {subnet_uid} not found")
        
        state = self.states[subnet_uid]
        state.total_stake = total_stake
        state.total_miner_stake = miner_stake
        state.total_validator_stake = validator_stake
    
    def update_consensus_data(
        self,
        subnet_uid: int,
        weight_matrix_hash: bytes,
        consensus_scores_root: bytes,
        emission_schedule_root: bytes,
        current_slot: int
    ) -> None:
        """Update consensus-related hashes."""
        if subnet_uid not in self.states:
            raise ValueError(f"Subnet {subnet_uid} not found")
        
        state = self.states[subnet_uid]
        state.weight_matrix_hash = weight_matrix_hash
        state.consensus_scores_root = consensus_scores_root
        state.emission_schedule_root = emission_schedule_root
        state.last_consensus_slot = current_slot
    
    def update_performance_metrics(
        self,
        subnet_uid: int,
        avg_miner_performance: float,
        avg_validator_performance: float,
        subnet_performance: float
    ) -> None:
        """Update performance metrics."""
        if subnet_uid not in self.states:
            raise ValueError(f"Subnet {subnet_uid} not found")
        
        state = self.states[subnet_uid]
        state.scaled_avg_miner_performance = int(avg_miner_performance * DATUM_INT_DIVISOR)
        state.scaled_avg_validator_performance = int(avg_validator_performance * DATUM_INT_DIVISOR)
        state.scaled_subnet_performance = int(subnet_performance * DATUM_INT_DIVISOR)
    
    def update_emission_data(
        self,
        subnet_uid: int,
        total_emission: int,
        miner_pool: int,
        validator_pool: int,
        current_slot: int
    ) -> None:
        """Update emission and reward pool data."""
        if subnet_uid not in self.states:
            raise ValueError(f"Subnet {subnet_uid} not found")
        
        state = self.states[subnet_uid]
        state.total_emission_this_epoch = total_emission
        state.miner_reward_pool = miner_pool
        state.validator_reward_pool = validator_pool
        state.last_emission_slot = current_slot
    
    def update_tokenomics_data(
        self,
        subnet_uid: int,
        utility_score: float,
        epoch_emission: int,
        total_burned: int,
        recycling_pool_balance: int,
        claim_root: bytes,
        dao_allocation: int,
        from_pool: int,
        from_mint: int
    ) -> None:
        """
        Update tokenomics data for adaptive emission.
        
        Args:
            subnet_uid: Subnet identifier
            utility_score: Network utility score (0.0-1.0)
            epoch_emission: Total emission this epoch
            total_burned: Cumulative tokens burned
            recycling_pool_balance: Current pool balance
            claim_root: Merkle root for claims
            dao_allocation: DAO treasury allocation
            from_pool: Amount from recycling pool
            from_mint: Amount minted
        """
        if subnet_uid not in self.states:
            raise ValueError(f"Subnet {subnet_uid} not found")
        
        state = self.states[subnet_uid]
        state.utility_score_scaled = int(utility_score * DATUM_INT_DIVISOR)
        state.epoch_emission = epoch_emission
        state.total_burned = total_burned
        state.recycling_pool_balance = recycling_pool_balance
        state.claim_root = claim_root
        state.dao_allocation_this_epoch = dao_allocation
        state.emission_from_pool = from_pool
        state.emission_from_mint = from_mint
    
    def get_state(self, subnet_uid: int) -> Optional[SubnetAggregatedDatum]:
        """Get the aggregated state for a subnet."""
        return self.states.get(subnet_uid)
    
    def increment_epoch(self, subnet_uid: int, current_slot: int) -> None:
        """Increment the epoch counter for a subnet."""
        if subnet_uid not in self.states:
            raise ValueError(f"Subnet {subnet_uid} not found")
        
        state = self.states[subnet_uid]
        state.current_epoch += 1
        state.last_update_slot = current_slot
    
    def calculate_aggregated_metrics(
        self,
        miners: list,
        validators: list
    ) -> tuple[float, float, float]:
        """
        Calculate aggregated performance metrics from individual participants.
        
        Args:
            miners: List of miner objects with performance attributes
            validators: List of validator objects with performance attributes
            
        Returns:
            Tuple of (avg_miner_perf, avg_validator_perf, subnet_perf)
        """
        # Calculate average miner performance
        if miners:
            miner_perfs = [getattr(m, 'last_performance', 0.0) for m in miners]
            avg_miner_perf = sum(miner_perfs) / len(miner_perfs)
        else:
            avg_miner_perf = 0.0
        
        # Calculate average validator performance
        if validators:
            validator_perfs = [getattr(v, 'last_performance', 0.0) for v in validators]
            avg_validator_perf = sum(validator_perfs) / len(validator_perfs)
        else:
            avg_validator_perf = 0.0
        
        # Calculate overall subnet performance (weighted average)
        total_participants = len(miners) + len(validators)
        if total_participants > 0:
            subnet_perf = (
                (len(miners) * avg_miner_perf + len(validators) * avg_validator_perf)
                / total_participants
            )
        else:
            subnet_perf = 0.0
        
        return avg_miner_perf, avg_validator_perf, subnet_perf
