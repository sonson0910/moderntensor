"""
Luxtensor Caching Layer

Provides efficient caching for RPC calls with:
- TTL (Time-To-Live) expiration
- LRU (Least Recently Used) eviction
- Memory and Redis backends
- Automatic cache invalidation on new blocks
"""

import asyncio
import time
import hashlib
import json
import logging
from typing import Optional, Dict, Any, Callable, TypeVar, Generic
from dataclasses import dataclass, field
from collections import OrderedDict
from functools import wraps
from abc import ABC, abstractmethod

logger = logging.getLogger(__name__)

T = TypeVar('T')


@dataclass
class CacheEntry(Generic[T]):
    """Single cache entry with metadata"""
    value: T
    expires_at: float
    block_number: Optional[int] = None
    hit_count: int = 0

    @property
    def is_expired(self) -> bool:
        return time.time() > self.expires_at


class CacheBackend(ABC):
    """Abstract cache backend interface"""

    @abstractmethod
    async def get(self, key: str) -> Optional[Any]:
        """Get value from cache"""
        pass

    @abstractmethod
    async def set(
        self, key: str, value: Any, ttl: int, block_number: Optional[int] = None
    ) -> None:
        """Set value in cache with TTL"""
        pass

    @abstractmethod
    async def delete(self, key: str) -> None:
        """Delete key from cache"""
        pass

    @abstractmethod
    async def clear(self) -> None:
        """Clear entire cache"""
        pass

    @abstractmethod
    async def invalidate_by_block(self, block_number: int) -> int:
        """Invalidate entries older than block. Returns count invalidated."""
        pass


class MemoryCache(CacheBackend):
    """
    In-memory LRU cache with TTL support.

    Suitable for single-process deployments.
    """

    def __init__(self, max_size: int = 10000):
        """
        Initialize memory cache.

        Args:
            max_size: Maximum number of entries (LRU eviction after)
        """
        self.max_size = max_size
        self._cache: OrderedDict[str, CacheEntry] = OrderedDict()
        self._lock = asyncio.Lock()

    async def get(self, key: str) -> Optional[Any]:
        async with self._lock:
            entry = self._cache.get(key)
            if entry is None:
                return None

            if entry.is_expired:
                del self._cache[key]
                return None

            # Move to end (most recently used)
            self._cache.move_to_end(key)
            entry.hit_count += 1
            return entry.value

    async def set(
        self, key: str, value: Any, ttl: int, block_number: Optional[int] = None
    ) -> None:
        async with self._lock:
            # Evict if at capacity
            while len(self._cache) >= self.max_size:
                self._cache.popitem(last=False)  # Remove oldest

            self._cache[key] = CacheEntry(
                value=value,
                expires_at=time.time() + ttl,
                block_number=block_number
            )
            self._cache.move_to_end(key)

    async def delete(self, key: str) -> None:
        async with self._lock:
            self._cache.pop(key, None)

    async def clear(self) -> None:
        async with self._lock:
            self._cache.clear()

    async def invalidate_by_block(self, block_number: int) -> int:
        """Invalidate all entries from blocks older than given block."""
        async with self._lock:
            to_delete = []
            for key, entry in self._cache.items():
                if entry.block_number and entry.block_number < block_number:
                    to_delete.append(key)

            for key in to_delete:
                del self._cache[key]

            return len(to_delete)

    @property
    def size(self) -> int:
        return len(self._cache)

    def stats(self) -> Dict[str, Any]:
        """Get cache statistics."""
        total_hits = sum(e.hit_count for e in self._cache.values())
        return {
            "size": len(self._cache),
            "max_size": self.max_size,
            "total_hits": total_hits,
            "utilization": len(self._cache) / self.max_size if self.max_size > 0 else 0
        }


class RedisCache(CacheBackend):
    """
    Redis-backed cache for distributed deployments.

    Requires redis-py: pip install redis
    """

    def __init__(
        self,
        host: str = "localhost",
        port: int = 6379,
        db: int = 0,
        prefix: str = "luxtensor:"
    ):
        """
        Initialize Redis cache.

        Args:
            host: Redis host
            port: Redis port
            db: Redis database number
            prefix: Key prefix for namespacing
        """
        self.prefix = prefix
        self._redis = None
        self._host = host
        self._port = port
        self._db = db

    async def _get_redis(self):
        if self._redis is None:
            try:
                import redis.asyncio as redis
                self._redis = redis.Redis(
                    host=self._host,
                    port=self._port,
                    db=self._db,
                    decode_responses=True
                )
            except ImportError:
                raise ImportError("redis-py required: pip install redis")
        return self._redis

    def _make_key(self, key: str) -> str:
        return f"{self.prefix}{key}"

    async def get(self, key: str) -> Optional[Any]:
        redis = await self._get_redis()
        data = await redis.get(self._make_key(key))
        if data:
            try:
                return json.loads(data)
            except json.JSONDecodeError:
                return data
        return None

    async def set(
        self, key: str, value: Any, ttl: int, block_number: Optional[int] = None
    ) -> None:
        redis = await self._get_redis()
        data = json.dumps(value) if not isinstance(value, str) else value
        await redis.setex(self._make_key(key), ttl, data)

        # Store block number for invalidation
        if block_number:
            await redis.setex(
                f"{self._make_key(key)}:block",
                ttl,
                str(block_number)
            )

    async def delete(self, key: str) -> None:
        redis = await self._get_redis()
        await redis.delete(self._make_key(key))
        await redis.delete(f"{self._make_key(key)}:block")

    async def clear(self) -> None:
        redis = await self._get_redis()
        keys = await redis.keys(f"{self.prefix}*")
        if keys:
            await redis.delete(*keys)

    async def invalidate_by_block(self, block_number: int) -> int:
        # Redis doesn't support this efficiently without scanning
        # In production, use Redis Streams or pub/sub for invalidation
        return 0


class LuxtensorCache:
    """
    High-level caching for Luxtensor RPC calls.

    Usage:
        ```python
        cache = LuxtensorCache()

        # Cached RPC call
        @cache.cached(ttl=60)
        async def get_balance(address: str):
            return await client.get_balance(address)

        # Or manual usage
        result = await cache.get_or_fetch(
            key=f"balance:{address}",
            fetch_fn=lambda: client.get_balance(address),
            ttl=60
        )
        ```
    """

    # Default TTLs for different query types (seconds)
    TTL_BLOCK = 3  # Block data changes frequently
    TTL_TRANSACTION = 3600  # Transaction data is immutable
    TTL_ACCOUNT = 10  # Account balance can change
    TTL_SUBNET = 60  # Subnet params change rarely
    TTL_NEURON = 30  # Neuron weights change periodically
    TTL_STAKE = 30  # Stake amounts change occasionally
    TTL_HYPERPARAMS = 300  # Hyperparameters rarely change

    def __init__(
        self,
        backend: Optional[CacheBackend] = None,
        enabled: bool = True
    ):
        """
        Initialize cache.

        Args:
            backend: Cache backend (default: MemoryCache)
            enabled: Whether caching is enabled
        """
        self.backend = backend or MemoryCache()
        self.enabled = enabled
        self._current_block: Optional[int] = None

    def _make_key(self, method: str, *args, **kwargs) -> str:
        """Generate cache key from method and arguments."""
        key_data = f"{method}:{args}:{sorted(kwargs.items())}"
        return hashlib.md5(key_data.encode()).hexdigest()

    async def get(self, key: str) -> Optional[Any]:
        """Get value from cache."""
        if not self.enabled:
            return None
        return await self.backend.get(key)

    async def set(
        self,
        key: str,
        value: Any,
        ttl: int,
        block_number: Optional[int] = None
    ) -> None:
        """Set value in cache."""
        if not self.enabled:
            return
        await self.backend.set(key, value, ttl, block_number)

    async def delete(self, key: str) -> None:
        """Delete from cache."""
        await self.backend.delete(key)

    async def clear(self) -> None:
        """Clear entire cache."""
        await self.backend.clear()

    async def on_new_block(self, block_number: int) -> None:
        """
        Called when a new block is received.
        Invalidates stale cache entries.
        """
        self._current_block = block_number
        invalidated = await self.backend.invalidate_by_block(block_number - 10)
        if invalidated > 0:
            logger.debug(f"Invalidated {invalidated} cache entries on block {block_number}")

    async def get_or_fetch(
        self,
        key: str,
        fetch_fn: Callable,
        ttl: int,
        block_number: Optional[int] = None
    ) -> Any:
        """
        Get from cache or fetch and cache result.

        Args:
            key: Cache key
            fetch_fn: Async function to call if cache miss
            ttl: Time-to-live in seconds
            block_number: Current block for invalidation
        """
        # Try cache first
        cached = await self.get(key)
        if cached is not None:
            return cached

        # Fetch fresh data
        if asyncio.iscoroutinefunction(fetch_fn):
            result = await fetch_fn()
        else:
            result = fetch_fn()

        # Cache result
        await self.set(key, result, ttl, block_number or self._current_block)

        return result

    def cached(
        self,
        ttl: int,
        key_prefix: Optional[str] = None
    ):
        """
        Decorator for caching async function results.

        Args:
            ttl: Time-to-live in seconds
            key_prefix: Optional key prefix (default: function name)
        """
        def decorator(func: Callable):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                prefix = key_prefix or func.__name__
                key = f"{prefix}:{self._make_key(prefix, *args, **kwargs)}"

                return await self.get_or_fetch(
                    key=key,
                    fetch_fn=lambda: func(*args, **kwargs),
                    ttl=ttl
                )
            return wrapper
        return decorator


# Global cache instance for convenience
_default_cache: Optional[LuxtensorCache] = None


def get_cache() -> LuxtensorCache:
    """Get global cache instance."""
    global _default_cache
    if _default_cache is None:
        _default_cache = LuxtensorCache()
    return _default_cache


def set_cache(cache: LuxtensorCache) -> None:
    """Set global cache instance."""
    global _default_cache
    _default_cache = cache


# Convenience decorator using global cache
def cached(ttl: int = 60, key_prefix: Optional[str] = None):
    """
    Decorator for caching with global cache.

    Usage:
        ```python
        @cached(ttl=60)
        async def get_balance(address: str):
            return await client.get_balance(address)
        ```
    """
    def decorator(func: Callable):
        @wraps(func)
        async def wrapper(*args, **kwargs):
            cache = get_cache()
            prefix = key_prefix or func.__name__
            key = f"{prefix}:{cache._make_key(prefix, *args, **kwargs)}"

            return await cache.get_or_fetch(
                key=key,
                fetch_fn=lambda: func(*args, **kwargs),
                ttl=ttl
            )
        return wrapper
    return decorator
