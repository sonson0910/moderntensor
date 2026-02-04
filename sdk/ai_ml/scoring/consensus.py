"""
Consensus Aggregation - Advanced consensus mechanisms surpassing Bittensor.

Provides sophisticated consensus algorithms for aggregating scores from
multiple validators.
"""

import logging
from enum import Enum
from typing import List, Dict, Any
from dataclasses import dataclass

# Try to import numpy, fallback to stdlib if not available
try:
    import numpy as np
    HAS_NUMPY = True
except ImportError:
    HAS_NUMPY = False
    import statistics

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
        scores = self._to_array([vs.score.value for vs in validator_scores])
        confidences = self._to_array([vs.score.confidence for vs in validator_scores])
        stakes = self._to_array([vs.stake for vs in validator_scores])
        
        # Detect and filter outliers
        outlier_mask = self._detect_outliers(scores)
        num_outliers = self._sum(outlier_mask)
        
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
            consensus_value = float(self._mean(scores))
        
        # Estimate consensus confidence
        consensus_confidence = self._estimate_consensus_confidence(
            scores, confidences, outlier_mask
        )
        
        # Prepare metadata
        metadata = {
            "method": self.method.value,
            "num_validators": len(validator_scores),
            "num_outliers": int(num_outliers),
            "score_std": float(self._std(scores)),
            "score_range": [float(self._min(scores)), float(self._max(scores))],
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
    
    def _detect_outliers(self, scores: Any) -> Any:
        """
        Detect outliers using modified Z-score.
        
        Returns:
            Boolean mask where True indicates outlier
        """
        if len(scores) < 3:
            return np.zeros(len(scores), dtype=bool)
        
        median = self._median(scores)
        mad = self._median(self._abs(scores - median))
        
        if mad == 0:
            return np.zeros(len(scores), dtype=bool)
        
        # Modified Z-score
        modified_z = 0.6745 * (scores - median) / mad
        
        return self._abs(modified_z) > self.outlier_threshold
    
    def _median_consensus(self, scores: Any) -> float:
        """Simple median consensus"""
        return float(self._median(scores))
    
    def _weighted_median_consensus(
        self,
        scores: Any,
        confidences: Any,
    ) -> float:
        """Weighted median using confidences"""
        # Sort by score
        sorted_indices = self._argsort(scores)
        sorted_scores = scores[sorted_indices]
        sorted_weights = confidences[sorted_indices]
        
        # Normalize weights
        sorted_weights = sorted_weights / self._sum(sorted_weights)
        
        # Find weighted median
        cumsum = self._cumsum(sorted_weights)
        median_idx = self._searchsorted(cumsum, 0.5)
        
        return float(sorted_scores[median_idx])
    
    def _trimmed_mean_consensus(
        self,
        scores: Any,
        outlier_mask: Any,
    ) -> float:
        """Trimmed mean (remove outliers)"""
        clean_scores = scores[~outlier_mask]
        
        if len(clean_scores) == 0:
            return float(self._mean(scores))
        
        return float(self._mean(clean_scores))
    
    def _stake_weighted_consensus(
        self,
        scores: Any,
        stakes: Any,
    ) -> float:
        """Stake-weighted average"""
        total_stake = self._sum(stakes)
        
        if total_stake == 0:
            return float(self._mean(scores))
        
        weighted_sum = self._sum(scores * stakes)
        return float(weighted_sum / total_stake)
    
    def _confidence_weighted_consensus(
        self,
        scores: Any,
        confidences: Any,
    ) -> float:
        """Confidence-weighted average"""
        total_confidence = self._sum(confidences)
        
        if total_confidence == 0:
            return float(self._mean(scores))
        
        weighted_sum = self._sum(scores * confidences)
        return float(weighted_sum / total_confidence)
    
    def _robust_consensus(
        self,
        scores: Any,
        confidences: Any,
        stakes: Any,
        outlier_mask: Any,
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
            return float(self._mean(scores))
        
        # Combine confidence and stake weights
        weights = clean_confidences * self._sqrt(clean_stakes)
        total_weight = self._sum(weights)
        
        if total_weight == 0:
            return float(self._mean(clean_scores))
        
        weighted_sum = self._sum(clean_scores * weights)
        return float(weighted_sum / total_weight)
    
    def _estimate_consensus_confidence(
        self,
        scores: Any,
        confidences: Any,
        outlier_mask: Any,
    ) -> float:
        """
        Estimate confidence in consensus.
        
        Higher confidence when:
        - Low variance in scores
        - High average validator confidence
        - Few outliers
        """
        # Score agreement (low variance = high confidence)
        score_std = self._std(scores)
        agreement = 1.0 - min(score_std * 2, 0.5)  # Normalize variance to confidence
        
        # Average validator confidence
        avg_confidence = float(self._mean(confidences))
        
        # Outlier penalty
        outlier_ratio = self._sum(outlier_mask) / len(scores)
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
    
    # Helper methods for numpy compatibility
    def _to_array(self, data):
        """Convert to array"""
        if HAS_NUMPY:
            return np.array(data)
        return list(data)
    
    def _median(self, data):
        """Calculate median"""
        if HAS_NUMPY:
            return np.median(data)
        return statistics.median(data)
    
    def _sum(self, data):
        """Calculate sum"""
        return sum(data)
    
    def _std(self, data):
        """Calculate standard deviation"""
        if HAS_NUMPY:
            return np.std(data)
        return statistics.stdev(data) if len(data) > 1 else 0
    
    def _min(self, data):
        """Calculate minimum"""
        return min(data)
    
    def _max(self, data):
        """Calculate maximum"""
        return max(data)
    
    def _mean(self, data):
        """Calculate mean"""
        if HAS_NUMPY:
            return np.mean(data)
        return statistics.mean(data)
    
    def _abs(self, data):
        """Calculate absolute values"""
        if HAS_NUMPY:
            return np.abs(data)
        return [abs(x) for x in data]
    
    def _argsort(self, data):
        """Get indices that would sort array"""
        if HAS_NUMPY:
            return np.argsort(data)
        return sorted(range(len(data)), key=lambda i: data[i])
    
    def _cumsum(self, data):
        """Calculate cumulative sum"""
        if HAS_NUMPY:
            return np.cumsum(data)
        result = []
        total = 0
        for x in data:
            total += x
            result.append(total)
        return result
    
    def _searchsorted(self, data, value):
        """Find index where value should be inserted"""
        if HAS_NUMPY:
            return np.searchsorted(data, value)
        for i, x in enumerate(data):
            if x >= value:
                return i
        return len(data)
    
    def _sqrt(self, data):
        """Calculate square root"""
        if HAS_NUMPY:
            return np.sqrt(data)
        if isinstance(data, (list, tuple)):
            return [x ** 0.5 for x in data]
        return data ** 0.5
