"""
Advanced task processors - Surpassing Bittensor with batching and parallel processing.

Features that Bittensor doesn't have:
- Automatic batching for efficiency
- Parallel task processing
- Dynamic batch size optimization
- Priority-based task scheduling
- Task queue management
"""

from .batch_processor import BatchProcessor, BatchConfig
from .parallel_processor import ParallelProcessor
from .queue_manager import TaskQueue, QueueConfig

__all__ = [
    "BatchProcessor",
    "BatchConfig",
    "ParallelProcessor",
    "TaskQueue",
    "QueueConfig",
]
