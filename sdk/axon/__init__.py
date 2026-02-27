"""
Axon module — Server component for miners and validators.

This module provides the server-side functionality for ModernTensor nodes,
allowing them to receive and process requests from the network.

Usage::

    from sdk.axon import Axon, AxonConfig, create_axon

    axon = create_axon(AxonConfig(port=8091))
    axon.attach("/forward", handler=my_forward_fn)
    axon.run()
"""

from .axon import Axon, create_axon
from .config import AxonConfig, AxonMetrics
from .middleware import (
    AuthenticationMiddleware,
    RateLimitMiddleware,
    BlacklistMiddleware,
)
from .security import SecurityManager

__all__ = [
    "Axon",
    "create_axon",
    "AxonConfig",
    "AxonMetrics",
    "AuthenticationMiddleware",
    "RateLimitMiddleware",
    "BlacklistMiddleware",
    "SecurityManager",
]
