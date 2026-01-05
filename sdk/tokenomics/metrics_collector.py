"""
Network Metrics Collector for tokenomics utility score calculation.

This module collects network metrics used to determine adaptive emission rates.
"""

from dataclasses import dataclass, field
from typing import Optional


@dataclass
class NetworkMetrics:
    """
    Network metrics for an epoch.
    
    Attributes:
        task_count: Number of tasks completed
        total_difficulty: Sum of task difficulties
        avg_difficulty: Average task difficulty (0.0-1.0)
        active_validators: Number of active validators
        total_validators: Total number of validators
        validator_ratio: Active validators / Total validators
    """
    task_count: int = 0
    total_difficulty: float = 0.0
    avg_difficulty: float = 0.0
    active_validators: int = 0
    total_validators: int = 0
    validator_ratio: float = 0.0


class NetworkMetricsCollector:
    """
    Collects network metrics for utility score calculation.
    
    This class tracks:
    - Task submission and completion
    - Task difficulty metrics
    - Validator participation
    """
    
    def __init__(self):
        """Initialize metrics collector."""
        self.current_epoch_metrics = NetworkMetrics()
        self.historical_metrics = []
    
    def record_task_submission(
        self,
        difficulty: float = 0.5
    ) -> None:
        """
        Record AI task submission.
        
        Args:
            difficulty: Task difficulty (0.0 to 1.0)
        """
        if not 0.0 <= difficulty <= 1.0:
            raise ValueError(f"Difficulty must be between 0.0 and 1.0, got {difficulty}")
        
        self.current_epoch_metrics.task_count += 1
        self.current_epoch_metrics.total_difficulty += difficulty
    
    def record_validator_participation(
        self,
        participated: bool
    ) -> None:
        """
        Record validator participation in consensus.
        
        Args:
            participated: Whether validator participated
        """
        if participated:
            self.current_epoch_metrics.active_validators += 1
    
    def set_validator_counts(
        self,
        active: int,
        total: int
    ) -> None:
        """
        Set validator counts directly.
        
        Args:
            active: Number of active validators
            total: Total number of validators
        """
        if active < 0 or total < 0:
            raise ValueError("Validator counts must be non-negative")
        
        if active > total:
            raise ValueError("Active validators cannot exceed total validators")
        
        self.current_epoch_metrics.active_validators = active
        self.current_epoch_metrics.total_validators = total
    
    def get_epoch_metrics(self) -> NetworkMetrics:
        """
        Get metrics for current epoch.
        
        Returns:
            NetworkMetrics with calculated averages
        """
        metrics = self.current_epoch_metrics
        
        # Calculate average difficulty
        if metrics.task_count > 0:
            metrics.avg_difficulty = metrics.total_difficulty / metrics.task_count
        else:
            metrics.avg_difficulty = 0.0
        
        # Calculate validator participation ratio
        if metrics.total_validators > 0:
            metrics.validator_ratio = metrics.active_validators / metrics.total_validators
        else:
            metrics.validator_ratio = 0.0
        
        return metrics
    
    def reset_for_new_epoch(self) -> None:
        """
        Reset metrics for new epoch.
        
        Stores current metrics to history before resetting.
        """
        # Store to history
        self.historical_metrics.append(self.current_epoch_metrics)
        
        # Keep only last 100 epochs
        if len(self.historical_metrics) > 100:
            self.historical_metrics = self.historical_metrics[-100:]
        
        # Reset current metrics
        self.current_epoch_metrics = NetworkMetrics()
    
    def get_historical_metrics(self, epochs: int = 10) -> list:
        """
        Get historical metrics.
        
        Args:
            epochs: Number of epochs to retrieve
            
        Returns:
            List of NetworkMetrics
        """
        return self.historical_metrics[-epochs:]
