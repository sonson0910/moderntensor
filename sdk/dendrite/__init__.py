"""
Dendrite module — Client component for querying miners.

This module provides the client-side functionality for ModernTensor validators,
allowing them to query multiple miners and aggregate responses.

Features:
    - Async HTTP with connection pooling
    - Exponential-backoff retry + circuit breaker
    - Request deduplication and response caching
    - Pluggable load-balancing strategies

Usage::

    from sdk.dendrite import Dendrite, DendriteConfig, create_dendrite

    dendrite = create_dendrite(DendriteConfig(max_retries=5))
    result = await dendrite.query(endpoints, data=payload)
    await dendrite.close()
"""

from .dendrite import Dendrite, create_dendrite
from .config import DendriteConfig, DendriteMetrics, LoadBalancingStrategy, RetryStrategy
from .pool import ConnectionPool
from .aggregator import ResponseAggregator

__all__ = [
    "Dendrite",
    "create_dendrite",
    "DendriteConfig",
    "DendriteMetrics",
    "LoadBalancingStrategy",
    "RetryStrategy",
    "ConnectionPool",
    "ResponseAggregator",
]
