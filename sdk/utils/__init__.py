"""
SDK Utilities Module

Collection of utility functions and classes for the ModernTensor SDK.
"""

# Balance utilities
from .balance import (
    Balance,
    BalanceError,
    format_balance,
    parse_balance,
    mdt_to_wei,
    wei_to_mdt,
    validate_balance,
    calculate_percentage,
    sum_balances,
    WEI_PER_MDT,
)

# Weight utilities
from .weight_utils import (
    WeightError,
    normalize_weights,
    validate_weight_matrix,
    compute_weight_consensus,
    apply_weight_decay,
    sparse_to_dense_weights,
    dense_to_sparse_weights,
    clip_weights,
    compute_weight_entropy,
    smooth_weights,
    top_k_weights,
)

# Network utilities
from .network import (
    NetworkError,
    EndpointStatus,
    EndpointInfo,
    check_endpoint_health,
    check_multiple_endpoints,
    is_port_open,
    parse_endpoint,
    retry_async,
    retry_sync,
    CircuitBreaker,
    wait_for_service,
    get_local_ip,
    format_url,
)

# Query optimization
from .query_optimization import (
    LRUCache,
    QueryCache,
    BatchProcessor,
    ConnectionPool,
    query_cache,
)

# Memory optimization
from .memory_optimization import (
    MemoryStats,
    get_object_size,
    get_memory_stats,
    optimize_gc,
    force_gc,
    ObjectPool,
    MemoryMonitor,
    profile_memory_usage,
    optimize_data_structure,
    memory_monitor,
)

# Concurrency optimization
from .concurrency_optimization import (
    TaskResult,
    AsyncTaskPool,
    ThreadPoolExecutor,
    ParallelProcessor,
    gather_with_concurrency,
    run_with_timeout,
    AsyncRateLimiter,
    optimize_async_loop,
    WorkQueue,
)

__all__ = [
    # Balance
    "Balance",
    "BalanceError",
    "format_balance",
    "parse_balance",
    "mdt_to_wei",
    "wei_to_mdt",
    "validate_balance",
    "calculate_percentage",
    "sum_balances",
    "WEI_PER_MDT",
    # Weight
    "WeightError",
    "normalize_weights",
    "validate_weight_matrix",
    "compute_weight_consensus",
    "apply_weight_decay",
    "sparse_to_dense_weights",
    "dense_to_sparse_weights",
    "clip_weights",
    "compute_weight_entropy",
    "smooth_weights",
    "top_k_weights",
    # Network
    "NetworkError",
    "EndpointStatus",
    "EndpointInfo",
    "check_endpoint_health",
    "check_multiple_endpoints",
    "is_port_open",
    "parse_endpoint",
    "retry_async",
    "retry_sync",
    "CircuitBreaker",
    "wait_for_service",
    "get_local_ip",
    "format_url",
    # Query Optimization
    "LRUCache",
    "QueryCache",
    "BatchProcessor",
    "ConnectionPool",
    "query_cache",
    # Memory Optimization
    "MemoryStats",
    "get_object_size",
    "get_memory_stats",
    "optimize_gc",
    "force_gc",
    "ObjectPool",
    "MemoryMonitor",
    "profile_memory_usage",
    "optimize_data_structure",
    "memory_monitor",
    # Concurrency Optimization
    "TaskResult",
    "AsyncTaskPool",
    "ThreadPoolExecutor",
    "ParallelProcessor",
    "gather_with_concurrency",
    "run_with_timeout",
    "AsyncRateLimiter",
    "optimize_async_loop",
    "WorkQueue",
]
