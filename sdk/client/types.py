"""
Type definitions for SDK client

Provides TypedDict definitions for structured return types.
"""

from typing import TypedDict


class RewardBalance(TypedDict):
    """Reward balance information."""

    available: int
    pendingRewards: int
    staked: int
    lockedUntil: int


class NetworkState(TypedDict):
    """Network state summary."""

    block_number: int
    total_subnets: int
    total_neurons: int
    total_stake: int
    total_issuance: int
    network_info: dict


class ValidatorInfo(TypedDict):
    """Validator information."""

    address: str
    stake: int
    commission: int
    active: bool


class SubnetInfo(TypedDict):
    """Subnet information."""

    subnet_id: int
    owner: str
    name: str
    emission: int
    tempo: int
    max_neurons: int


class NeuronInfo(TypedDict):
    """Neuron information."""

    neuron_id: int
    hotkey: str
    coldkey: str
    stake: int
    subnet_id: int
