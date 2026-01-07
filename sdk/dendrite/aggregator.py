"""
Response aggregation for Dendrite.

Aggregates responses from multiple miners using various strategies.
"""

from typing import List, Dict, Any, Optional, Callable
from collections import Counter
import statistics
import logging

logger = logging.getLogger(__name__)


class ResponseAggregator:
    """Aggregates responses from multiple miners."""
    
    @staticmethod
    def majority_vote(responses: List[Dict[str, Any]], key: str = "result") -> Any:
        """
        Aggregate using majority voting.
        
        Args:
            responses: List of response dictionaries
            key: Key to extract from responses for voting
            
        Returns:
            Most common response value
        """
        if not responses:
            return None
        
        values = [r.get(key) for r in responses if key in r]
        if not values:
            return None
        
        # Use Counter for majority vote
        counter = Counter(values)
        most_common = counter.most_common(1)
        
        if most_common:
            value, count = most_common[0]
            logger.debug(f"Majority vote: {value} ({count}/{len(values)} votes)")
            return value
        
        return None
    
    @staticmethod
    def average(responses: List[Dict[str, Any]], key: str = "result") -> Optional[float]:
        """
        Aggregate using average (for numerical responses).
        
        Args:
            responses: List of response dictionaries
            key: Key to extract from responses
            
        Returns:
            Average of response values
        """
        if not responses:
            return None
        
        values = []
        for r in responses:
            if key in r:
                try:
                    values.append(float(r[key]))
                except (ValueError, TypeError):
                    continue
        
        if not values:
            return None
        
        avg = statistics.mean(values)
        logger.debug(f"Average: {avg} from {len(values)} responses")
        return avg
    
    @staticmethod
    def median(responses: List[Dict[str, Any]], key: str = "result") -> Optional[float]:
        """
        Aggregate using median (for numerical responses).
        
        Args:
            responses: List of response dictionaries
            key: Key to extract from responses
            
        Returns:
            Median of response values
        """
        if not responses:
            return None
        
        values = []
        for r in responses:
            if key in r:
                try:
                    values.append(float(r[key]))
                except (ValueError, TypeError):
                    continue
        
        if not values:
            return None
        
        med = statistics.median(values)
        logger.debug(f"Median: {med} from {len(values)} responses")
        return med
    
    @staticmethod
    def weighted_average(
        responses: List[Dict[str, Any]],
        weights: List[float],
        key: str = "result"
    ) -> Optional[float]:
        """
        Aggregate using weighted average.
        
        Args:
            responses: List of response dictionaries
            weights: Weight for each response
            key: Key to extract from responses
            
        Returns:
            Weighted average of response values
        """
        if not responses or len(responses) != len(weights):
            return None
        
        values = []
        valid_weights = []
        
        for r, w in zip(responses, weights):
            if key in r:
                try:
                    values.append(float(r[key]))
                    valid_weights.append(w)
                except (ValueError, TypeError):
                    continue
        
        if not values:
            return None
        
        weighted_sum = sum(v * w for v, w in zip(values, valid_weights))
        total_weight = sum(valid_weights)
        
        if total_weight == 0:
            return None
        
        avg = weighted_sum / total_weight
        logger.debug(f"Weighted average: {avg} from {len(values)} responses")
        return avg
    
    @staticmethod
    def first_valid(responses: List[Dict[str, Any]], key: str = "result") -> Any:
        """
        Return first valid response.
        
        Args:
            responses: List of response dictionaries
            key: Key to extract from responses
            
        Returns:
            First valid response value
        """
        for r in responses:
            if key in r and r[key] is not None:
                return r[key]
        return None
    
    @staticmethod
    def all_responses(responses: List[Dict[str, Any]], key: str = "result") -> List[Any]:
        """
        Return all response values.
        
        Args:
            responses: List of response dictionaries
            key: Key to extract from responses
            
        Returns:
            List of all response values
        """
        return [r.get(key) for r in responses if key in r]
    
    @staticmethod
    def consensus(
        responses: List[Dict[str, Any]],
        key: str = "result",
        threshold: float = 0.66
    ) -> Optional[Any]:
        """
        Aggregate using consensus (requires threshold agreement).
        
        Args:
            responses: List of response dictionaries
            key: Key to extract from responses
            threshold: Minimum fraction of responses that must agree (0-1)
            
        Returns:
            Consensus value if threshold met, None otherwise
        """
        if not responses:
            return None
        
        values = [r.get(key) for r in responses if key in r]
        if not values:
            return None
        
        counter = Counter(values)
        most_common = counter.most_common(1)
        
        if most_common:
            value, count = most_common[0]
            agreement = count / len(values)
            
            if agreement >= threshold:
                logger.debug(
                    f"Consensus reached: {value} "
                    f"({count}/{len(values)} = {agreement:.2%})"
                )
                return value
            else:
                logger.debug(
                    f"Consensus not reached: best agreement = {agreement:.2%} "
                    f"(threshold = {threshold:.2%})"
                )
        
        return None
    
    @staticmethod
    def custom(
        responses: List[Dict[str, Any]],
        aggregation_func: Callable[[List[Dict[str, Any]]], Any]
    ) -> Any:
        """
        Aggregate using custom function.
        
        Args:
            responses: List of response dictionaries
            aggregation_func: Custom aggregation function
            
        Returns:
            Result of custom aggregation
        """
        return aggregation_func(responses)
    
    @classmethod
    def aggregate(
        cls,
        responses: List[Dict[str, Any]],
        strategy: str = "majority",
        key: str = "result",
        **kwargs
    ) -> Any:
        """
        Aggregate responses using specified strategy.
        
        Args:
            responses: List of response dictionaries
            strategy: Aggregation strategy name
            key: Key to extract from responses
            **kwargs: Additional arguments for specific strategies
            
        Returns:
            Aggregated result
        """
        if not responses:
            return None
        
        strategies = {
            "majority": cls.majority_vote,
            "average": cls.average,
            "median": cls.median,
            "weighted_average": cls.weighted_average,
            "first": cls.first_valid,
            "all": cls.all_responses,
            "consensus": cls.consensus,
        }
        
        if strategy not in strategies:
            logger.warning(f"Unknown strategy '{strategy}', using 'majority'")
            strategy = "majority"
        
        func = strategies[strategy]
        
        try:
            # Pass key and any additional kwargs
            if strategy == "weighted_average":
                weights = kwargs.get("weights", [1.0] * len(responses))
                return func(responses, weights, key)
            elif strategy == "consensus":
                threshold = kwargs.get("threshold", 0.66)
                return func(responses, key, threshold)
            else:
                return func(responses, key)
        except Exception as e:
            logger.error(f"Error in aggregation: {e}")
            return None
