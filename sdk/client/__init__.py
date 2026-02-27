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

from .account_mixin import AccountMixin
from .ai_mixin import AIMixin
from .balance_mixin import BalanceMixin

# Base types
from .base import (
    Account,
    BaseClient,
    ChainInfo,
    TransactionResult,
)

# Mixins
from .blockchain_mixin import BlockchainMixin
from .consensus_mixin import ConsensusMixin
from .governance_mixin import GovernanceMixin
from .neuron_mixin import NeuronMixin
from .registration_mixin import RegistrationMixin
from .reward_mixin import RewardMixin
from .staking_mixin import StakingMixin
from .subnet_config_mixin import SubnetConfigMixin
from .subnet_mixin import SubnetMixin
from .transaction_mixin import TransactionMixin
from .utils_mixin import UtilsMixin
from .weights_mixin import WeightsMixin
from .metagraph_mixin import MetagraphMixin

# L1 Feature Gap Mixins
from .training_mixin import TrainingMixin
from .zkml_mixin import ZkmlMixin
from .agent_mixin import AgentMixin
from .bridge_mixin import BridgeMixin
from .dispute_mixin import DisputeMixin
from .multisig_mixin import MultisigMixin
from .node_mixin import NodeMixin

logger = logging.getLogger(__name__)


class LuxtensorClient(
    # L1 Feature Gap Mixins
    NodeMixin,       # Node tier registration & queries
    MultisigMixin,   # Multi-signature wallets
    DisputeMixin,    # Dispute / fraud proof
    BridgeMixin,     # Cross-chain bridge queries
    AgentMixin,      # AI agent management
    ZkmlMixin,       # Zero-knowledge ML
    TrainingMixin,   # Federated learning
    # Core Mixins
    ConsensusMixin,  # Client-side verification
    GovernanceMixin,  # DAO and governance
    BalanceMixin,  # Balance queries
    RewardMixin,  # Reward queries
    RegistrationMixin,  # Registration & axon
    SubnetConfigMixin,  # Subnet parameters
    AIMixin,  # AI tasks and oracle
    WeightsMixin,  # Weight management
    MetagraphMixin,  # Metagraph state access
    UtilsMixin,  # Utility methods
    SubnetMixin,  # Subnet management
    NeuronMixin,  # Neuron and weight queries
    StakingMixin,  # Staking operations
    TransactionMixin,  # Transaction submission
    AccountMixin,  # Account queries
    BlockchainMixin,  # Blockchain queries
    BaseClient,  # MUST BE LAST - provides _call_rpc
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
        enable_consensus: bool = False,
    ):
        """
        Initialize Luxtensor client.

        Args:
            url: Luxtensor RPC endpoint URL
            network: Network name (mainnet, testnet, devnet)
            timeout: Request timeout in seconds
        """
        # Initialize all mixins via MRO (CRITICAL for ConsensusMixin, UtilsMixin, etc.)
        super().__init__()

        self.url = url
        self.network = network
        self.timeout = timeout
        self._request_id = 0

        # Also initialize domain clients for new-style access
        from ..clients import (
            BlockClient,
            NeuronClient,
            StakeClient,
            SubnetClient,
            TransactionClient,
        )

        self.blocks = BlockClient(url, timeout, self._get_request_id)
        self.stakes = StakeClient(url, timeout, self._get_request_id)
        self.neurons = NeuronClient(url, timeout, self._get_request_id)
        self.subnets = SubnetClient(url, timeout, self._get_request_id)
        self.txs = TransactionClient(url, timeout, self._get_request_id)

        # Optionally initialize consensus verification
        if enable_consensus:
            try:
                self.init_consensus()
                logger.info("Consensus verification enabled")
            except Exception as e:
                logger.warning("Failed to initialize consensus: %s", e)

        logger.info("Initialized Luxtensor client for %s at %s", network, url)

    # ── Backward-compatible properties (deprecated naming) ────────────────

    @property
    def neurons_client(self):
        """Deprecated: use ``.neurons`` instead."""
        return self.neurons

    @property
    def subnets_client(self):
        """Deprecated: use ``.subnets`` instead."""
        return self.subnets

    @property
    def transactions_client(self):
        """Deprecated: use ``.txs`` instead."""
        return self.txs


# Async client (real implementation)
from ..async_luxtensor_client import AsyncLuxtensorClient


# Helper functions
def connect(url: str = "http://localhost:8545", **kwargs) -> LuxtensorClient:
    """Create and return a LuxtensorClient instance."""
    return LuxtensorClient(url=url, **kwargs)


async def async_connect(
    url: str = "http://localhost:8545", **kwargs
) -> AsyncLuxtensorClient:
    """
    Create and return an AsyncLuxtensorClient instance.

    The client is already connected when returned.

    Example::

        client = await async_connect("http://localhost:8545")
        block = await client.get_block_number()
        await client.close()

    Prefer using the async context manager instead::

        async with AsyncLuxtensorClient("http://...") as client:
            block = await client.get_block_number()
    """
    client = AsyncLuxtensorClient(url=url, **kwargs)
    await client.connect()
    return client


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
    # Core Mixins
    "BlockchainMixin",
    "AccountMixin",
    "TransactionMixin",
    "StakingMixin",
    "SubnetMixin",
    "NeuronMixin",
    "ConsensusMixin",
    "UtilsMixin",
    # L1 Feature Gap Mixins
    "TrainingMixin",
    "ZkmlMixin",
    "AgentMixin",
    "BridgeMixin",
    "DisputeMixin",
    "MultisigMixin",
    "NodeMixin",
    "MetagraphMixin",
]
