"""
LuxtensorClient Module

Modular client architecture using mixins for maintainability.

Usage:
    # Standard usage
    from sdk.client import LuxtensorClient
    client = LuxtensorClient("http://localhost:8545")

    # Block methods
    block = client.get_block_number()

    # Account methods
    account = client.get_account("0x...")

    # Staking methods
    stake = client.get_stake("0x...")

    # Subnet methods
    subnets = client.get_all_subnets()

    # Custom modular client
    from sdk.client import (
        BaseClient,
        BlockchainMixin,
        AccountMixin,
        StakingMixin,
    )

    class MyLightClient(BlockchainMixin, AccountMixin, BaseClient):
        '''Custom client with only blockchain and account features'''
        pass
"""

import logging

# Base types
from .base import (
    BaseClient,
    ChainInfo,
    Account,
    TransactionResult,
)

# Mixins
from .blockchain_mixin import BlockchainMixin
from .account_mixin import AccountMixin
from .transaction_mixin import TransactionMixin
from .staking_mixin import StakingMixin
from .subnet_mixin import SubnetMixin
from .neuron_mixin import NeuronMixin

logger = logging.getLogger(__name__)


class LuxtensorClient(
    BlockchainMixin,
    AccountMixin,
    TransactionMixin,
    StakingMixin,
    SubnetMixin,
    NeuronMixin,
    BaseClient,
):
    """
    Full-featured Luxtensor client.

    Composes all mixins for complete functionality.
    For lighter clients, compose only needed mixins.
    """

    def __init__(
        self,
        url: str = "http://localhost:8545",
        network: str = "testnet",
        timeout: int = 30,
    ):
        """
        Initialize Luxtensor client.

        Args:
            url: Luxtensor RPC endpoint URL
            network: Network name (mainnet, testnet, devnet)
            timeout: Request timeout in seconds
        """
        self.url = url
        self.network = network
        self.timeout = timeout
        self._request_id = 0

        # Also initialize domain clients for new-style access
        from ..clients import BlockClient, StakeClient, NeuronClient, SubnetClient, TransactionClient

        self.blocks = BlockClient(url, timeout, self._get_request_id)
        self.stakes = StakeClient(url, timeout, self._get_request_id)
        self.neurons_client = NeuronClient(url, timeout, self._get_request_id)
        self.subnets_client = SubnetClient(url, timeout, self._get_request_id)
        self.transactions_client = TransactionClient(url, timeout, self._get_request_id)

        logger.info(f"Initialized Luxtensor client for {network} at {url}")


# Helper functions
def connect(url: str = "http://localhost:8545", **kwargs) -> LuxtensorClient:
    """Create and return a LuxtensorClient instance."""
    return LuxtensorClient(url=url, **kwargs)


async def async_connect(url: str = "http://localhost:8545", **kwargs):
    """Create async client (placeholder for async implementation)."""
    return LuxtensorClient(url=url, **kwargs)


# Classes for type alias compatibility
AsyncLuxtensorClient = LuxtensorClient  # Placeholder


__all__ = [
    # Main client
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
    "SubnetMixin",
    "NeuronMixin",
]
