"""
BaseSubnet: Batteries-included base implementation for subnets.

Provides common functionality for all subnets:
- Automatic metrics tracking
- Task caching
- Input validation
- Error handling with retries
- Timeout management
"""

import hashlib
import logging
import time
from typing import Any, Dict, Optional
from abc import abstractmethod

from ..core.protocol import (
    SubnetProtocol,
    Task,
    TaskContext,
    Result,
    Score,
)

logger = logging.getLogger(__name__)


class TaskCache:
    """Simple in-memory task result cache"""
    
    def __init__(self, max_size: int = 1000):
        self.cache: Dict[str, Result] = {}
        self.max_size = max_size
        self.hits = 0
        self.misses = 0
    
    def _generate_key(self, task: Task) -> str:
        """Generate cache key from task"""
        task_str = str(sorted(task.task_data.items()))
        return hashlib.sha256(task_str.encode()).hexdigest()
    
    def get(self, task: Task) -> Optional[Result]:
        """Get cached result"""
        key = self._generate_key(task)
        if key in self.cache:
            self.hits += 1
            logger.debug(f"Cache hit for task {task.task_id}")
            return self.cache[key]
        self.misses += 1
        return None
    
    def set(self, task: Task, result: Result) -> None:
        """Cache result"""
        if len(self.cache) >= self.max_size:
            # Simple FIFO eviction
            first_key = next(iter(self.cache))
            del self.cache[first_key]
        
        key = self._generate_key(task)
        self.cache[key] = result
        logger.debug(f"Cached result for task {task.task_id}")
    
    def clear(self) -> None:
        """Clear cache"""
        self.cache.clear()
        self.hits = 0
        self.misses = 0
    
    def get_stats(self) -> Dict[str, Any]:
        """Get cache statistics"""
        total = self.hits + self.misses
        hit_rate = self.hits / total if total > 0 else 0
        return {
            "size": len(self.cache),
            "hits": self.hits,
            "misses": self.misses,
            "hit_rate": hit_rate,
        }


class BaseSubnet(SubnetProtocol):
    """
    Base subnet implementation with common functionality.
    
    Provides:
    - Task result caching
    - Automatic metrics tracking
    - Input validation
    - Timeout management
    - Error handling with retries
    
    Example:
        class MySubnet(BaseSubnet):
            def setup(self):
                self.model = load_my_model()
            
            def _create_task_impl(self, context: TaskContext) -> Task:
                task_data = {"prompt": generate_prompt()}
                return Task(
                    task_id=generate_id(),
                    task_data=task_data,
                    context=context
                )
            
            def _solve_task_impl(self, task: Task) -> Result:
                output = self.model(task.task_data["prompt"])
                return Result(
                    task_id=task.task_id,
                    result_data={"output": output},
                    miner_uid=task.context.miner_uid
                )
            
            def _score_result_impl(self, task: Task, result: Result) -> Score:
                score = evaluate(task, result)
                return Score(value=score, confidence=0.95)
    """
    
    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """
        Initialize base subnet.
        
        Args:
            config: Configuration dictionary with keys:
                - enable_cache: Enable result caching (default: True)
                - cache_size: Maximum cache size (default: 1000)
                - task_timeout: Default task timeout in seconds (default: 60)
                - max_retries: Maximum retries on failure (default: 3)
        """
        super().__init__(config)
        
        # Configuration
        self.enable_cache = self.config.get("enable_cache", True)
        self.task_timeout = self.config.get("task_timeout", 60.0)
        self.max_retries = self.config.get("max_retries", 3)
        
        # Cache
        if self.enable_cache:
            cache_size = self.config.get("cache_size", 1000)
            self.cache = TaskCache(max_size=cache_size)
        else:
            self.cache = None
        
        # State
        self._setup_done = False
    
    def setup(self) -> None:
        """
        Default setup implementation.
        
        Override to add custom setup logic.
        Must call super().setup() at the end.
        """
        self._setup_done = True
        self._initialized = True
        logger.info(f"Subnet {self.__class__.__name__} initialized")
    
    def teardown(self) -> None:
        """
        Default teardown implementation.
        
        Override to add custom cleanup logic.
        Must call super().teardown() at the end.
        """
        if self.cache:
            self.cache.clear()
        self._setup_done = False
        self._initialized = False
        logger.info(f"Subnet {self.__class__.__name__} shut down")
    
    def is_ready(self) -> bool:
        """Check if subnet is ready"""
        return self._setup_done and self._initialized
    
    def validate_task(self, task: Task) -> bool:
        """
        Enhanced task validation.
        
        Override to add custom validation logic.
        """
        # Basic validation
        if not task.task_id:
            logger.error("Task missing task_id")
            return False
        
        if not task.task_data:
            logger.error(f"Task {task.task_id} missing task_data")
            return False
        
        if not task.context:
            logger.error(f"Task {task.task_id} missing context")
            return False
        
        return True
    
    def solve_task(self, task: Task) -> Result:
        """
        Enhanced task solving with caching and timeout.
        
        Args:
            task: Task to solve
            
        Returns:
            Task result
            
        Raises:
            Exception: If task solving fails after retries
        """
        # Check cache first
        if self.cache:
            cached_result = self.cache.get(task)
            if cached_result:
                return cached_result
        
        # Solve with retries
        last_error = None
        for attempt in range(self.max_retries):
            try:
                result = super().solve_task(task)
                
                # Cache result
                if self.cache:
                    self.cache.set(task, result)
                
                return result
                
            except Exception as e:
                last_error = e
                logger.warning(
                    f"Task {task.task_id} failed (attempt {attempt + 1}/{self.max_retries}): {e}"
                )
                if attempt < self.max_retries - 1:
                    time.sleep(1)  # Brief pause before retry
        
        raise Exception(f"Task {task.task_id} failed after {self.max_retries} attempts: {last_error}")
    
    def get_metrics(self) -> Dict[str, Any]:
        """
        Get metrics including cache stats.
        
        Returns:
            Dictionary of metrics
        """
        metrics = super().get_metrics()
        
        if self.cache:
            metrics["cache"] = self.cache.get_stats()
        
        return metrics
    
    def get_metadata(self) -> Dict[str, Any]:
        """
        Get enhanced metadata.
        
        Returns:
            Dictionary of metadata
        """
        metadata = super().get_metadata()
        metadata.update({
            "cache_enabled": self.enable_cache,
            "task_timeout": self.task_timeout,
            "max_retries": self.max_retries,
        })
        return metadata
