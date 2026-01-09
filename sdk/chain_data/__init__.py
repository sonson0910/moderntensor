"""
Chain Data Models

Standardized data models for ModernTensor blockchain (Luxtensor) data structures.
This module provides Pydantic models for all chain data types, similar to Bittensor's
chain_data module, but optimized for ModernTensor's custom Layer 1 blockchain.
"""

# Re-export models from sdk.models for backward compatibility
# and provide a centralized location for all chain data models
from sdk.models import (
    NeuronInfo,
    SubnetInfo,
    SubnetHyperparameters,
    StakeInfo,
    ValidatorInfo,
    MinerInfo,
    AxonInfo,
    DelegateInfo,
    PrometheusInfo,
    BlockInfo,
    TransactionInfo,
)

# Import new specialized models
from .neuron_info_lite import NeuronInfoLite
from .proxy_info import ProxyInfo
from .schedule_info import ScheduleInfo
from .identity_info import IdentityInfo

__all__ = [
    # Core Models (from sdk.models)
    "NeuronInfo",
    "SubnetInfo",
    "SubnetHyperparameters",
    "StakeInfo",
    "ValidatorInfo",
    "MinerInfo",
    "AxonInfo",
    "DelegateInfo",
    "PrometheusInfo",
    "BlockInfo",
    "TransactionInfo",
    # Additional Models
    "NeuronInfoLite",
    "ProxyInfo",
    "ScheduleInfo",
    "IdentityInfo",
]

__version__ = "0.4.0"
