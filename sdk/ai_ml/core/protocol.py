"""
Enhanced SubnetProtocol with proper AI/ML support.

This module provides the core protocol interface for AI/ML subnets with:
- Proper lifecycle management (setup/teardown)
- Input/output validation
- Error handling
- Performance metrics
- zkML proof support
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional
from enum import Enum
import time


class TaskStatus(Enum):
    """Status of a task"""
    PENDING = "pending"
    PROCESSING = "processing"
    COMPLETED = "completed"
    FAILED = "failed"
    TIMEOUT = "timeout"


@dataclass
class TaskContext:
    """Context information for task creation"""
    miner_uid: str
    difficulty: float
    subnet_uid: int
    cycle: int
    metadata: Dict[str, Any] = field(default_factory=dict)


@dataclass
class Task:
    """Represents a task to be solved by a miner"""
    task_id: str
    task_data: Dict[str, Any]
    context: TaskContext
    created_at: float = field(default_factory=time.time)
    timeout: Optional[float] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for serialization"""
        return {
            "task_id": self.task_id,
            "task_data": self.task_data,
            "context": {
                "miner_uid": self.context.miner_uid,
                "difficulty": self.context.difficulty,
                "subnet_uid": self.context.subnet_uid,
                "cycle": self.context.cycle,
                "metadata": self.context.metadata,
            },
            "created_at": self.created_at,
            "timeout": self.timeout,
        }


@dataclass
class Result:
    """Represents a result from solving a task"""
    task_id: str
    result_data: Dict[str, Any]
    miner_uid: str
    completed_at: float = field(default_factory=time.time)
    execution_time: Optional[float] = None
    proof: Optional[bytes] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for serialization"""
        return {
            "task_id": self.task_id,
            "result_data": self.result_data,
            "miner_uid": self.miner_uid,
            "completed_at": self.completed_at,
            "execution_time": self.execution_time,
            "proof": self.proof.hex() if self.proof else None,
            "metadata": self.metadata,
        }


@dataclass
class Score:
    """Represents a score for a result"""
    value: float  # Score value between 0.0 and 1.0
    confidence: float = 1.0  # Confidence in the score (0.0 to 1.0)
    metadata: Dict[str, Any] = field(default_factory=dict)
    validator_uid: Optional[str] = None
    timestamp: float = field(default_factory=time.time)
    
    def __post_init__(self):
        """Validate score values"""
        if not 0.0 <= self.value <= 1.0:
            raise ValueError(f"Score value must be between 0.0 and 1.0, got {self.value}")
        if not 0.0 <= self.confidence <= 1.0:
            raise ValueError(f"Confidence must be between 0.0 and 1.0, got {self.confidence}")


class SubnetProtocol(ABC):
    """
    Enhanced abstract base class for AI/ML subnet protocols.
    
    This class provides a complete interface for implementing AI/ML subnets with:
    - Lifecycle management (setup/teardown)
    - Task creation and validation
    - Task solving with proper error handling
    - Result scoring with confidence
    - zkML proof verification support
    - Performance metrics
    
    Example:
        class MySubnet(SubnetProtocol):
            def setup(self):
                self.model = load_model("my_model")
            
            def _create_task_impl(self, context: TaskContext) -> Task:
                return Task(...)
            
            def _solve_task_impl(self, task: Task) -> Result:
                output = self.model(task.task_data["input"])
                return Result(...)
            
            def _score_result_impl(self, task: Task, result: Result) -> Score:
                score = evaluate(task, result)
                return Score(value=score, confidence=0.95)
    """
    
    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """
        Initialize the subnet protocol.
        
        Args:
            config: Optional configuration dictionary
        """
        self.config = config or {}
        self._initialized = False
        self._metrics: Dict[str, List[float]] = {
            "task_creation_time": [],
            "solve_time": [],
            "score_time": [],
        }
    
    # ===========================
    # Lifecycle Management
    # ===========================
    
    @abstractmethod
    def setup(self) -> None:
        """
        Initialize resources (load models, etc.).
        
        Called once before the subnet starts processing tasks.
        Should load models, initialize resources, etc.
        
        Raises:
            Exception: If setup fails
        """
        pass
    
    def teardown(self) -> None:
        """
        Cleanup resources.
        
        Called when the subnet is shutting down.
        Override to cleanup resources if needed.
        """
        pass
    
    def is_ready(self) -> bool:
        """Check if subnet is ready to process tasks"""
        return self._initialized
    
    # ===========================
    # Task Creation
    # ===========================
    
    def create_task(self, context: TaskContext) -> Task:
        """
        Create a task with proper error handling and metrics.
        
        Args:
            context: Task creation context
            
        Returns:
            Created task
            
        Raises:
            Exception: If task creation fails
        """
        start_time = time.time()
        try:
            task = self._create_task_impl(context)
            self._metrics["task_creation_time"].append(time.time() - start_time)
            return task
        except Exception as e:
            raise Exception(f"Task creation failed: {e}") from e
    
    @abstractmethod
    def _create_task_impl(self, context: TaskContext) -> Task:
        """
        Actual task creation implementation.
        
        Subclasses must implement this method.
        
        Args:
            context: Task creation context
            
        Returns:
            Created task
        """
        pass
    
    def validate_task(self, task: Task) -> bool:
        """
        Validate task before processing.
        
        Override to implement custom validation logic.
        
        Args:
            task: Task to validate
            
        Returns:
            True if task is valid, False otherwise
        """
        return True
    
    # ===========================
    # Task Solving
    # ===========================
    
    def solve_task(self, task: Task) -> Result:
        """
        Solve task with proper error handling and metrics.
        
        Args:
            task: Task to solve
            
        Returns:
            Task result
            
        Raises:
            Exception: If task solving fails
        """
        if not self.validate_task(task):
            raise ValueError(f"Invalid task: {task.task_id}")
        
        start_time = time.time()
        try:
            result = self._solve_task_impl(task)
            execution_time = time.time() - start_time
            result.execution_time = execution_time
            self._metrics["solve_time"].append(execution_time)
            return result
        except Exception as e:
            raise Exception(f"Task solving failed: {e}") from e
    
    @abstractmethod
    def _solve_task_impl(self, task: Task) -> Result:
        """
        Actual task solving implementation.
        
        Subclasses must implement this method.
        
        Args:
            task: Task to solve
            
        Returns:
            Task result
        """
        pass
    
    # ===========================
    # Result Scoring
    # ===========================
    
    def score_result(self, task: Task, result: Result) -> Score:
        """
        Score result with proper error handling and metrics.
        
        Args:
            task: Original task
            result: Result to score
            
        Returns:
            Score for the result
            
        Raises:
            Exception: If scoring fails
        """
        start_time = time.time()
        try:
            score = self._score_result_impl(task, result)
            self._metrics["score_time"].append(time.time() - start_time)
            return score
        except Exception as e:
            raise Exception(f"Result scoring failed: {e}") from e
    
    @abstractmethod
    def _score_result_impl(self, task: Task, result: Result) -> Score:
        """
        Actual result scoring implementation.
        
        Subclasses must implement this method.
        
        Args:
            task: Original task
            result: Result to score
            
        Returns:
            Score for the result
        """
        pass
    
    # ===========================
    # zkML Proof Support
    # ===========================
    
    def verify_proof(self, task: Task, result: Result, proof: bytes) -> bool:
        """
        Verify zkML proof for a result.
        
        Override to implement proof verification.
        
        Args:
            task: Original task
            result: Result with proof
            proof: zkML proof to verify
            
        Returns:
            True if proof is valid, False otherwise
        """
        # Default: no verification (override in subclass)
        return True
    
    def requires_proof(self) -> bool:
        """
        Check if this subnet requires zkML proofs.
        
        Override to return True if proofs are required.
        
        Returns:
            True if proofs are required, False otherwise
        """
        return False
    
    # ===========================
    # Metrics
    # ===========================
    
    def get_metrics(self) -> Dict[str, Any]:
        """
        Get performance metrics for this subnet.
        
        Returns:
            Dictionary of metrics
        """
        metrics = {}
        for key, values in self._metrics.items():
            if values:
                metrics[f"{key}_avg"] = sum(values) / len(values)
                metrics[f"{key}_min"] = min(values)
                metrics[f"{key}_max"] = max(values)
                metrics[f"{key}_count"] = len(values)
        return metrics
    
    def reset_metrics(self) -> None:
        """Reset all metrics"""
        for key in self._metrics:
            self._metrics[key] = []
    
    # ===========================
    # Metadata
    # ===========================
    
    def get_metadata(self) -> Dict[str, Any]:
        """
        Get metadata about this subnet.
        
        Override to provide custom metadata.
        
        Returns:
            Dictionary of metadata
        """
        return {
            "name": self.__class__.__name__,
            "version": "0.1.0",
            "description": "Base subnet protocol",
            "requires_proof": self.requires_proof(),
        }
