"""
ModernTensor Extrinsics (Transactions)

Provides transaction builders for all blockchain operations.
Similar to Bittensor's extrinsics but optimized for Luxtensor blockchain.
"""

from .transfer import transfer, batch_transfer
from .staking import stake, unstake, add_stake, unstake_all
from .registration import register, burned_register
from .weights import set_weights, commit_weights, reveal_weights
from .serving import serve_axon, serve_prometheus
from .proxy import add_proxy, remove_proxy, proxy_call
from .delegation import delegate, undelegate, nominate

__all__ = [
    # Transfer operations
    "transfer",
    "batch_transfer",
    # Staking operations
    "stake",
    "unstake",
    "add_stake",
    "unstake_all",
    # Registration
    "register",
    "burned_register",
    # Weights
    "set_weights",
    "commit_weights",
    "reveal_weights",
    # Serving
    "serve_axon",
    "serve_prometheus",
    # Proxy operations
    "add_proxy",
    "remove_proxy",
    "proxy_call",
    # Delegation
    "delegate",
    "undelegate",
    "nominate",
]

__version__ = "0.4.0"
