# sdk/consensus/layer1_integration.py
"""
Integration module for Layer 1 Phase 1 features with existing consensus system.

This module provides utilities to integrate:
1. SubnetAggregatedDatum with existing consensus state
2. WeightMatrixManager with existing scoring mechanisms
3. Epoch processing with aggregated state updates
4. Adaptive tokenomics with consensus results
"""

from typing import List, Optional, Dict, Tuple
import numpy as np

from sdk.metagraph.aggregated_state import (
    SubnetAggregatedDatum,
    SubnetAggregatedStateManager
)
from sdk.consensus.weight_matrix import WeightMatrixManager
from sdk.core.datatypes import MinerInfo, ValidatorInfo
from sdk.metagraph.metagraph_datum import STATUS_ACTIVE
from sdk.tokenomics import (
    TokenomicsIntegration,
    ConsensusData,
    NetworkMetricsCollector
)


class Layer1ConsensusIntegrator:
    """
    Integrates Layer 1 Phase 1 features with existing consensus system.
    
    This class bridges the gap between the new aggregated state model
    and the existing consensus scoring mechanisms.
    """
    
    def __init__(
        self,
        state_manager: Optional[SubnetAggregatedStateManager] = None,
        weight_manager: Optional[WeightMatrixManager] = None,
        tokenomics: Optional[TokenomicsIntegration] = None
    ):
        """
        Initialize the integrator.
        
        Args:
            state_manager: Manager for aggregated subnet states
            weight_manager: Manager for weight matrices
            tokenomics: Tokenomics integration (created if not provided)
        """
        self.state_manager = state_manager or SubnetAggregatedStateManager()
        self.weight_manager = weight_manager or WeightMatrixManager()
        self.tokenomics = tokenomics or TokenomicsIntegration()
        self.metrics_collector = NetworkMetricsCollector()
    
    async def process_consensus_round(
        self,
        subnet_uid: int,
        current_epoch: int,
        current_slot: int,
        miners: List[MinerInfo],
        validators: List[ValidatorInfo],
        validator_scores: Dict[str, List[float]]
    ) -> SubnetAggregatedDatum:
        """
        Process a complete consensus round and update aggregated state.
        
        This method:
        1. Calculates consensus scores from validator inputs
        2. Builds weight matrix
        3. Updates aggregated state with new data
        4. Returns updated aggregated datum
        
        Args:
            subnet_uid: Subnet identifier
            current_epoch: Current epoch number
            current_slot: Current blockchain slot
            miners: List of miner information
            validators: List of validator information
            validator_scores: Dict mapping validator UIDs to miner scores
            
        Returns:
            Updated SubnetAggregatedDatum
        """
        # Ensure subnet state exists
        if self.state_manager.get_state(subnet_uid) is None:
            self.state_manager.create_subnet_state(subnet_uid, current_slot)
        
        # Build weight matrix (validators x miners)
        weight_matrix = self._build_weight_matrix(validators, miners, validator_scores)
        
        # Store weight matrix and get Merkle root
        merkle_root, ipfs_hash = await self.weight_manager.store_weight_matrix(
            subnet_uid=subnet_uid,
            epoch=current_epoch,
            weights=weight_matrix,
            upload_to_ipfs=False  # Set to True in production
        )
        
        # Calculate consensus scores (simplified - weighted average)
        consensus_scores = self._calculate_consensus_scores(
            weight_matrix,
            validators,
            miners
        )
        
        # Calculate Merkle root for consensus scores
        consensus_root = self._calculate_consensus_root(consensus_scores)
        
        # Calculate emission schedule (simplified)
        emission_schedule = self._calculate_emission_schedule(
            miners,
            consensus_scores
        )
        emission_root = self._calculate_emission_root(emission_schedule)
        
        # Update aggregated state
        self._update_aggregated_state(
            subnet_uid=subnet_uid,
            current_slot=current_slot,
            miners=miners,
            validators=validators,
            weight_hash=merkle_root,
            consensus_root=consensus_root,
            emission_root=emission_root,
            total_emission=sum(emission_schedule.values())
        )
        
        # Process tokenomics
        tokenomics_result = self._process_tokenomics(
            subnet_uid=subnet_uid,
            epoch=current_epoch,
            consensus_scores=consensus_scores,
            validators=validators
        )
        
        return self.state_manager.get_state(subnet_uid)
    
    def _build_weight_matrix(
        self,
        validators: List[ValidatorInfo],
        miners: List[MinerInfo],
        validator_scores: Dict[str, List[float]]
    ) -> np.ndarray:
        """
        Build weight matrix from validator scores.
        
        Args:
            validators: List of validators
            miners: List of miners
            validator_scores: Scores from each validator
            
        Returns:
            Weight matrix (num_validators x num_miners)
        """
        num_validators = len(validators)
        num_miners = len(miners)
        
        weight_matrix = np.zeros((num_validators, num_miners))
        
        for v_idx, validator in enumerate(validators):
            v_uid = validator.uid
            if v_uid in validator_scores:
                scores = validator_scores[v_uid]
                # Normalize scores to sum to 1.0 (validator's total weight)
                score_sum = sum(scores) if sum(scores) > 0 else 1.0
                normalized_scores = [s / score_sum for s in scores]
                
                # Fill matrix row with validator's weights
                for m_idx, score in enumerate(normalized_scores[:num_miners]):
                    weight_matrix[v_idx, m_idx] = score
        
        return weight_matrix
    
    def _calculate_consensus_scores(
        self,
        weight_matrix: np.ndarray,
        validators: List[ValidatorInfo],
        miners: List[MinerInfo]
    ) -> Dict[str, float]:
        """
        Calculate consensus scores from weight matrix.
        
        Uses stake-weighted averaging of validator scores.
        
        Args:
            weight_matrix: Weight matrix (validators x miners)
            validators: List of validators with stakes
            miners: List of miners
            
        Returns:
            Dict mapping miner UIDs to consensus scores
        """
        # Get validator stakes
        validator_stakes = np.array([v.stake for v in validators])
        total_stake = np.sum(validator_stakes) if np.sum(validator_stakes) > 0 else 1.0
        
        # Normalize stakes to sum to 1.0
        stake_weights = validator_stakes / total_stake
        
        # Calculate weighted average scores for each miner
        consensus_scores = {}
        for m_idx, miner in enumerate(miners):
            # Get all validator weights for this miner
            miner_weights = weight_matrix[:, m_idx]
            
            # Calculate stake-weighted consensus
            consensus_score = np.dot(stake_weights, miner_weights)
            consensus_scores[miner.uid] = float(consensus_score)
        
        return consensus_scores
    
    def _calculate_emission_schedule(
        self,
        miners: List[MinerInfo],
        consensus_scores: Dict[str, float]
    ) -> Dict[str, int]:
        """
        Calculate emission schedule based on consensus scores.
        
        Args:
            miners: List of miners
            consensus_scores: Consensus scores for each miner
            
        Returns:
            Dict mapping miner UIDs to emission amounts
        """
        # Total emission per epoch (simplified - use adaptive emission in production)
        total_epoch_emission = 1000000  # 1M tokens per epoch
        
        # Calculate emission proportional to consensus scores
        total_score = sum(consensus_scores.values())
        if total_score == 0:
            total_score = 1.0
        
        emission_schedule = {}
        for miner in miners:
            score = consensus_scores.get(miner.uid, 0.0)
            emission = int((score / total_score) * total_epoch_emission)
            emission_schedule[miner.uid] = emission
        
        return emission_schedule
    
    def _calculate_consensus_root(self, consensus_scores: Dict[str, float]) -> bytes:
        """Calculate Merkle root for consensus scores."""
        import hashlib
        
        # Sort by UID for deterministic ordering
        sorted_items = sorted(consensus_scores.items())
        
        # Hash each score
        score_hashes = []
        for uid, score in sorted_items:
            score_bytes = f"{uid}:{score}".encode('utf-8')
            score_hash = hashlib.sha256(score_bytes).digest()
            score_hashes.append(score_hash)
        
        # Calculate root (simplified)
        all_hashes = b''.join(score_hashes)
        return hashlib.sha256(all_hashes).digest()
    
    def _calculate_emission_root(self, emission_schedule: Dict[str, int]) -> bytes:
        """Calculate Merkle root for emission schedule."""
        import hashlib
        
        # Sort by UID for deterministic ordering
        sorted_items = sorted(emission_schedule.items())
        
        # Hash each emission
        emission_hashes = []
        for uid, emission in sorted_items:
            emission_bytes = f"{uid}:{emission}".encode('utf-8')
            emission_hash = hashlib.sha256(emission_bytes).digest()
            emission_hashes.append(emission_hash)
        
        # Calculate root (simplified)
        all_hashes = b''.join(emission_hashes)
        return hashlib.sha256(all_hashes).digest()
    
    def _update_aggregated_state(
        self,
        subnet_uid: int,
        current_slot: int,
        miners: List[MinerInfo],
        validators: List[ValidatorInfo],
        weight_hash: bytes,
        consensus_root: bytes,
        emission_root: bytes,
        total_emission: int
    ) -> None:
        """Update aggregated state with new consensus data."""
        # Update participant counts
        active_miners = sum(1 for m in miners if m.status == STATUS_ACTIVE)
        active_validators = sum(1 for v in validators if v.status == STATUS_ACTIVE)
        
        self.state_manager.update_participant_counts(
            subnet_uid=subnet_uid,
            total_miners=len(miners),
            total_validators=len(validators),
            active_miners=active_miners,
            active_validators=active_validators
        )
        
        # Update economic metrics
        total_stake = sum(m.stake for m in miners) + sum(v.stake for v in validators)
        miner_stake = sum(m.stake for m in miners)
        validator_stake = sum(v.stake for v in validators)
        
        self.state_manager.update_economic_metrics(
            subnet_uid=subnet_uid,
            total_stake=total_stake,
            miner_stake=miner_stake,
            validator_stake=validator_stake
        )
        
        # Update consensus data
        self.state_manager.update_consensus_data(
            subnet_uid=subnet_uid,
            weight_matrix_hash=weight_hash,
            consensus_scores_root=consensus_root,
            emission_schedule_root=emission_root,
            current_slot=current_slot
        )
        
        # Update performance metrics
        avg_miner, avg_validator, subnet_perf = \
            self.state_manager.calculate_aggregated_metrics(miners, validators)
        
        self.state_manager.update_performance_metrics(
            subnet_uid=subnet_uid,
            avg_miner_performance=avg_miner,
            avg_validator_performance=avg_validator,
            subnet_performance=subnet_perf
        )
        
        # Update emission data
        miner_pool = int(total_emission * 0.6)  # 60% to miners
        validator_pool = int(total_emission * 0.4)  # 40% to validators
        
        self.state_manager.update_emission_data(
            subnet_uid=subnet_uid,
            total_emission=total_emission,
            miner_pool=miner_pool,
            validator_pool=validator_pool,
            current_slot=current_slot
        )
    
    async def verify_consensus_integrity(
        self,
        subnet_uid: int,
        epoch: int,
        miners: List[MinerInfo],
        validators: List[ValidatorInfo],
        validator_scores: Dict[str, List[float]]
    ) -> Tuple[bool, str]:
        """
        Verify the integrity of consensus data for an epoch.
        
        Args:
            subnet_uid: Subnet identifier
            epoch: Epoch to verify
            miners: List of miners
            validators: List of validators
            validator_scores: Validator scores
            
        Returns:
            Tuple of (is_valid, message)
        """
        # Get aggregated state
        state = self.state_manager.get_state(subnet_uid)
        if state is None:
            return False, "Aggregated state not found"
        
        # Verify weight matrix
        weight_matrix = self._build_weight_matrix(validators, miners, validator_scores)
        is_valid = await self.weight_manager.verify_weight_matrix(
            subnet_uid=subnet_uid,
            epoch=epoch,
            weights=weight_matrix,
            merkle_root=state.weight_matrix_hash
        )
        
        if not is_valid:
            return False, "Weight matrix verification failed"
        
        # Verify participant counts
        if state.total_miners != len(miners):
            return False, f"Miner count mismatch: {state.total_miners} != {len(miners)}"
        
        if state.total_validators != len(validators):
            return False, f"Validator count mismatch: {state.total_validators} != {len(validators)}"
        
        return True, "Consensus integrity verified successfully"
    
    def get_subnet_summary(self, subnet_uid: int) -> Optional[Dict]:
        """
        Get a summary of subnet state.
        
        Args:
            subnet_uid: Subnet identifier
            
        Returns:
            Dictionary with subnet summary or None
        """
        state = self.state_manager.get_state(subnet_uid)
        if state is None:
            return None
        
        return state.to_dict()
    
    def _process_tokenomics(
        self,
        subnet_uid: int,
        epoch: int,
        consensus_scores: Dict[str, float],
        validators: List[ValidatorInfo]
    ) -> None:
        """
        Process tokenomics for the epoch.
        
        This integrates adaptive tokenomics with consensus results.
        
        Args:
            subnet_uid: Subnet identifier
            epoch: Current epoch number
            consensus_scores: Miner consensus scores
            validators: List of validators with stakes
        """
        # Get network metrics
        network_metrics = self.metrics_collector.get_epoch_metrics()
        
        # Prepare consensus data
        validator_stakes = {v.uid: v.stake for v in validators}
        
        # Calculate average quality score (simplified - could be more sophisticated)
        quality_score = sum(consensus_scores.values()) / len(consensus_scores) if consensus_scores else 0.5
        
        consensus_data = ConsensusData(
            miner_scores=consensus_scores,
            validator_stakes=validator_stakes,
            quality_score=quality_score
        )
        
        # Process epoch tokenomics
        tokenomics_result = self.tokenomics.process_epoch_tokenomics(
            epoch=epoch,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        # Update aggregated state with tokenomics data
        self.state_manager.update_tokenomics_data(
            subnet_uid=subnet_uid,
            utility_score=tokenomics_result.utility_score,
            epoch_emission=tokenomics_result.emission_amount,
            total_burned=self.tokenomics.burn.total_burned,
            recycling_pool_balance=self.tokenomics.pool.pool_balance,
            claim_root=tokenomics_result.claim_root,
            dao_allocation=tokenomics_result.dao_allocation,
            from_pool=tokenomics_result.from_pool,
            from_mint=tokenomics_result.from_mint
        )
        
        # Reset metrics for next epoch
        self.metrics_collector.reset_for_new_epoch()
    
    def record_task_submission(self, difficulty: float = 0.5) -> None:
        """
        Record a task submission for metrics.
        
        Args:
            difficulty: Task difficulty (0.0-1.0)
        """
        self.metrics_collector.record_task_submission(difficulty)
    
    def add_to_recycling_pool(self, amount: int, source: str) -> None:
        """
        Add tokens to recycling pool.
        
        Args:
            amount: Amount to add
            source: Source of tokens (e.g., 'registration_fees', 'slashing_penalties')
        """
        self.tokenomics.add_to_recycling_pool(amount, source)
    
    def get_tokenomics_stats(self) -> Dict:
        """
        Get comprehensive tokenomics statistics.
        
        Returns:
            Dictionary with tokenomics metrics
        """
        return self.tokenomics.get_stats()
