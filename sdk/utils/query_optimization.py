"""
Query Optimization Module

Provides query caching, batch operations, and connection pooling for improved performance.
"""

import asyncio
import hashlib
import json
import time
from typing import Any, Optional, Dict, List, Callable, TypeVar, Generic
from dataclasses import dataclass, field
from collections import OrderedDict
import logging

logger = logging.getLogger(__name__)

T = TypeVar('T')


@dataclass
class CacheEntry(Generic[T]):
    """Cache entry with value and metadata."""
    value: T
    timestamp: float
    hits: int = 0
    ttl: Optional[float] = None
    
    def is_expired(self) -> bool:
        """Check if cache entry has expired."""
        if self.ttl is None:
            return False
        return (time.time() - self.timestamp) > self.ttl
    
    def is_valid(self) -> bool:
        """Check if cache entry is still valid."""
        return not self.is_expired()


class LRUCache(Generic[T]):
    """
    LRU (Least Recently Used) cache implementation.
    
    Thread-safe cache with TTL support and size limits.
    
    Examples:
        >>> cache = LRUCache(max_size=100, default_ttl=60.0)
        >>> cache.set("key1", "value1")
        >>> value = cache.get("key1")
        >>> print(value)
        value1
    """
    
    def __init__(self, max_size: int = 1000, default_ttl: Optional[float] = None):
        """
        Initialize LRU cache.
        
        Args:
            max_size: Maximum number of entries to store
            default_ttl: Default time-to-live in seconds (None = no expiration)
        """
        self.max_size = max_size
        self.default_ttl = default_ttl
        self._cache: OrderedDict[str, CacheEntry[T]] = OrderedDict()
        self._lock = asyncio.Lock()
        
        # Statistics
        self._hits = 0
        self._misses = 0
        self._evictions = 0
    
    async def get(self, key: str) -> Optional[T]:
        """
        Get value from cache.
        
        Args:
            key: Cache key
            
        Returns:
            Cached value or None if not found/expired
        """
        async with self._lock:
            entry = self._cache.get(key)
            
            if entry is None:
                self._misses += 1
                return None
            
            if not entry.is_valid():
                # Entry expired, remove it
                del self._cache[key]
                self._misses += 1
                return None
            
            # Move to end (most recently used)
            self._cache.move_to_end(key)
            entry.hits += 1
            self._hits += 1
            
            return entry.value
    
    async def set(self, key: str, value: T, ttl: Optional[float] = None) -> None:
        """
        Set value in cache.
        
        Args:
            key: Cache key
            value: Value to cache
            ttl: Time-to-live in seconds (overrides default_ttl)
        """
        async with self._lock:
            # Remove oldest entry if at capacity
            if key not in self._cache and len(self._cache) >= self.max_size:
                self._cache.popitem(last=False)
                self._evictions += 1
            
            # Create cache entry
            entry = CacheEntry(
                value=value,
                timestamp=time.time(),
                ttl=ttl if ttl is not None else self.default_ttl
            )
            
            # Add/update entry
            self._cache[key] = entry
            self._cache.move_to_end(key)
    
    async def delete(self, key: str) -> bool:
        """
        Delete entry from cache.
        
        Args:
            key: Cache key
            
        Returns:
            True if key was found and deleted
        """
        async with self._lock:
            if key in self._cache:
                del self._cache[key]
                return True
            return False
    
    async def clear(self) -> None:
        """Clear all cache entries."""
        async with self._lock:
            self._cache.clear()
            self._hits = 0
            self._misses = 0
            self._evictions = 0
    
    def get_stats(self) -> Dict[str, Any]:
        """
        Get cache statistics.
        
        Returns:
            Dictionary with cache stats
        """
        total_requests = self._hits + self._misses
        hit_rate = self._hits / total_requests if total_requests > 0 else 0.0
        
        return {
            "size": len(self._cache),
            "max_size": self.max_size,
            "hits": self._hits,
            "misses": self._misses,
            "evictions": self._evictions,
            "hit_rate": hit_rate,
            "total_requests": total_requests
        }


class QueryCache:
    """
    Query result cache with automatic key generation.
    
    Caches query results based on query parameters.
    
    Examples:
        >>> cache = QueryCache(max_size=500, default_ttl=300.0)
        >>> 
        >>> @cache.cached(ttl=60.0)
        >>> async def get_user(user_id: int):
        ...     return await db.fetch_user(user_id)
    """
    
    def __init__(self, max_size: int = 1000, default_ttl: Optional[float] = 300.0):
        """
        Initialize query cache.
        
        Args:
            max_size: Maximum cache size
            default_ttl: Default TTL in seconds (5 minutes default)
        """
        self._cache = LRUCache[Any](max_size=max_size, default_ttl=default_ttl)
    
    def _generate_key(self, func_name: str, args: tuple, kwargs: dict) -> str:
        """
        Generate cache key from function name and arguments.
        
        Args:
            func_name: Function name
            args: Positional arguments
            kwargs: Keyword arguments
            
        Returns:
            Cache key string
        """
        # Create a deterministic key from function and arguments
        key_data = {
            "func": func_name,
            "args": args,
            "kwargs": sorted(kwargs.items())
        }
        key_str = json.dumps(key_data, sort_keys=True, default=str)
        key_hash = hashlib.md5(key_str.encode()).hexdigest()
        return f"{func_name}:{key_hash}"
    
    def cached(self, ttl: Optional[float] = None):
        """
        Decorator to cache function results.
        
        Args:
            ttl: Time-to-live for this function's cache (overrides default)
            
        Returns:
            Decorated function
            
        Examples:
            >>> @query_cache.cached(ttl=60.0)
            >>> async def expensive_query(param):
            ...     return await database.query(param)
        """
        def decorator(func: Callable):
            async def wrapper(*args, **kwargs):
                # Generate cache key
                cache_key = self._generate_key(func.__name__, args, kwargs)
                
                # Try to get from cache
                cached_value = await self._cache.get(cache_key)
                if cached_value is not None:
                    logger.debug(f"Cache hit for {func.__name__}")
                    return cached_value
                
                # Cache miss - call function
                logger.debug(f"Cache miss for {func.__name__}")
                result = await func(*args, **kwargs)
                
                # Store in cache
                await self._cache.set(cache_key, result, ttl=ttl)
                
                return result
            
            return wrapper
        return decorator
    
    async def invalidate(self, func_name: str, *args, **kwargs) -> None:
        """
        Invalidate cached result for specific function call.
        
        Args:
            func_name: Function name
            *args: Positional arguments
            **kwargs: Keyword arguments
        """
        cache_key = self._generate_key(func_name, args, kwargs)
        await self._cache.delete(cache_key)
    
    async def clear(self) -> None:
        """Clear all cached queries."""
        await self._cache.clear()
    
    def get_stats(self) -> Dict[str, Any]:
        """Get cache statistics."""
        return self._cache.get_stats()


class BatchProcessor:
    """
    Batch query processor for improved efficiency.
    
    Collects multiple queries and processes them in batches.
    
    Examples:
        >>> processor = BatchProcessor(batch_size=10, timeout=0.1)
        >>> 
        >>> async def fetch_users(user_ids: List[int]):
        ...     return await db.fetch_many(user_ids)
        >>> 
        >>> result = await processor.add(user_id, fetch_users)
    """
    
    def __init__(self, batch_size: int = 50, timeout: float = 0.1):
        """
        Initialize batch processor.
        
        Args:
            batch_size: Maximum batch size before processing
            timeout: Maximum wait time in seconds before processing
        """
        self.batch_size = batch_size
        self.timeout = timeout
        
        self._batch: List[Any] = []
        self._futures: List[asyncio.Future] = []
        self._lock = asyncio.Lock()
        self._task: Optional[asyncio.Task] = None
    
    async def add(self, item: Any, processor: Callable[[List[Any]], List[Any]]) -> Any:
        """
        Add item to batch for processing.
        
        Args:
            item: Item to process
            processor: Async function that processes a batch
            
        Returns:
            Processing result for this item
        """
        async with self._lock:
            future = asyncio.Future()
            self._batch.append(item)
            self._futures.append(future)
            
            # Start timeout task if not already running
            if self._task is None:
                self._task = asyncio.create_task(self._wait_and_process(processor))
            
            # Process immediately if batch is full
            if len(self._batch) >= self.batch_size:
                await self._process_batch(processor)
        
        return await future
    
    async def _wait_and_process(self, processor: Callable):
        """Wait for timeout then process batch."""
        await asyncio.sleep(self.timeout)
        async with self._lock:
            if self._batch:
                await self._process_batch(processor)
    
    async def _process_batch(self, processor: Callable):
        """Process current batch."""
        if not self._batch:
            return
        
        try:
            # Process entire batch
            results = await processor(self._batch)
            
            # Resolve futures with results
            for future, result in zip(self._futures, results):
                if not future.done():
                    future.set_result(result)
        
        except Exception as e:
            # Fail all futures with exception
            for future in self._futures:
                if not future.done():
                    future.set_exception(e)
        
        finally:
            # Clear batch
            self._batch.clear()
            self._futures.clear()
            self._task = None


class ConnectionPool:
    """
    Connection pool for reusing connections.
    
    Manages a pool of connections to avoid connection overhead.
    
    Examples:
        >>> pool = ConnectionPool(min_size=5, max_size=20)
        >>> 
        >>> async with pool.acquire() as conn:
        ...     result = await conn.query("SELECT * FROM users")
    """
    
    def __init__(
        self,
        connector: Callable,
        min_size: int = 5,
        max_size: int = 20,
        timeout: float = 30.0
    ):
        """
        Initialize connection pool.
        
        Args:
            connector: Async function that creates a new connection
            min_size: Minimum pool size
            max_size: Maximum pool size
            timeout: Connection acquisition timeout
        """
        self.connector = connector
        self.min_size = min_size
        self.max_size = max_size
        self.timeout = timeout
        
        self._pool: asyncio.Queue = asyncio.Queue(maxsize=max_size)
        self._size = 0
        self._lock = asyncio.Lock()
        
        # Statistics
        self._acquisitions = 0
        self._releases = 0
        self._created = 0
        self._closed = 0
    
    async def initialize(self):
        """Initialize pool with minimum connections."""
        for _ in range(self.min_size):
            conn = await self.connector()
            await self._pool.put(conn)
            self._size += 1
            self._created += 1
    
    async def acquire(self, timeout: Optional[float] = None):
        """
        Acquire connection from pool.
        
        Args:
            timeout: Acquisition timeout (uses pool timeout if None)
            
        Returns:
            Connection object
        """
        self._acquisitions += 1
        timeout = timeout if timeout is not None else self.timeout
        
        try:
            # Try to get from pool
            conn = await asyncio.wait_for(
                self._pool.get(),
                timeout=timeout
            )
            return conn
        
        except asyncio.TimeoutError:
            # Pool empty and timeout reached
            async with self._lock:
                if self._size < self.max_size:
                    # Create new connection
                    conn = await self.connector()
                    self._size += 1
                    self._created += 1
                    return conn
            
            # Cannot create more connections
            raise ConnectionError("Connection pool exhausted")
    
    async def release(self, conn: Any):
        """
        Release connection back to pool.
        
        Args:
            conn: Connection to release
        """
        self._releases += 1
        await self._pool.put(conn)
    
    async def close(self):
        """Close all connections in pool."""
        while not self._pool.empty():
            conn = await self._pool.get()
            if hasattr(conn, 'close'):
                await conn.close()
            self._closed += 1
            self._size -= 1
    
    def get_stats(self) -> Dict[str, Any]:
        """Get pool statistics."""
        return {
            "size": self._size,
            "min_size": self.min_size,
            "max_size": self.max_size,
            "available": self._pool.qsize(),
            "acquisitions": self._acquisitions,
            "releases": self._releases,
            "created": self._created,
            "closed": self._closed
        }


# Global query cache instance
query_cache = QueryCache(max_size=1000, default_ttl=300.0)
