"""
Luxtensor Python Client — Backward-Compatibility Shim

.. deprecated::
    This module is deprecated. Use ``from sdk.client import LuxtensorClient`` instead.

    The full-featured, mixin-based client now lives in ``sdk.client``.
    This shim re-exports all public names so that existing code like::

        from sdk.luxtensor_client import LuxtensorClient

    continues to work without modification. However, a ``DeprecationWarning``
    is emitted on import to encourage migration.

Migration guide::

    # Old (deprecated)
    from sdk.luxtensor_client import LuxtensorClient

    # New (preferred)
    from sdk.client import LuxtensorClient

    # Or simply:
    from sdk import LuxtensorClient
"""

import warnings

warnings.warn(
    "Importing from 'sdk.luxtensor_client' is deprecated. "
    "Use 'from sdk.client import LuxtensorClient' or 'from sdk import LuxtensorClient' instead. "
    "This compatibility shim will be removed in v1.0.",
    DeprecationWarning,
    stacklevel=2,
)

# Re-export everything from the modular client for backward compatibility
from .client import (  # noqa: E402, F401
    LuxtensorClient,
    AsyncLuxtensorClient,
    connect,
    async_connect,
    BaseClient,
    ChainInfo,
    Account,
    TransactionResult,
)

__all__ = [
    "LuxtensorClient",
    "AsyncLuxtensorClient",
    "connect",
    "async_connect",
    "BaseClient",
    "ChainInfo",
    "Account",
    "TransactionResult",
]
