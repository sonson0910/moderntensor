"""
Advanced Scoring System - Multi-criteria scoring surpassing Bittensor.

Provides sophisticated scoring mechanisms beyond simple consensus:
- Multiple scoring methods
- Weighted criteria
- Ensemble scoring
- Confidence intervals
"""

import logging
from enum import Enum
from typing import Any, Dict, List, Callable
from dataclasses import dataclass

# Try to import numpy, fallback to stdlib if not available
try:
    import numpy as np
    HAS_NUMPY = True
except ImportError:
    HAS_NUMPY = False
    import statistics

from ..core.protocol import Task, Result, Score

logger = logging.getLogger(__name__)


class ScoringMethod(Enum):
    """Scoring method types"""
    SIMPLE = "simple"  # Basic scoring
    WEIGHTED = "weighted"  # Weighted multi-criteria
    ENSEMBLE = "ensemble"  # Ensemble of multiple scorers
    REWARD_MODEL = "reward_model"  # ML-based reward model
    COMPARATIVE = "comparative"  # Compare against reference


@dataclass
class ScoringCriteria:
    """Single scoring criterion"""
    name: str
    weight: float
    scorer_func: Callable[[Task, Result], float]
    min_score: float = 0.0
    max_score: float = 1.0


class AdvancedScorer:
    """
    Advanced multi-criteria scoring system.
    
    Features:
    - Multiple scoring criteria with weights
    - Ensemble scoring
    - Confidence estimation
    - Score normalization
    - Outlier detection
    
    Example:
        scorer = AdvancedScorer(method=ScoringMethod.WEIGHTED)
        
        # Add criteria
        scorer.add_criterion("quality", weight=0.4, scorer_func=quality_scorer)
        scorer.add_criterion("speed", weight=0.3, scorer_func=speed_scorer)
        scorer.add_criterion("relevance", weight=0.3, scorer_func=relevance_scorer)
        
        # Score result
        score = scorer.score(task, result)
    """
    
    def __init__(
        self,
        method: ScoringMethod = ScoringMethod.WEIGHTED,
        normalize: bool = True,
    ):
        """
        Initialize scorer.
        
        Args:
            method: Scoring method to use
            normalize: Whether to normalize scores
        """
        self.method = method
        self.normalize = normalize
        self.criteria: List[ScoringCriteria] = []
        
        logger.info(f"AdvancedScorer initialized with method: {method}")
    
    def add_criterion(
        self,
        name: str,
        weight: float,
        scorer_func: Callable[[Task, Result], float],
        min_score: float = 0.0,
        max_score: float = 1.0,
    ) -> None:
        """
        Add scoring criterion.
        
        Args:
            name: Criterion name
            weight: Weight in final score (0-1)
            scorer_func: Function that scores task/result
            min_score: Minimum possible score
            max_score: Maximum possible score
        """
        criterion = ScoringCriteria(
            name=name,
            weight=weight,
            scorer_func=scorer_func,
            min_score=min_score,
            max_score=max_score,
        )
        self.criteria.append(criterion)
        
        logger.debug(f"Added criterion: {name} (weight={weight})")
    
    def score(
        self,
        task: Task,
        result: Result,
        return_breakdown: bool = False,
    ) -> Score:
        """
        Score a result using configured criteria.
        
        Args:
            task: Original task
            result: Result to score
            return_breakdown: Include per-criterion scores in metadata
        
        Returns:
            Score object with value and confidence
        """
        if not self.criteria:
            logger.warning("No criteria defined, returning default score")
            return Score(value=0.5, confidence=0.5)
        
        # Score each criterion
        criterion_scores = {}
        for criterion in self.criteria:
            try:
                raw_score = criterion.scorer_func(task, result)
                
                # Normalize if needed
                if self.normalize:
                    normalized = self._normalize_score(
                        raw_score,
                        criterion.min_score,
                        criterion.max_score,
                    )
                else:
                    normalized = raw_score
                
                criterion_scores[criterion.name] = {
                    "raw": raw_score,
                    "normalized": normalized,
                    "weight": criterion.weight,
                }
                
            except Exception as e:
                logger.error(f"Error scoring criterion {criterion.name}: {e}")
                criterion_scores[criterion.name] = {
                    "raw": 0.0,
                    "normalized": 0.0,
                    "weight": criterion.weight,
                }
        
        # Compute final score
        if self.method == ScoringMethod.WEIGHTED:
            final_score = self._weighted_score(criterion_scores)
        elif self.method == ScoringMethod.ENSEMBLE:
            final_score = self._ensemble_score(criterion_scores)
        else:
            final_score = self._simple_score(criterion_scores)
        
        # Estimate confidence
        confidence = self._estimate_confidence(criterion_scores)
        
        # Prepare metadata
        metadata = {
            "method": self.method.value,
            "num_criteria": len(self.criteria),
        }
        
        if return_breakdown:
            metadata["breakdown"] = criterion_scores
        
        return Score(
            value=final_score,
            confidence=confidence,
            metadata=metadata,
        )
    
    def _normalize_score(
        self,
        score: float,
        min_val: float,
        max_val: float,
    ) -> float:
        """Normalize score to [0, 1]"""
        if max_val == min_val:
            return 0.5
        
        normalized = (score - min_val) / (max_val - min_val)
        return max(0.0, min(1.0, normalized))
    
    def _weighted_score(self, criterion_scores: Dict) -> float:
        """Compute weighted average score"""
        total_weight = sum(c["weight"] for c in criterion_scores.values())
        
        if total_weight == 0:
            return 0.0
        
        weighted_sum = sum(
            c["normalized"] * c["weight"]
            for c in criterion_scores.values()
        )
        
        return weighted_sum / total_weight
    
    def _ensemble_score(self, criterion_scores: Dict) -> float:
        """Compute ensemble score (median + weighted average)"""
        scores = [c["normalized"] for c in criterion_scores.values()]
        
        # Median for robustness
        if HAS_NUMPY:
            median = float(np.median(scores))
        else:
            median = statistics.median(scores)
        
        # Weighted average
        weighted_avg = self._weighted_score(criterion_scores)
        
        # Combine (70% weighted, 30% median for outlier resistance)
        return 0.7 * weighted_avg + 0.3 * median
    
    def _simple_score(self, criterion_scores: Dict) -> float:
        """Simple average score"""
        scores = [c["normalized"] for c in criterion_scores.values()]
        if HAS_NUMPY:
            return float(np.mean(scores))
        else:
            return statistics.mean(scores)
    
    def _estimate_confidence(self, criterion_scores: Dict) -> float:
        """
        Estimate confidence based on score variance.
        
        Low variance = high confidence
        High variance = low confidence
        """
        scores = [c["normalized"] for c in criterion_scores.values()]
        
        if len(scores) < 2:
            return 0.8
        
        # Calculate variance
        if HAS_NUMPY:
            variance = float(np.var(scores))
        else:
            variance = statistics.variance(scores)
        
        # Convert to confidence (inverse relationship)
        # variance=0 -> confidence=1.0
        # variance=0.25 -> confidence=0.5
        confidence = 1.0 - min(variance * 2, 0.5)
        
        return max(0.5, min(1.0, confidence))
    
    def get_criteria_summary(self) -> Dict[str, Any]:
        """Get summary of configured criteria"""
        return {
            "method": self.method.value,
            "num_criteria": len(self.criteria),
            "criteria": [
                {
                    "name": c.name,
                    "weight": c.weight,
                    "range": [c.min_score, c.max_score],
                }
                for c in self.criteria
            ],
        }


# Example scoring functions
def length_scorer(task: Task, result: Result) -> float:
    """Score based on output length"""
    expected = task.task_data.get("max_length", 100)
    actual = len(result.result_data.get("text", "").split())
    return min(actual / expected, 1.0)


def speed_scorer(task: Task, result: Result) -> float:
    """Score based on execution speed"""
    exec_time = result.execution_time or 1.0
    target_time = task.task_data.get("target_time", 5.0)
    
    # Faster is better, but diminishing returns
    if exec_time <= target_time:
        return 1.0
    else:
        # Linear penalty after target
        return max(0.0, 1.0 - (exec_time - target_time) / (target_time * 2))


def quality_scorer(task: Task, result: Result) -> float:
    """Score based on output quality (simple keyword matching)"""
    prompt = task.task_data.get("prompt", "")
    text = result.result_data.get("text", "")
    
    # Extract keywords
    prompt_words = set(prompt.lower().split())
    text_words = set(text.lower().split())
    
    # Remove common words
    common = {"the", "a", "an", "in", "on", "at", "to", "for", "of", "and"}
    prompt_kw = prompt_words - common
    text_kw = text_words - common
    
    if not prompt_kw:
        return 0.5
    
    # Calculate overlap
    overlap = len(prompt_kw & text_kw)
    return min(overlap / len(prompt_kw), 1.0)
