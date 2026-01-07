"""
Dendrite module - Client component for querying miners.

This module provides the client-side functionality for ModernTensor validators,
allowing them to query multiple miners and aggregate responses.
"""

from .dendrite import Dendrite
from .config import DendriteConfig
from .pool import ConnectionPool
from .aggregator import ResponseAggregator

__all__ = [
    "Dendrite",
    "DendriteConfig",
    "ConnectionPool",
    "ResponseAggregator",
]
