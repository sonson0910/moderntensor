# sdk/consensus/yudkowsky_v2.py
"""
Enhanced Yudkowsky Consensus Algorithm (Version 2).

This module implements an improved consensus mechanism over the basic
stake-weighted averaging, with the following enhancements:

1. Non-linear bonding curves to reward top performers exponentially
2. Stake-weighted voting with dampening to reduce centralization
3. Outlier detection to remove extreme/malicious scores
4. Weighted median instead of weighted average (robust to outliers)
5. Historical performance tracking for validator trust

Compared to Bittensor's simple weighted average, this provides:
- Better resistance to manipulation
- Fairer reward distribution
- Exponential rewards for top performers
- Automatic outlier removal
"""

import math
import hashlib
from typing import List, Dict, Tuple, Optional
from dataclasses import dataclass
import numpy as np
from collections import defaultdict

from sdk.core.datatypes import ValidatorInfo, MinerInfo


@dataclass
class ConsensusConfig:
    """Configuration for YudkowskyConsensusV2."""
    
    # Bonding curve parameter (alpha)
    # f(x) = x^alpha where alpha > 1
    # Higher alpha = more aggressive rewarding of top performers
    bonding_curve_alpha: float = 2.0
    
    # Stake dampening to reduce whale dominance
    # stake_weight = stake^dampening_factor
    # 1.0 = no dampening, 0.5 = square root dampening
    stake_dampening_factor: float = 0.5
    
    # Outlier detection threshold (in standard deviations)
    # Scores beyond this threshold are considered outliers
    outlier_threshold_std: float = 2.5
    
    # Minimum number of validators required for consensus
    min_validators: int = 3
    
    # Trust decay rate per epoch (for inactive validators)
    trust_decay_rate: float = 0.95
    
    # Trust update rate (how quickly trust adapts to new scores)
    trust_update_rate: float = 0.1
    
    # Use weighted median instead of weighted mean
    use_weighted_median: bool = True
    
    # Minimum trust score (validators below this are ignored)
    min_trust_score: float = 0.1


@dataclass
class ValidatorTrust:
    """Historical trust tracking for a validator."""
    
    validator_uid: str
    trust_score: float  # 0.0 to 1.0
    total_participations: int
    successful_participations: int
    avg_deviation_from_consensus: float
    last_updated_epoch: int
    
    @property
    def participation_rate(self) -> float:
        """Calculate participation rate."""
        if self.total_participations == 0:
            return 0.0
        return self.successful_participations / self.total_participations
    
    def update_trust(self, deviation: float, participated: bool, epoch: int, config: ConsensusConfig):
        """
        Update trust score based on participation and deviation.
        
        Args:
            deviation: How far validator's score deviated from consensus (0-1)
            participated: Whether validator participated in this epoch
            epoch: Current epoch number
            config: Consensus configuration
        """
        # Update participation tracking
        self.total_participations += 1
        if participated:
            self.successful_participations += 1
        
        # Update average deviation (exponential moving average)
        if participated:
            if self.avg_deviation_from_consensus == 0:
                self.avg_deviation_from_consensus = deviation
            else:
                alpha = config.trust_update_rate
                self.avg_deviation_from_consensus = (
                    alpha * deviation + (1 - alpha) * self.avg_deviation_from_consensus
                )
        
        # Calculate new trust score
        # Trust = participation_rate * (1 - deviation_penalty)
        participation_factor = self.participation_rate
        deviation_penalty = min(self.avg_deviation_from_consensus, 1.0)
        new_trust = participation_factor * (1 - deviation_penalty)
        
        # Smooth trust update
        self.trust_score = (
            config.trust_update_rate * new_trust + 
            (1 - config.trust_update_rate) * self.trust_score
        )
        
        # Apply decay if not participated
        if not participated:
            self.trust_score *= config.trust_decay_rate
        
        # Clamp to valid range
        self.trust_score = max(config.min_trust_score, min(1.0, self.trust_score))
        self.last_updated_epoch = epoch


class YudkowskyConsensusV2:
    """
    Enhanced Yudkowsky consensus algorithm with bonding curves and outlier detection.
    
    This class provides improved consensus calculation over simple stake-weighted
    averaging, with the following features:
    - Non-linear bonding curves
    - Stake dampening
    - Outlier detection
    - Weighted median
    - Historical trust tracking
    """
    
    def __init__(self, config: Optional[ConsensusConfig] = None):
        """
        Initialize the consensus algorithm.
        
        Args:
            config: Configuration parameters. If None, uses default config.
        """
        self.config = config or ConsensusConfig()
        self.validator_trust: Dict[str, ValidatorTrust] = {}
    
    def calculate_consensus(
        self,
        validator_scores: Dict[str, List[float]],
        validator_stakes: Dict[str, int],
        miners: List[MinerInfo],
        current_epoch: int
    ) -> Dict[str, float]:
        """
        Calculate consensus scores with enhanced Yudkowsky algorithm.
        
        Args:
            validator_scores: Dict mapping validator UIDs to list of miner scores
            validator_stakes: Dict mapping validator UIDs to stake amounts
            miners: List of miner information
            current_epoch: Current epoch number
            
        Returns:
            Dict mapping miner UIDs to consensus scores (with bonding curve applied)
        """
        if len(validator_scores) < self.config.min_validators:
            raise ValueError(
                f"Insufficient validators: {len(validator_scores)} < "
                f"{self.config.min_validators}"
            )
        
        # Step 1: Initialize trust for new validators
        self._initialize_trust(validator_scores.keys(), current_epoch)
        
        # Step 2: Calculate dampened stake weights with trust factor
        stake_weights = self._calculate_stake_weights(
            validator_scores.keys(),
            validator_stakes
        )
        
        # Step 3: Filter outliers for each miner
        filtered_scores = self._filter_outliers(validator_scores, miners)
        
        # Step 4: Calculate consensus scores
        consensus_scores = {}
        for miner_idx, miner in enumerate(miners):
            # Get scores for this miner from all validators
            miner_scores = [
                (filtered_scores[val_uid][miner_idx], stake_weights[val_uid])
                for val_uid in filtered_scores.keys()
                if miner_idx < len(filtered_scores[val_uid])
            ]
            
            if not miner_scores:
                consensus_scores[miner.uid] = 0.0
                continue
            
            # Calculate weighted median or mean
            if self.config.use_weighted_median:
                consensus_score = self._weighted_median(miner_scores)
            else:
                consensus_score = self._weighted_mean(miner_scores)
            
            # Apply bonding curve
            bonded_score = self._apply_bonding_curve(consensus_score)
            
            consensus_scores[miner.uid] = bonded_score
        
        # Step 5: Update validator trust based on deviations
        self._update_validator_trust(
            validator_scores,
            consensus_scores,
            miners,
            current_epoch
        )
        
        return consensus_scores
    
    def _initialize_trust(self, validator_uids: List[str], current_epoch: int):
        """Initialize trust for new validators."""
        for val_uid in validator_uids:
            if val_uid not in self.validator_trust:
                self.validator_trust[val_uid] = ValidatorTrust(
                    validator_uid=val_uid,
                    trust_score=0.5,  # Start with neutral trust
                    total_participations=0,
                    successful_participations=0,
                    avg_deviation_from_consensus=0.0,
                    last_updated_epoch=current_epoch
                )
    
    def _calculate_stake_weights(
        self,
        validator_uids: List[str],
        validator_stakes: Dict[str, int]
    ) -> Dict[str, float]:
        """
        Calculate dampened stake weights with trust factor.
        
        Applies:
        1. Stake dampening (sqrt to reduce whale dominance)
        2. Trust factor (0.5 to 1.5 range)
        3. Normalization to sum to 1.0
        
        Args:
            validator_uids: List of validator UIDs
            validator_stakes: Dict mapping validator UIDs to stakes
            
        Returns:
            Dict mapping validator UIDs to normalized weights
        """
        weights = {}
        
        for val_uid in validator_uids:
            stake = validator_stakes.get(val_uid, 0)
            if stake <= 0:
                weights[val_uid] = 0.0
                continue
            
            # Apply stake dampening (sqrt by default)
            dampened_stake = math.pow(stake, self.config.stake_dampening_factor)
            
            # Get trust factor (0.5 to 1.5 range)
            trust = self.validator_trust.get(val_uid)
            if trust:
                trust_factor = 0.5 + trust.trust_score
            else:
                trust_factor = 1.0  # Neutral for new validators
            
            # Combined weight
            weights[val_uid] = dampened_stake * trust_factor
        
        # Normalize to sum to 1.0
        total_weight = sum(weights.values())
        if total_weight > 0:
            weights = {uid: w / total_weight for uid, w in weights.items()}
        
        return weights
    
    def _filter_outliers(
        self,
        validator_scores: Dict[str, List[float]],
        miners: List[MinerInfo]
    ) -> Dict[str, List[float]]:
        """
        Remove outlier scores for each miner.
        
        For each miner, calculates mean and std dev of validator scores,
        then removes scores beyond threshold standard deviations.
        
        Args:
            validator_scores: Dict mapping validator UIDs to miner scores
            miners: List of miners
            
        Returns:
            Filtered validator scores (outliers removed)
        """
        filtered = {}
        num_miners = len(miners)
        
        # For each validator, collect their scores
        for val_uid, scores in validator_scores.items():
            filtered[val_uid] = list(scores)  # Copy
        
        # For each miner, detect and remove outliers across validators
        for miner_idx in range(num_miners):
            # Collect all validator scores for this miner
            miner_scores = []
            for val_uid, scores in validator_scores.items():
                if miner_idx < len(scores):
                    miner_scores.append(scores[miner_idx])
            
            if len(miner_scores) < 3:
                continue  # Need at least 3 scores for outlier detection
            
            # Calculate statistics
            mean_score = np.mean(miner_scores)
            std_score = np.std(miner_scores)
            
            if std_score == 0:
                continue  # All scores identical, no outliers
            
            # Determine outlier threshold
            lower_bound = mean_score - self.config.outlier_threshold_std * std_score
            upper_bound = mean_score + self.config.outlier_threshold_std * std_score
            
            # Mark outliers (set to median for smoothing)
            median_score = np.median(miner_scores)
            for val_uid in validator_scores.keys():
                if miner_idx < len(filtered[val_uid]):
                    score = filtered[val_uid][miner_idx]
                    if score < lower_bound or score > upper_bound:
                        # Replace outlier with median
                        filtered[val_uid][miner_idx] = median_score
        
        return filtered
    
    def _weighted_median(self, scores_weights: List[Tuple[float, float]]) -> float:
        """
        Calculate weighted median (robust to outliers).
        
        Args:
            scores_weights: List of (score, weight) tuples
            
        Returns:
            Weighted median score
        """
        if not scores_weights:
            return 0.0
        
        # Sort by score
        sorted_sw = sorted(scores_weights, key=lambda x: x[0])
        
        # Calculate cumulative weights
        total_weight = sum(w for _, w in sorted_sw)
        if total_weight == 0:
            return 0.0
        
        cumulative = 0.0
        for score, weight in sorted_sw:
            cumulative += weight
            if cumulative >= total_weight / 2:
                return score
        
        # Fallback (should not reach here)
        return sorted_sw[-1][0]
    
    def _weighted_mean(self, scores_weights: List[Tuple[float, float]]) -> float:
        """
        Calculate weighted mean.
        
        Args:
            scores_weights: List of (score, weight) tuples
            
        Returns:
            Weighted mean score
        """
        if not scores_weights:
            return 0.0
        
        total_weight = sum(w for _, w in scores_weights)
        if total_weight == 0:
            return 0.0
        
        weighted_sum = sum(s * w for s, w in scores_weights)
        return weighted_sum / total_weight
    
    def _apply_bonding_curve(self, score: float) -> float:
        """
        Apply bonding curve to reward top performers exponentially.
        
        f(x) = x^alpha where alpha > 1
        
        This creates non-linear rewards:
        - score = 0.5, alpha = 2.0 → bonded = 0.25 (penalty)
        - score = 0.8, alpha = 2.0 → bonded = 0.64 (slight penalty)
        - score = 1.0, alpha = 2.0 → bonded = 1.00 (same)
        - score = 0.9, alpha = 2.0 → bonded = 0.81
        
        Args:
            score: Original score (0-1)
            
        Returns:
            Bonded score (0-1)
        """
        if score < 0:
            return 0.0
        if score > 1:
            return 1.0
        
        return math.pow(score, self.config.bonding_curve_alpha)
    
    def _update_validator_trust(
        self,
        validator_scores: Dict[str, List[float]],
        consensus_scores: Dict[str, float],
        miners: List[MinerInfo],
        current_epoch: int
    ):
        """
        Update validator trust based on deviation from consensus.
        
        Args:
            validator_scores: Original validator scores
            consensus_scores: Final consensus scores
            miners: List of miners
            current_epoch: Current epoch
        """
        for val_uid, scores in validator_scores.items():
            if val_uid not in self.validator_trust:
                continue
            
            # Calculate average deviation from consensus
            deviations = []
            for miner_idx, miner in enumerate(miners):
                if miner_idx >= len(scores):
                    continue
                
                val_score = scores[miner_idx]
                consensus_score = consensus_scores.get(miner.uid, 0.0)
                
                # Calculate absolute deviation (normalized)
                deviation = abs(val_score - consensus_score)
                deviations.append(deviation)
            
            # Update trust
            if deviations:
                avg_deviation = np.mean(deviations)
                participated = True
            else:
                avg_deviation = 0.0
                participated = False
            
            trust = self.validator_trust[val_uid]
            trust.update_trust(avg_deviation, participated, current_epoch, self.config)
    
    def get_validator_trust_scores(self) -> Dict[str, float]:
        """
        Get current trust scores for all validators.
        
        Returns:
            Dict mapping validator UIDs to trust scores
        """
        return {
            uid: trust.trust_score
            for uid, trust in self.validator_trust.items()
        }
    
    def export_trust_state(self) -> Dict[str, dict]:
        """
        Export validator trust state for persistence.
        
        Returns:
            Dict mapping validator UIDs to trust state dicts
        """
        return {
            uid: {
                'trust_score': trust.trust_score,
                'total_participations': trust.total_participations,
                'successful_participations': trust.successful_participations,
                'avg_deviation_from_consensus': trust.avg_deviation_from_consensus,
                'last_updated_epoch': trust.last_updated_epoch,
            }
            for uid, trust in self.validator_trust.items()
        }
    
    def import_trust_state(self, trust_state: Dict[str, dict]):
        """
        Import validator trust state from persistence.
        
        Args:
            trust_state: Dict mapping validator UIDs to trust state dicts
        """
        for uid, state in trust_state.items():
            self.validator_trust[uid] = ValidatorTrust(
                validator_uid=uid,
                trust_score=state['trust_score'],
                total_participations=state['total_participations'],
                successful_participations=state['successful_participations'],
                avg_deviation_from_consensus=state['avg_deviation_from_consensus'],
                last_updated_epoch=state['last_updated_epoch']
            )
