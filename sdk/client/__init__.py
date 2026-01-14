"""
LuxtensorClient Module

This module provides:
1. Full LuxtensorClient class (re-exported from main module)
2. Mixin classes for modular extension
3. Base classes and data types

Usage:
    # Standard usage (backward compatible)
    from sdk.client import LuxtensorClient

    # Using mixins for custom client
    from sdk.client import BaseClient, BlockchainMixin, AccountMixin

    class MyClient(BlockchainMixin, AccountMixin, BaseClient):
        pass
"""

# Re-export from main module (backward compat)
from ..luxtensor_client import (
    LuxtensorClient,
    AsyncLuxtensorClient,
    connect,
    async_connect,
)

# Base types
from .base import (
    BaseClient,
    ChainInfo,
    Account,
    TransactionResult,
)

# Mixins for modular client building
from .blockchain_mixin import BlockchainMixin
from .account_mixin import AccountMixin
from .transaction_mixin import TransactionMixin
from .staking_mixin import StakingMixin
from .subnet0_mixin import Subnet0Mixin
from .neuron_mixin import NeuronMixin

__all__ = [
    # Main client (backward compat)
    "LuxtensorClient",
    "AsyncLuxtensorClient",
    "connect",
    "async_connect",
    # Base
    "BaseClient",
    "ChainInfo",
    "Account",
    "TransactionResult",
    # Mixins
    "BlockchainMixin",
    "AccountMixin",
    "TransactionMixin",
    "StakingMixin",
    "Subnet0Mixin",
    "NeuronMixin",
]
