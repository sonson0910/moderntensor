"""
Concurrency Optimization Module

Provides utilities for parallel processing, async optimization, and thread pool management.
"""

import asyncio
import concurrent.futures
from typing import List, Callable, Any, TypeVar, Optional, Iterable
from dataclasses import dataclass
import logging
import time

logger = logging.getLogger(__name__)

T = TypeVar('T')
R = TypeVar('R')


@dataclass
class TaskResult:
    """Result of an async task execution."""
    success: bool
    result: Any = None
    error: Optional[Exception] = None
    duration: float = 0.0


class AsyncTaskPool:
    """
    Async task pool for managing concurrent tasks with limits.
    
    Controls concurrency to prevent overwhelming the system.
    
    Examples:
        >>> pool = AsyncTaskPool(max_concurrent=10)
        >>> 
        >>> async def fetch_data(id):
        ...     return await api.get(id)
        >>> 
        >>> tasks = [fetch_data(i) for i in range(100)]
        >>> results = await pool.run_all(tasks)
    """
    
    def __init__(self, max_concurrent: int = 100):
        """
        Initialize async task pool.
        
        Args:
            max_concurrent: Maximum number of concurrent tasks
        """
        self.max_concurrent = max_concurrent
        self._semaphore = asyncio.Semaphore(max_concurrent)
    
    async def run_task(self, coro) -> TaskResult:
        """
        Run a single coroutine with semaphore control.
        
        Args:
            coro: Coroutine to execute
            
        Returns:
            TaskResult with outcome
        """
        start_time = time.time()
        
        async with self._semaphore:
            try:
                result = await coro
                duration = time.time() - start_time
                return TaskResult(
                    success=True,
                    result=result,
                    duration=duration
                )
            except Exception as e:
                duration = time.time() - start_time
                logger.error(f"Task failed: {e}")
                return TaskResult(
                    success=False,
                    error=e,
                    duration=duration
                )
    
    async def run_all(self, coros: List) -> List[TaskResult]:
        """
        Run all coroutines with concurrency control.
        
        Args:
            coros: List of coroutines to execute
            
        Returns:
            List of TaskResults
        """
        tasks = [self.run_task(coro) for coro in coros]
        return await asyncio.gather(*tasks)
    
    async def map(
        self,
        func: Callable,
        items: Iterable[T]
    ) -> List[TaskResult]:
        """
        Map async function over items with concurrency control.
        
        Args:
            func: Async function to apply
            items: Items to process
            
        Returns:
            List of TaskResults
            
        Examples:
            >>> async def process(item):
            ...     return item * 2
            >>> 
            >>> results = await pool.map(process, range(100))
        """
        coros = [func(item) for item in items]
        return await self.run_all(coros)


class ThreadPoolExecutor:
    """
    Enhanced thread pool executor with monitoring.
    
    Wrapper around concurrent.futures.ThreadPoolExecutor with stats.
    
    Examples:
        >>> executor = ThreadPoolExecutor(max_workers=8)
        >>> 
        >>> def cpu_intensive(x):
        ...     return x ** 2
        >>> 
        >>> results = executor.map(cpu_intensive, range(1000))
    """
    
    def __init__(self, max_workers: Optional[int] = None):
        """
        Initialize thread pool.
        
        Args:
            max_workers: Maximum worker threads (None = CPU count * 5)
        """
        self.max_workers = max_workers
        self._executor = concurrent.futures.ThreadPoolExecutor(max_workers=max_workers)
        self._tasks_submitted = 0
        self._tasks_completed = 0
        self._tasks_failed = 0
    
    def submit(self, func: Callable, *args, **kwargs) -> concurrent.futures.Future:
        """
        Submit task to thread pool.
        
        Args:
            func: Function to execute
            *args: Positional arguments
            **kwargs: Keyword arguments
            
        Returns:
            Future object
        """
        self._tasks_submitted += 1
        future = self._executor.submit(func, *args, **kwargs)
        future.add_done_callback(self._on_task_done)
        return future
    
    def map(
        self,
        func: Callable[[T], R],
        iterable: Iterable[T],
        timeout: Optional[float] = None,
        chunksize: int = 1
    ) -> Iterable[R]:
        """
        Map function over iterable using thread pool.
        
        Args:
            func: Function to apply
            iterable: Items to process
            timeout: Timeout for each task
            chunksize: Number of items per task
            
        Returns:
            Iterator of results
        """
        return self._executor.map(func, iterable, timeout=timeout, chunksize=chunksize)
    
    def _on_task_done(self, future: concurrent.futures.Future):
        """Callback when task completes."""
        try:
            future.result()
            self._tasks_completed += 1
        except Exception as e:
            self._tasks_failed += 1
            logger.error(f"Thread pool task failed: {e}")
    
    def shutdown(self, wait: bool = True):
        """
        Shutdown thread pool.
        
        Args:
            wait: Wait for tasks to complete
        """
        self._executor.shutdown(wait=wait)
    
    def get_stats(self) -> dict:
        """Get thread pool statistics."""
        return {
            "max_workers": self.max_workers,
            "submitted": self._tasks_submitted,
            "completed": self._tasks_completed,
            "failed": self._tasks_failed,
            "pending": self._tasks_submitted - self._tasks_completed - self._tasks_failed
        }


class ParallelProcessor:
    """
    Parallel data processor with automatic batching.
    
    Processes data in parallel across multiple workers.
    
    Examples:
        >>> processor = ParallelProcessor(num_workers=4)
        >>> 
        >>> def transform(batch):
        ...     return [x * 2 for x in batch]
        >>> 
        >>> results = processor.process(data, transform, batch_size=100)
    """
    
    def __init__(self, num_workers: Optional[int] = None):
        """
        Initialize parallel processor.
        
        Args:
            num_workers: Number of worker threads (None = CPU count)
        """
        self.num_workers = num_workers
        self._executor = ThreadPoolExecutor(max_workers=num_workers)
    
    def process(
        self,
        data: List[T],
        func: Callable[[List[T]], List[R]],
        batch_size: int = 100
    ) -> List[R]:
        """
        Process data in parallel batches.
        
        Args:
            data: Data to process
            func: Function that processes a batch
            batch_size: Size of each batch
            
        Returns:
            Combined results
        """
        # Split data into batches
        batches = [
            data[i:i + batch_size]
            for i in range(0, len(data), batch_size)
        ]
        
        # Process batches in parallel
        batch_results = list(self._executor.map(func, batches))
        
        # Flatten results
        results = []
        for batch_result in batch_results:
            results.extend(batch_result)
        
        return results
    
    def shutdown(self):
        """Shutdown the processor."""
        self._executor.shutdown()


async def gather_with_concurrency(
    n: int,
    *coros
) -> List[Any]:
    """
    Gather coroutines with concurrency limit.
    
    Like asyncio.gather but with maximum concurrency control.
    
    Args:
        n: Maximum concurrent coroutines
        *coros: Coroutines to execute
        
    Returns:
        List of results
        
    Examples:
        >>> results = await gather_with_concurrency(
        ...     10,
        ...     fetch(1), fetch(2), fetch(3), ...
        ... )
    """
    semaphore = asyncio.Semaphore(n)
    
    async def sem_coro(coro):
        async with semaphore:
            return await coro
    
    return await asyncio.gather(*(sem_coro(c) for c in coros))


async def run_with_timeout(
    coro,
    timeout: float,
    default: Any = None
) -> Any:
    """
    Run coroutine with timeout.
    
    Args:
        coro: Coroutine to run
        timeout: Timeout in seconds
        default: Default value if timeout
        
    Returns:
        Result or default if timeout
        
    Examples:
        >>> result = await run_with_timeout(
        ...     slow_operation(),
        ...     timeout=5.0,
        ...     default="timed out"
        ... )
    """
    try:
        return await asyncio.wait_for(coro, timeout=timeout)
    except asyncio.TimeoutError:
        logger.warning(f"Operation timed out after {timeout}s")
        return default


class AsyncRateLimiter:
    """
    Rate limiter for async operations.
    
    Limits number of operations per time window.
    
    Examples:
        >>> limiter = AsyncRateLimiter(rate=10, per=1.0)  # 10 per second
        >>> 
        >>> async def api_call():
        ...     async with limiter:
        ...         return await client.get("/data")
    """
    
    def __init__(self, rate: int, per: float = 1.0):
        """
        Initialize rate limiter.
        
        Args:
            rate: Number of operations allowed
            per: Time window in seconds
        """
        self.rate = rate
        self.per = per
        self._tokens = rate
        self._updated_at = time.time()
        self._lock = asyncio.Lock()
    
    async def __aenter__(self):
        """Acquire rate limit token."""
        async with self._lock:
            while self._tokens <= 0:
                await self._refill()
                if self._tokens <= 0:
                    await asyncio.sleep(0.1)
            
            self._tokens -= 1
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Exit context."""
        pass
    
    async def _refill(self):
        """Refill tokens based on elapsed time."""
        now = time.time()
        elapsed = now - self._updated_at
        
        # Add tokens based on elapsed time
        tokens_to_add = (elapsed / self.per) * self.rate
        self._tokens = min(self.rate, self._tokens + tokens_to_add)
        self._updated_at = now


def optimize_async_loop():
    """
    Optimize the async event loop for better performance.
    
    Sets up optimal event loop configuration.
    
    Examples:
        >>> optimize_async_loop()
        >>> # Event loop now configured for better performance
    """
    try:
        import uvloop
        asyncio.set_event_loop_policy(uvloop.EventLoopPolicy())
        logger.info("Using uvloop for better async performance")
    except ImportError:
        logger.info("uvloop not available, using default event loop")


class WorkQueue:
    """
    Async work queue for background task processing.
    
    Processes tasks asynchronously in the background.
    
    Examples:
        >>> queue = WorkQueue(num_workers=4)
        >>> await queue.start()
        >>> 
        >>> async def task(data):
        ...     await process(data)
        >>> 
        >>> await queue.add_task(task, {"key": "value"})
    """
    
    def __init__(self, num_workers: int = 4):
        """
        Initialize work queue.
        
        Args:
            num_workers: Number of worker coroutines
        """
        self.num_workers = num_workers
        self._queue: asyncio.Queue = asyncio.Queue()
        self._workers: List[asyncio.Task] = []
        self._running = False
    
    async def start(self):
        """Start worker coroutines."""
        if self._running:
            return
        
        self._running = True
        self._workers = [
            asyncio.create_task(self._worker())
            for _ in range(self.num_workers)
        ]
        logger.info(f"Started {self.num_workers} workers")
    
    async def stop(self):
        """Stop all workers."""
        self._running = False
        
        # Cancel all workers
        for worker in self._workers:
            worker.cancel()
        
        # Wait for cancellation
        await asyncio.gather(*self._workers, return_exceptions=True)
        self._workers.clear()
        logger.info("All workers stopped")
    
    async def add_task(self, coro_func: Callable, *args, **kwargs):
        """
        Add task to queue.
        
        Args:
            coro_func: Async function to execute
            *args: Positional arguments
            **kwargs: Keyword arguments
        """
        await self._queue.put((coro_func, args, kwargs))
    
    async def _worker(self):
        """Worker coroutine that processes tasks."""
        while self._running:
            try:
                # Get task from queue
                coro_func, args, kwargs = await asyncio.wait_for(
                    self._queue.get(),
                    timeout=1.0
                )
                
                # Execute task
                try:
                    await coro_func(*args, **kwargs)
                except Exception as e:
                    logger.error(f"Task execution failed: {e}")
                finally:
                    self._queue.task_done()
            
            except asyncio.TimeoutError:
                continue
            except asyncio.CancelledError:
                break
    
    def qsize(self) -> int:
        """Get current queue size."""
        return self._queue.qsize()
