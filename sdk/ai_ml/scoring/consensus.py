"""
Consensus Aggregation - Advanced consensus mechanisms surpassing Bittensor.

Provides sophisticated consensus algorithms for aggregating scores from
multiple validators.
"""

import logging
import numpy as np
from enum import Enum
from typing import List, Dict, Any, Optional
from dataclasses import dataclass

from ..core.protocol import Score

logger = logging.getLogger(__name__)


class ConsensusMethod(Enum):
    """Consensus aggregation methods"""
    MEDIAN = "median"  # Robust to outliers
    WEIGHTED_MEDIAN = "weighted_median"  # Weighted by confidence
    TRIMMED_MEAN = "trimmed_mean"  # Remove outliers, then average
    STAKE_WEIGHTED = "stake_weighted"  # Weighted by validator stake
    CONFIDENCE_WEIGHTED = "confidence_weighted"  # Weighted by confidence
    ROBUST = "robust"  # Combination of methods


@dataclass
class ValidatorScore:
    """Score from a single validator"""
    validator_uid: str
    score: Score
    stake: float = 1.0
    reputation: float = 1.0


class ConsensusAggregator:
    """
    Aggregate scores from multiple validators using advanced consensus.
    
    Features:
    - Multiple aggregation methods
    - Outlier detection and removal
    - Stake-weighted consensus
    - Confidence-weighted consensus
    - Byzantine fault tolerance
    
    Example:
        aggregator = ConsensusAggregator(method=ConsensusMethod.ROBUST)
        
        # Add validator scores
        validator_scores = [
            ValidatorScore("v1", score1, stake=100),
            ValidatorScore("v2", score2, stake=50),
            ValidatorScore("v3", score3, stake=75),
        ]
        
        # Compute consensus
        consensus = aggregator.aggregate(validator_scores)
    """
    
    def __init__(
        self,
        method: ConsensusMethod = ConsensusMethod.ROBUST,
        outlier_threshold: float = 2.0,  # Standard deviations
        min_validators: int = 3,
    ):
        """
        Initialize consensus aggregator.
        
        Args:
            method: Consensus method to use
            outlier_threshold: Threshold for outlier detection (std devs)
            min_validators: Minimum validators required for consensus
        """
        self.method = method
        self.outlier_threshold = outlier_threshold
        self.min_validators = min_validators
        
        logger.info(f"ConsensusAggregator initialized with method: {method}")
    
    def aggregate(
        self,
        validator_scores: List[ValidatorScore],
        return_details: bool = False,
    ) -> Score:
        """
        Aggregate validator scores to consensus.
        
        Args:
            validator_scores: List of scores from validators
            return_details: Include aggregation details in metadata
        
        Returns:
            Consensus score
        """
        if len(validator_scores) < self.min_validators:
            logger.warning(
                f"Insufficient validators: {len(validator_scores)} < {self.min_validators}"
            )
            return Score(value=0.0, confidence=0.0, metadata={"error": "insufficient_validators"})
        
        # Extract score values and weights
        scores = np.array([vs.score.value for vs in validator_scores])
        confidences = np.array([vs.score.confidence for vs in validator_scores])
        stakes = np.array([vs.stake for vs in validator_scores])
        
        # Detect and filter outliers
        outlier_mask = self._detect_outliers(scores)
        num_outliers = np.sum(outlier_mask)
        
        # Aggregate based on method
        if self.method == ConsensusMethod.MEDIAN:
            consensus_value = self._median_consensus(scores)
        elif self.method == ConsensusMethod.WEIGHTED_MEDIAN:
            consensus_value = self._weighted_median_consensus(scores, confidences)
        elif self.method == ConsensusMethod.TRIMMED_MEAN:
            consensus_value = self._trimmed_mean_consensus(scores, outlier_mask)
        elif self.method == ConsensusMethod.STAKE_WEIGHTED:
            consensus_value = self._stake_weighted_consensus(scores, stakes)
        elif self.method == ConsensusMethod.CONFIDENCE_WEIGHTED:
            consensus_value = self._confidence_weighted_consensus(scores, confidences)
        elif self.method == ConsensusMethod.ROBUST:
            consensus_value = self._robust_consensus(
                scores, confidences, stakes, outlier_mask
            )
        else:
            consensus_value = float(np.mean(scores))
        
        # Estimate consensus confidence
        consensus_confidence = self._estimate_consensus_confidence(
            scores, confidences, outlier_mask
        )
        
        # Prepare metadata
        metadata = {
            "method": self.method.value,
            "num_validators": len(validator_scores),
            "num_outliers": int(num_outliers),
            "score_std": float(np.std(scores)),
            "score_range": [float(np.min(scores)), float(np.max(scores))],
        }
        
        if return_details:
            metadata["validator_scores"] = [
                {
                    "uid": vs.validator_uid,
                    "score": vs.score.value,
                    "confidence": vs.score.confidence,
                    "stake": vs.stake,
                    "is_outlier": bool(outlier_mask[i]),
                }
                for i, vs in enumerate(validator_scores)
            ]
        
        return Score(
            value=consensus_value,
            confidence=consensus_confidence,
            metadata=metadata,
        )
    
    def _detect_outliers(self, scores: np.ndarray) -> np.ndarray:
        """
        Detect outliers using modified Z-score.
        
        Returns:
            Boolean mask where True indicates outlier
        """
        if len(scores) < 3:
            return np.zeros(len(scores), dtype=bool)
        
        median = np.median(scores)
        mad = np.median(np.abs(scores - median))
        
        if mad == 0:
            return np.zeros(len(scores), dtype=bool)
        
        # Modified Z-score
        modified_z = 0.6745 * (scores - median) / mad
        
        return np.abs(modified_z) > self.outlier_threshold
    
    def _median_consensus(self, scores: np.ndarray) -> float:
        """Simple median consensus"""
        return float(np.median(scores))
    
    def _weighted_median_consensus(
        self,
        scores: np.ndarray,
        confidences: np.ndarray,
    ) -> float:
        """Weighted median using confidences"""
        # Sort by score
        sorted_indices = np.argsort(scores)
        sorted_scores = scores[sorted_indices]
        sorted_weights = confidences[sorted_indices]
        
        # Normalize weights
        sorted_weights = sorted_weights / np.sum(sorted_weights)
        
        # Find weighted median
        cumsum = np.cumsum(sorted_weights)
        median_idx = np.searchsorted(cumsum, 0.5)
        
        return float(sorted_scores[median_idx])
    
    def _trimmed_mean_consensus(
        self,
        scores: np.ndarray,
        outlier_mask: np.ndarray,
    ) -> float:
        """Trimmed mean (remove outliers)"""
        clean_scores = scores[~outlier_mask]
        
        if len(clean_scores) == 0:
            return float(np.mean(scores))
        
        return float(np.mean(clean_scores))
    
    def _stake_weighted_consensus(
        self,
        scores: np.ndarray,
        stakes: np.ndarray,
    ) -> float:
        """Stake-weighted average"""
        total_stake = np.sum(stakes)
        
        if total_stake == 0:
            return float(np.mean(scores))
        
        weighted_sum = np.sum(scores * stakes)
        return float(weighted_sum / total_stake)
    
    def _confidence_weighted_consensus(
        self,
        scores: np.ndarray,
        confidences: np.ndarray,
    ) -> float:
        """Confidence-weighted average"""
        total_confidence = np.sum(confidences)
        
        if total_confidence == 0:
            return float(np.mean(scores))
        
        weighted_sum = np.sum(scores * confidences)
        return float(weighted_sum / total_confidence)
    
    def _robust_consensus(
        self,
        scores: np.ndarray,
        confidences: np.ndarray,
        stakes: np.ndarray,
        outlier_mask: np.ndarray,
    ) -> float:
        """
        Robust consensus combining multiple methods.
        
        Uses trimmed mean weighted by both confidence and stake.
        """
        # Remove outliers
        clean_scores = scores[~outlier_mask]
        clean_confidences = confidences[~outlier_mask]
        clean_stakes = stakes[~outlier_mask]
        
        if len(clean_scores) == 0:
            # Fallback to simple mean
            return float(np.mean(scores))
        
        # Combine confidence and stake weights
        weights = clean_confidences * np.sqrt(clean_stakes)
        total_weight = np.sum(weights)
        
        if total_weight == 0:
            return float(np.mean(clean_scores))
        
        weighted_sum = np.sum(clean_scores * weights)
        return float(weighted_sum / total_weight)
    
    def _estimate_consensus_confidence(
        self,
        scores: np.ndarray,
        confidences: np.ndarray,
        outlier_mask: np.ndarray,
    ) -> float:
        """
        Estimate confidence in consensus.
        
        Higher confidence when:
        - Low variance in scores
        - High average validator confidence
        - Few outliers
        """
        # Score agreement (low variance = high confidence)
        score_std = np.std(scores)
        agreement = 1.0 - min(score_std * 2, 0.5)  # Normalize variance to confidence
        
        # Average validator confidence
        avg_confidence = float(np.mean(confidences))
        
        # Outlier penalty
        outlier_ratio = np.sum(outlier_mask) / len(scores)
        outlier_penalty = 1.0 - (outlier_ratio * 0.3)  # Max 30% penalty
        
        # Combine factors
        consensus_confidence = (
            0.4 * agreement +
            0.4 * avg_confidence +
            0.2 * outlier_penalty
        )
        
        return max(0.5, min(1.0, consensus_confidence))
    
    def get_config(self) -> Dict[str, Any]:
        """Get aggregator configuration"""
        return {
            "method": self.method.value,
            "outlier_threshold": self.outlier_threshold,
            "min_validators": self.min_validators,
        }
