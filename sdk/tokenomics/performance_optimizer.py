"""
Performance Optimization Module for ModernTensor Tokenomics

This module provides caching, optimization, and performance monitoring
for tokenomics calculations and operations.

Month 2 - Week 1-2: Performance Optimization
"""

import asyncio
import time
from dataclasses import dataclass, field
from typing import Dict, Any, Optional, List, Tuple
from functools import lru_cache, wraps
import hashlib
import json
from collections import OrderedDict


@dataclass
class CacheConfig:
    """Configuration for caching system."""
    max_size: int = 1000
    ttl_seconds: int = 300  # 5 minutes
    enable_metrics: bool = True


@dataclass
class CacheMetrics:
    """Metrics for cache performance."""
    hits: int = 0
    misses: int = 0
    evictions: int = 0
    total_requests: int = 0
    
    @property
    def hit_rate(self) -> float:
        """Calculate cache hit rate."""
        if self.total_requests == 0:
            return 0.0
        return (self.hits / self.total_requests) * 100.0


class TTLCache:
    """
    Time-To-Live cache with LRU eviction.
    
    Features:
    - TTL expiration
    - LRU eviction when full
    - Thread-safe operations
    - Performance metrics
    """
    
    def __init__(self, config: CacheConfig):
        self.config = config
        self.cache: OrderedDict[str, Tuple[Any, float]] = OrderedDict()
        self.metrics = CacheMetrics()
        self._lock = asyncio.Lock()
    
    async def get(self, key: str) -> Optional[Any]:
        """
        Get value from cache.
        
        Args:
            key: Cache key
            
        Returns:
            Cached value or None if not found/expired
        """
        async with self._lock:
            self.metrics.total_requests += 1
            
            if key not in self.cache:
                self.metrics.misses += 1
                return None
            
            value, expiry = self.cache[key]
            
            # Check if expired
            if time.time() > expiry:
                del self.cache[key]
                self.metrics.misses += 1
                return None
            
            # Move to end (most recently used)
            self.cache.move_to_end(key)
            self.metrics.hits += 1
            return value
    
    async def set(self, key: str, value: Any, ttl: Optional[int] = None) -> None:
        """
        Set value in cache.
        
        Args:
            key: Cache key
            value: Value to cache
            ttl: Time to live in seconds (optional, uses config default)
        """
        async with self._lock:
            # Evict oldest if at capacity
            if len(self.cache) >= self.config.max_size:
                self.cache.popitem(last=False)
                self.metrics.evictions += 1
            
            ttl = ttl or self.config.ttl_seconds
            expiry = time.time() + ttl
            self.cache[key] = (value, expiry)
    
    async def invalidate(self, key: str) -> None:
        """Invalidate a specific cache entry."""
        async with self._lock:
            if key in self.cache:
                del self.cache[key]
    
    async def clear(self) -> None:
        """Clear all cache entries."""
        async with self._lock:
            self.cache.clear()
    
    def get_metrics(self) -> CacheMetrics:
        """Get cache performance metrics."""
        return self.metrics


class PerformanceOptimizer:
    """
    Main performance optimization coordinator.
    
    Features:
    - Utility score calculation caching
    - Batch operation optimization
    - Performance profiling
    - Memory-efficient data structures
    """
    
    def __init__(self, cache_config: Optional[CacheConfig] = None):
        self.cache_config = cache_config or CacheConfig()
        self.utility_cache = TTLCache(self.cache_config)
        self.distribution_cache = TTLCache(self.cache_config)
        self.stake_cache = TTLCache(self.cache_config)
        
        # Performance tracking
        self.operation_times: Dict[str, List[float]] = {}
        
    def _generate_cache_key(self, prefix: str, *args, **kwargs) -> str:
        """
        Generate deterministic cache key.
        
        Args:
            prefix: Key prefix
            *args: Positional arguments
            **kwargs: Keyword arguments
            
        Returns:
            Cache key string
        """
        data = {
            'args': args,
            'kwargs': sorted(kwargs.items())
        }
        data_str = json.dumps(data, sort_keys=True)
        hash_str = hashlib.sha256(data_str.encode()).hexdigest()[:16]
        return f"{prefix}:{hash_str}"
    
    async def get_cached_utility_score(
        self,
        task_volume: int,
        avg_task_difficulty: float,
        validator_participation: float
    ) -> Optional[float]:
        """
        Get cached utility score.
        
        Args:
            task_volume: Number of tasks
            avg_task_difficulty: Average difficulty (0-1)
            validator_participation: Participation rate (0-1)
            
        Returns:
            Cached utility score or None
        """
        key = self._generate_cache_key(
            'utility',
            task_volume=task_volume,
            difficulty=avg_task_difficulty,
            participation=validator_participation
        )
        return await self.utility_cache.get(key)
    
    async def cache_utility_score(
        self,
        task_volume: int,
        avg_task_difficulty: float,
        validator_participation: float,
        score: float
    ) -> None:
        """Cache utility score calculation."""
        key = self._generate_cache_key(
            'utility',
            task_volume=task_volume,
            difficulty=avg_task_difficulty,
            participation=validator_participation
        )
        await self.utility_cache.set(key, score)
    
    async def get_cached_distribution(
        self,
        epoch: int,
        total_emission: int
    ) -> Optional[Dict[str, Any]]:
        """
        Get cached reward distribution.
        
        Args:
            epoch: Epoch number
            total_emission: Total emission amount
            
        Returns:
            Cached distribution or None
        """
        key = self._generate_cache_key(
            'distribution',
            epoch=epoch,
            emission=total_emission
        )
        return await self.distribution_cache.get(key)
    
    async def cache_distribution(
        self,
        epoch: int,
        total_emission: int,
        distribution: Dict[str, Any]
    ) -> None:
        """Cache reward distribution calculation."""
        key = self._generate_cache_key(
            'distribution',
            epoch=epoch,
            emission=total_emission
        )
        await self.distribution_cache.set(key, distribution)
    
    def profile_operation(self, operation_name: str):
        """
        Decorator to profile operation performance.
        
        Usage:
            @optimizer.profile_operation('my_operation')
            async def my_function():
                ...
        """
        def decorator(func):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                start = time.perf_counter()
                try:
                    result = await func(*args, **kwargs)
                    return result
                finally:
                    elapsed = time.perf_counter() - start
                    if operation_name not in self.operation_times:
                        self.operation_times[operation_name] = []
                    self.operation_times[operation_name].append(elapsed)
            return wrapper
        return decorator
    
    def get_performance_stats(self) -> Dict[str, Any]:
        """
        Get performance statistics.
        
        Returns:
            Dictionary with performance metrics
        """
        stats = {
            'cache_metrics': {
                'utility': {
                    'hits': self.utility_cache.metrics.hits,
                    'misses': self.utility_cache.metrics.misses,
                    'hit_rate': self.utility_cache.metrics.hit_rate,
                    'evictions': self.utility_cache.metrics.evictions
                },
                'distribution': {
                    'hits': self.distribution_cache.metrics.hits,
                    'misses': self.distribution_cache.metrics.misses,
                    'hit_rate': self.distribution_cache.metrics.hit_rate,
                    'evictions': self.distribution_cache.metrics.evictions
                }
            },
            'operation_timings': {}
        }
        
        # Calculate operation statistics
        for op_name, times in self.operation_times.items():
            if times:
                stats['operation_timings'][op_name] = {
                    'count': len(times),
                    'avg_ms': sum(times) / len(times) * 1000,
                    'min_ms': min(times) * 1000,
                    'max_ms': max(times) * 1000,
                    'total_ms': sum(times) * 1000
                }
        
        return stats
    
    async def invalidate_epoch_caches(self, epoch: int) -> None:
        """
        Invalidate all caches for a specific epoch.
        
        Args:
            epoch: Epoch number to invalidate
        """
        # Clear distribution cache for epoch
        await self.distribution_cache.clear()
        
    async def clear_all_caches(self) -> None:
        """Clear all caches."""
        await self.utility_cache.clear()
        await self.distribution_cache.clear()
        await self.stake_cache.clear()


class BatchOperationOptimizer:
    """
    Optimize batch operations for better performance.
    
    Features:
    - Request batching
    - Parallel processing
    - Memory-efficient streaming
    """
    
    def __init__(self, max_batch_size: int = 100):
        self.max_batch_size = max_batch_size
    
    async def batch_process(
        self,
        items: List[Any],
        processor: callable,
        max_concurrent: int = 10
    ) -> List[Any]:
        """
        Process items in batches with concurrency control.
        
        Args:
            items: Items to process
            processor: Async function to process each item
            max_concurrent: Maximum concurrent operations
            
        Returns:
            List of processed results
        """
        results = []
        semaphore = asyncio.Semaphore(max_concurrent)
        
        async def process_with_semaphore(item):
            async with semaphore:
                return await processor(item)
        
        # Process in batches
        for i in range(0, len(items), self.max_batch_size):
            batch = items[i:i + self.max_batch_size]
            batch_results = await asyncio.gather(
                *[process_with_semaphore(item) for item in batch],
                return_exceptions=True
            )
            results.extend(batch_results)
        
        return results
    
    def chunk_rewards(
        self,
        rewards: Dict[str, int],
        chunk_size: int = 50
    ) -> List[Dict[str, int]]:
        """
        Split rewards into chunks for batch processing.
        
        Args:
            rewards: Address -> amount mapping
            chunk_size: Size of each chunk
            
        Returns:
            List of reward chunks
        """
        items = list(rewards.items())
        return [
            dict(items[i:i + chunk_size])
            for i in range(0, len(items), chunk_size)
        ]


@lru_cache(maxsize=128)
def calculate_stake_weight(stake: int, total_stake: int) -> float:
    """
    Calculate stake weight with caching.
    
    This is a pure function suitable for LRU caching.
    
    Args:
        stake: Individual stake amount
        total_stake: Total network stake
        
    Returns:
        Stake weight (0-1)
    """
    if total_stake == 0:
        return 0.0
    return stake / total_stake


@lru_cache(maxsize=256)
def calculate_performance_score(
    accuracy: float,
    latency_ms: int,
    uptime: float
) -> float:
    """
    Calculate miner performance score with caching.
    
    Args:
        accuracy: Task accuracy (0-1)
        latency_ms: Response latency in milliseconds
        uptime: Uptime percentage (0-1)
        
    Returns:
        Performance score (0-1)
    """
    # Normalize latency (assume 1000ms is baseline)
    latency_score = max(0, 1.0 - (latency_ms / 1000.0))
    
    # Weighted combination
    score = (
        0.5 * accuracy +
        0.3 * latency_score +
        0.2 * uptime
    )
    
    return max(0.0, min(1.0, score))


class MemoryOptimizer:
    """
    Memory optimization utilities.
    
    Features:
    - Efficient data structures
    - Memory monitoring
    - Garbage collection hints
    """
    
    @staticmethod
    def compress_reward_data(
        rewards: Dict[str, int]
    ) -> bytes:
        """
        Compress reward data for efficient storage.
        
        Args:
            rewards: Reward mapping
            
        Returns:
            Compressed bytes
        """
        import pickle
        import zlib
        data = pickle.dumps(rewards)
        return zlib.compress(data)
    
    @staticmethod
    def decompress_reward_data(
        compressed: bytes
    ) -> Dict[str, int]:
        """
        Decompress reward data.
        
        Args:
            compressed: Compressed bytes
            
        Returns:
            Reward mapping
        """
        import pickle
        import zlib
        data = zlib.decompress(compressed)
        return pickle.loads(data)
