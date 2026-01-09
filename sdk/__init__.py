"""
ModernTensor SDK

Python SDK for interacting with Luxtensor blockchain and building AI/ML subnets.
"""

# Luxtensor client (blockchain interaction)
from .luxtensor_client import (
    LuxtensorClient,
    AsyncLuxtensorClient,
    connect,
    async_connect,
    ChainInfo,
    Account,
    TransactionResult,
)

# Transactions
from .transactions import (
    LuxtensorTransaction,
    create_transfer_transaction,
    sign_transaction,
    verify_transaction_signature,
    encode_transaction_for_rpc,
)

# Version
from .version import __version__

__all__ = [
    # Luxtensor client
    "LuxtensorClient",
    "AsyncLuxtensorClient",
    "connect",
    "async_connect",
    "ChainInfo",
    "Account",
    "TransactionResult",
    # Transactions
    "LuxtensorTransaction",
    "create_transfer_transaction",
    "sign_transaction",
    "verify_transaction_signature",
    "encode_transaction_for_rpc",
    # Version
    "__version__",
]
