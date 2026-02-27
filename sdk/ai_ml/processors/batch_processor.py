"""
Batch Processor - Advanced batching for efficient task processing.

This surpasses Bittensor's capabilities with:
- Automatic batching for better GPU utilization
- Dynamic batch size optimization
- Batch timeout management
- Performance metrics per batch
"""

import asyncio
import logging
import time
from dataclasses import dataclass
from typing import Any, Dict, List, Callable
from collections import deque

from ..core.protocol import Task, Result

logger = logging.getLogger(__name__)


@dataclass
class BatchConfig:
    """Configuration for batch processing"""
    max_batch_size: int = 32
    min_batch_size: int = 1
    batch_timeout_ms: float = 100.0  # Max wait time to form a batch
    enable_dynamic_batching: bool = True
    target_latency_ms: float = 500.0  # Target latency for auto-tuning


class BatchProcessor:
    """
    Advanced batch processor for efficient task handling.
    
    Features:
    - Automatic batching of tasks
    - Dynamic batch size optimization
    - Timeout-based batch formation
    - Performance tracking
    
    Example:
        config = BatchConfig(max_batch_size=32, batch_timeout_ms=100)
        processor = BatchProcessor(config, process_func=my_batch_function)
        
        # Submit tasks
        results = await processor.process(tasks)
    """
    
    def __init__(
        self,
        config: BatchConfig,
        process_func: Callable[[List[Task]], List[Result]],
    ):
        """
        Initialize batch processor.
        
        Args:
            config: Batch configuration
            process_func: Function to process a batch of tasks
                         Signature: (tasks: List[Task]) -> List[Result]
        """
        self.config = config
        self.process_func = process_func
        
        # Task queue
        self.queue: deque = deque()
        self.queue_lock = asyncio.Lock()
        
        # Batch formation
        self.current_batch: List[Task] = []
        self.batch_ready_event = asyncio.Event()
        
        # Performance metrics
        self.total_tasks = 0
        self.total_batches = 0
        self.batch_sizes: List[int] = []
        self.batch_latencies: List[float] = []
        
        # Dynamic batching
        self.current_batch_size = config.max_batch_size
        
        logger.info("BatchProcessor initialized with config: %s", config)
    
    async def process(self, tasks: List[Task]) -> List[Result]:
        """
        Process tasks with automatic batching.
        
        Args:
            tasks: List of tasks to process
        
        Returns:
            List of results
        """
        if not tasks:
            return []
        
        # If tasks fit in one batch, process directly
        if len(tasks) <= self.config.max_batch_size:
            return await self._process_batch(tasks)
        
        # Split into multiple batches
        results = []
        for i in range(0, len(tasks), self.config.max_batch_size):
            batch = tasks[i:i + self.config.max_batch_size]
            batch_results = await self._process_batch(batch)
            results.extend(batch_results)
        
        return results
    
    async def process_single(self, task: Task) -> Result:
        """
        Process a single task (may be batched with others).
        
        Args:
            task: Task to process
        
        Returns:
            Result
        """
        results = await self.process([task])
        return results[0] if results else None
    
    async def _process_batch(self, tasks: List[Task]) -> List[Result]:
        """
        Process a batch of tasks.
        
        Args:
            tasks: Batch of tasks
        
        Returns:
            List of results
        """
        if not tasks:
            return []
        
        start_time = time.time()
        batch_size = len(tasks)
        
        logger.debug("Processing batch of %s tasks", batch_size)
        
        try:
            # Process batch
            results = self.process_func(tasks)
            
            # Track metrics
            latency_ms = (time.time() - start_time) * 1000
            self._update_metrics(batch_size, latency_ms)
            
            # Dynamic batch size adjustment
            if self.config.enable_dynamic_batching:
                self._adjust_batch_size(latency_ms)
            
            logger.debug("Batch processed in %sms", latency_ms:.2f)
            
            return results
        
        except Exception as e:
            logger.error("Batch processing failed: %s", e)
            raise
    
    def _update_metrics(self, batch_size: int, latency_ms: float) -> None:
        """Update performance metrics"""
        self.total_tasks += batch_size
        self.total_batches += 1
        self.batch_sizes.append(batch_size)
        self.batch_latencies.append(latency_ms)
        
        # Keep only recent history
        max_history = 100
        if len(self.batch_sizes) > max_history:
            self.batch_sizes = self.batch_sizes[-max_history:]
            self.batch_latencies = self.batch_latencies[-max_history:]
    
    def _adjust_batch_size(self, latency_ms: float) -> None:
        """
        Dynamically adjust batch size based on latency.
        
        Args:
            latency_ms: Latest batch latency
        """
        target = self.config.target_latency_ms
        
        # If latency too high, decrease batch size
        if latency_ms > target * 1.2:
            self.current_batch_size = max(
                self.config.min_batch_size,
                int(self.current_batch_size * 0.8)
            )
            logger.debug("Decreased batch size to %s", self.current_batch_size)
        
        # If latency too low, increase batch size
        elif latency_ms < target * 0.8:
            self.current_batch_size = min(
                self.config.max_batch_size,
                int(self.current_batch_size * 1.2)
            )
            logger.debug("Increased batch size to %s", self.current_batch_size)
    
    def get_metrics(self) -> Dict[str, Any]:
        """
        Get performance metrics.
        
        Returns:
            Dictionary of metrics
        """
        if not self.batch_latencies:
            return {
                "total_tasks": 0,
                "total_batches": 0,
                "avg_batch_size": 0,
                "avg_latency_ms": 0,
                "throughput_tasks_per_sec": 0,
            }
        
        avg_batch_size = sum(self.batch_sizes) / len(self.batch_sizes)
        avg_latency = sum(self.batch_latencies) / len(self.batch_latencies)
        total_latency = sum(self.batch_latencies)
        
        # Prevent division by zero
        throughput = (self.total_tasks / total_latency) * 1000 if total_latency > 0 else 0  # tasks/sec
        
        return {
            "total_tasks": self.total_tasks,
            "total_batches": self.total_batches,
            "avg_batch_size": avg_batch_size,
            "avg_latency_ms": avg_latency,
            "min_latency_ms": min(self.batch_latencies),
            "max_latency_ms": max(self.batch_latencies),
            "current_batch_size": self.current_batch_size,
            "throughput_tasks_per_sec": throughput,
        }
    
    def reset_metrics(self) -> None:
        """Reset performance metrics"""
        self.total_tasks = 0
        self.total_batches = 0
        self.batch_sizes.clear()
        self.batch_latencies.clear()
        logger.info("Metrics reset")
