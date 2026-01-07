"""
Task Queue Manager - Priority-based task scheduling.

Surpasses Bittensor with:
- Priority queue management
- Task scheduling
- Queue monitoring
"""

import asyncio
import logging
from dataclasses import dataclass
from typing import Optional
from collections import deque
import heapq
import itertools

from ..core.protocol import Task

logger = logging.getLogger(__name__)


@dataclass
class QueueConfig:
    """Queue configuration"""
    max_size: int = 1000
    enable_priority: bool = True


class TaskQueue:
    """
    Priority-based task queue.
    
    Example:
        queue = TaskQueue(QueueConfig(enable_priority=True))
        await queue.put(task, priority=1)
        task = await queue.get()
    """
    
    def __init__(self, config: QueueConfig):
        """Initialize queue"""
        self.config = config
        self.tasks = [] if config.enable_priority else deque()
        self.lock = asyncio.Lock()
        self.not_empty = asyncio.Condition(self.lock)
        self.size = 0
        self.counter = itertools.count()  # For tie-breaking in priority queue
        
        logger.info(f"TaskQueue initialized: {config}")
    
    async def put(self, task: Task, priority: int = 0) -> None:
        """
        Add task to queue.
        
        Args:
            task: Task to add
            priority: Priority (lower = higher priority)
        """
        async with self.not_empty:
            if self.size >= self.config.max_size:
                raise RuntimeError("Queue is full")
            
            if self.config.enable_priority:
                # Use counter for tie-breaking to avoid Task comparison
                count = next(self.counter)
                heapq.heappush(self.tasks, (priority, count, task))
            else:
                self.tasks.append(task)
            
            self.size += 1
            self.not_empty.notify()
    
    async def get(self) -> Task:
        """Get next task from queue"""
        async with self.not_empty:
            while self.size == 0:
                await self.not_empty.wait()
            
            if self.config.enable_priority:
                _, _, task = heapq.heappop(self.tasks)
            else:
                task = self.tasks.popleft()
            
            self.size -= 1
            return task
    
    def qsize(self) -> int:
        """Get queue size"""
        return self.size
    
    def empty(self) -> bool:
        """Check if queue is empty"""
        return self.size == 0

