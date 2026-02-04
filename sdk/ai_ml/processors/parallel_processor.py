"""
Parallel Processor - Multi-worker task processing.

Surpasses Bittensor with:
- Concurrent task processing
- Worker pool management
- Load balancing
"""

import asyncio
import logging
from typing import Callable, List
from concurrent.futures import ThreadPoolExecutor, ProcessPoolExecutor

from ..core.protocol import Task, Result

logger = logging.getLogger(__name__)


class ParallelProcessor:
    """
    Parallel task processor with worker pools.
    
    Example:
        processor = ParallelProcessor(num_workers=4)
        results = await processor.process_parallel(tasks, process_func)
    """
    
    def __init__(self, num_workers: int = 4, use_processes: bool = False):
        """
        Initialize parallel processor.
        
        Args:
            num_workers: Number of worker threads/processes
            use_processes: Use processes instead of threads (for CPU-bound tasks)
        """
        self.num_workers = num_workers
        self.use_processes = use_processes
        
        if use_processes:
            self.executor = ProcessPoolExecutor(max_workers=num_workers)
        else:
            self.executor = ThreadPoolExecutor(max_workers=num_workers)
        
        logger.info(f"ParallelProcessor initialized with {num_workers} workers")
    
    async def process_parallel(
        self,
        tasks: List[Task],
        process_func: Callable[[Task], Result],
    ) -> List[Result]:
        """
        Process tasks in parallel.
        
        Args:
            tasks: List of tasks
            process_func: Function to process single task
        
        Returns:
            List of results
        """
        loop = asyncio.get_event_loop()
        
        # Submit all tasks to executor
        futures = [
            loop.run_in_executor(self.executor, process_func, task)
            for task in tasks
        ]
        
        # Wait for all to complete
        results = await asyncio.gather(*futures)
        
        return results
    
    def shutdown(self):
        """Shutdown executor"""
        self.executor.shutdown(wait=True)
        logger.info("ParallelProcessor shutdown")
