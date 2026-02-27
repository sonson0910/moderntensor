"""
Domain-Specific RPC Clients

Clean Code refactoring following the Single Responsibility Principle:
each client handles one domain (blocks, staking, neurons, subnets, transactions).
"""

from .base import BaseRpcClient, RpcError, RpcConnectionError
from .block_client import BlockClient
from .stake_client import StakeClient
from .neuron_client import NeuronClient
from .subnet_client import SubnetClient
from .transaction_client import TransactionClient

__all__ = [
    "BaseRpcClient",
    "RpcError",
    "RpcConnectionError",
    "BlockClient",
    "StakeClient",
    "NeuronClient",
    "SubnetClient",
    "TransactionClient",
]
