"""
Memory Optimization Module

Provides utilities for memory profiling, optimization, and monitoring.
"""

import gc
import sys
import weakref
from typing import Any, Dict, List, Optional, Type
from dataclasses import dataclass
import logging

logger = logging.getLogger(__name__)


@dataclass
class MemoryStats:
    """Memory usage statistics."""
    total_bytes: int
    available_bytes: int
    used_bytes: int
    percent_used: float
    gc_stats: Dict[str, Any]


def get_object_size(obj: Any, seen: Optional[set] = None) -> int:
    """
    Calculate deep size of an object including all referenced objects.
    
    Args:
        obj: Object to measure
        seen: Set of already seen object IDs (for recursion)
        
    Returns:
        Total size in bytes
        
    Examples:
        >>> data = {"key": [1, 2, 3], "nested": {"a": "b"}}
        >>> size = get_object_size(data)
        >>> print(f"Size: {size} bytes")
    """
    size = sys.getsizeof(obj)
    
    if seen is None:
        seen = set()
    
    obj_id = id(obj)
    if obj_id in seen:
        return 0
    
    seen.add(obj_id)
    
    # Handle different types
    if isinstance(obj, dict):
        size += sum(get_object_size(k, seen) + get_object_size(v, seen) 
                    for k, v in obj.items())
    elif hasattr(obj, '__dict__'):
        size += get_object_size(obj.__dict__, seen)
    elif hasattr(obj, '__iter__') and not isinstance(obj, (str, bytes, bytearray)):
        try:
            size += sum(get_object_size(item, seen) for item in obj)
        except TypeError:
            pass
    
    return size


def get_memory_stats() -> MemoryStats:
    """
    Get current memory usage statistics.
    
    Returns:
        MemoryStats with current memory info
        
    Examples:
        >>> stats = get_memory_stats()
        >>> print(f"Memory usage: {stats.percent_used:.1f}%")
    """
    try:
        import psutil
        process = psutil.Process()
        mem_info = process.memory_info()
        
        total = psutil.virtual_memory().total
        available = psutil.virtual_memory().available
        used = mem_info.rss
        percent = (used / total) * 100
    except ImportError:
        # Fallback if psutil not available
        import resource
        usage = resource.getrusage(resource.RUSAGE_SELF)
        used = usage.ru_maxrss * 1024  # Convert KB to bytes
        total = 0
        available = 0
        percent = 0.0
    
    # Get GC stats
    gc_counts = gc.get_count()
    gc_stats = {
        "collections": {
            "gen0": gc_counts[0],
            "gen1": gc_counts[1],
            "gen2": gc_counts[2]
        },
        "threshold": gc.get_threshold(),
        "objects": len(gc.get_objects())
    }
    
    return MemoryStats(
        total_bytes=total,
        available_bytes=available,
        used_bytes=used,
        percent_used=percent,
        gc_stats=gc_stats
    )


def optimize_gc():
    """
    Optimize garbage collection settings for better performance.
    
    Adjusts GC thresholds based on current memory usage.
    
    Examples:
        >>> optimize_gc()
        >>> # GC will now be more aggressive or lenient based on memory
    """
    stats = get_memory_stats()
    
    if stats.percent_used > 80:
        # High memory usage - be more aggressive
        gc.set_threshold(500, 5, 5)
        logger.info("Enabled aggressive GC (high memory usage)")
    elif stats.percent_used > 60:
        # Moderate usage - balanced
        gc.set_threshold(700, 10, 10)
        logger.info("Set balanced GC thresholds")
    else:
        # Low usage - be lenient
        gc.set_threshold(1000, 15, 15)
        logger.info("Set lenient GC thresholds")


def force_gc() -> Dict[str, int]:
    """
    Force full garbage collection and return stats.
    
    Returns:
        Dictionary with collected object counts per generation
        
    Examples:
        >>> collected = force_gc()
        >>> print(f"Collected {sum(collected.values())} objects")
    """
    collected = {
        "gen0": gc.collect(0),
        "gen1": gc.collect(1),
        "gen2": gc.collect(2)
    }
    
    logger.info(f"GC collected: {collected}")
    return collected


class ObjectPool:
    """
    Object pool for reusing expensive objects.
    
    Reduces object creation/destruction overhead.
    
    Examples:
        >>> pool = ObjectPool(MyExpensiveClass, max_size=10)
        >>> obj = pool.acquire()
        >>> # Use object
        >>> pool.release(obj)
    """
    
    def __init__(self, factory: Type, max_size: int = 100):
        """
        Initialize object pool.
        
        Args:
            factory: Class or factory function to create objects
            max_size: Maximum pool size
        """
        self.factory = factory
        self.max_size = max_size
        self._pool: List[Any] = []
        self._active: weakref.WeakSet = weakref.WeakSet()
    
    def acquire(self) -> Any:
        """
        Acquire object from pool.
        
        Returns:
            Object instance
        """
        if self._pool:
            obj = self._pool.pop()
        else:
            obj = self.factory()
        
        self._active.add(obj)
        return obj
    
    def release(self, obj: Any):
        """
        Release object back to pool.
        
        Args:
            obj: Object to release
        """
        if obj in self._active:
            self._active.remove(obj)
        
        if len(self._pool) < self.max_size:
            # Reset object state if it has a reset method
            if hasattr(obj, 'reset'):
                obj.reset()
            self._pool.append(obj)
    
    def clear(self):
        """Clear the pool."""
        self._pool.clear()
    
    def get_stats(self) -> Dict[str, int]:
        """Get pool statistics."""
        return {
            "available": len(self._pool),
            "active": len(self._active),
            "max_size": self.max_size
        }


class MemoryMonitor:
    """
    Memory usage monitor with threshold alerts.
    
    Monitors memory usage and triggers callbacks when thresholds are exceeded.
    
    Examples:
        >>> def on_high_memory(stats):
        ...     print(f"Warning: Memory at {stats.percent_used}%")
        >>> 
        >>> monitor = MemoryMonitor(warning_threshold=80.0)
        >>> monitor.add_callback(on_high_memory)
        >>> monitor.check()  # Check current memory
    """
    
    def __init__(
        self,
        warning_threshold: float = 80.0,
        critical_threshold: float = 90.0
    ):
        """
        Initialize memory monitor.
        
        Args:
            warning_threshold: Warning threshold percentage
            critical_threshold: Critical threshold percentage
        """
        self.warning_threshold = warning_threshold
        self.critical_threshold = critical_threshold
        self._callbacks: List[callable] = []
        self._last_stats: Optional[MemoryStats] = None
    
    def add_callback(self, callback: callable):
        """
        Add callback for memory alerts.
        
        Args:
            callback: Function to call with MemoryStats when threshold exceeded
        """
        self._callbacks.append(callback)
    
    def check(self) -> MemoryStats:
        """
        Check current memory usage.
        
        Returns:
            Current memory statistics
        """
        stats = get_memory_stats()
        self._last_stats = stats
        
        # Check thresholds
        if stats.percent_used >= self.critical_threshold:
            logger.critical(f"Critical memory usage: {stats.percent_used:.1f}%")
            self._trigger_callbacks(stats, level="critical")
            # Force GC on critical
            force_gc()
        elif stats.percent_used >= self.warning_threshold:
            logger.warning(f"High memory usage: {stats.percent_used:.1f}%")
            self._trigger_callbacks(stats, level="warning")
        
        return stats
    
    def _trigger_callbacks(self, stats: MemoryStats, level: str):
        """Trigger registered callbacks."""
        for callback in self._callbacks:
            try:
                callback(stats, level)
            except Exception as e:
                logger.error(f"Memory callback error: {e}")
    
    def get_last_stats(self) -> Optional[MemoryStats]:
        """Get last checked statistics."""
        return self._last_stats


def profile_memory_usage(func):
    """
    Decorator to profile memory usage of a function.
    
    Logs memory before and after function execution.
    
    Examples:
        >>> @profile_memory_usage
        >>> def process_data(data):
        ...     # Process large dataset
        ...     pass
    """
    def wrapper(*args, **kwargs):
        # Get memory before
        before = get_memory_stats()
        
        # Execute function
        result = func(*args, **kwargs)
        
        # Get memory after
        after = get_memory_stats()
        
        # Log difference
        diff = after.used_bytes - before.used_bytes
        logger.info(
            f"{func.__name__} memory usage: "
            f"{diff / 1024 / 1024:.2f} MB "
            f"({before.percent_used:.1f}% → {after.percent_used:.1f}%)"
        )
        
        return result
    
    return wrapper


def optimize_data_structure(data: Any) -> Any:
    """
    Optimize data structure for memory efficiency.
    
    Converts lists to tuples, dicts to frozensets where appropriate.
    
    Args:
        data: Data structure to optimize
        
    Returns:
        Optimized data structure
        
    Examples:
        >>> data = [1, 2, 3, [4, 5]]
        >>> optimized = optimize_data_structure(data)
        >>> # Returns tuple: (1, 2, 3, (4, 5))
    """
    if isinstance(data, list):
        # Convert lists to tuples (immutable, less memory)
        return tuple(optimize_data_structure(item) for item in data)
    elif isinstance(data, dict):
        # For small dicts with hashable values, consider frozenset
        try:
            items = tuple((k, optimize_data_structure(v)) for k, v in data.items())
            if len(items) < 10:  # Only optimize small dicts
                return dict(items)
        except (TypeError, ValueError):
            pass
        return data
    elif isinstance(data, set):
        # Convert to frozenset if all items are hashable
        try:
            return frozenset(data)
        except TypeError:
            return data
    else:
        return data


# Global memory monitor instance
memory_monitor = MemoryMonitor(warning_threshold=80.0, critical_threshold=90.0)
