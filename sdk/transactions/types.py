"""
Transaction Types and Data Models

Defines all transaction types supported by ModernTensor network.
"""

from enum import Enum
from typing import Optional, Dict, Any, List, Literal
from pydantic import BaseModel, Field, field_validator, ConfigDict, ValidationInfo


class TransactionType(str, Enum):
    """Transaction types supported by ModernTensor."""
    
    # Basic Operations
    TRANSFER = "transfer"
    STAKE = "stake"
    UNSTAKE = "unstake"
    
    # Network Operations
    REGISTER = "register"
    DEREGISTER = "deregister"
    
    # Subnet Operations
    SET_WEIGHTS = "set_weights"
    SERVE_AXON = "serve_axon"
    SERVE_PROMETHEUS = "serve_prometheus"
    
    # Hotkey Operations
    SWAP_HOTKEY = "swap_hotkey"
    
    # Governance
    PROPOSE = "propose"
    VOTE = "vote"
    
    # Advanced
    DELEGATE = "delegate"
    UNDELEGATE = "undelegate"
    PROXY = "proxy"


class BaseTransaction(BaseModel):
    """Base class for all transactions."""
    
    model_config = ConfigDict(use_enum_values=True)
    
    transaction_type: TransactionType
    from_address: str = Field(..., description="Sender address")
    nonce: Optional[int] = Field(default=None, description="Transaction nonce")
    fee: Optional[float] = Field(default=None, description="Transaction fee")
    memo: Optional[str] = Field(default=None, description="Optional memo")


class TransferTransaction(BaseTransaction):
    """Transfer tokens to another address."""
    
    transaction_type: Literal[TransactionType.TRANSFER] = TransactionType.TRANSFER
    to_address: str = Field(..., description="Recipient address")
    amount: float = Field(..., description="Amount to transfer", gt=0)
    
    @field_validator('amount')
    @classmethod
    def validate_amount(cls, v):
        if v <= 0:
            raise ValueError("Transfer amount must be positive")
        return v


class StakeTransaction(BaseTransaction):
    """Stake tokens to a hotkey."""
    
    transaction_type: Literal[TransactionType.STAKE] = TransactionType.STAKE
    hotkey: str = Field(..., description="Hotkey to stake to")
    amount: float = Field(..., description="Amount to stake", gt=0)
    subnet_id: Optional[int] = Field(default=None, description="Subnet ID")
    
    @field_validator('amount')
    @classmethod
    def validate_amount(cls, v):
        if v <= 0:
            raise ValueError("Stake amount must be positive")
        return v


class UnstakeTransaction(BaseTransaction):
    """Unstake tokens from a hotkey."""
    
    transaction_type: Literal[TransactionType.UNSTAKE] = TransactionType.UNSTAKE
    hotkey: str = Field(..., description="Hotkey to unstake from")
    amount: float = Field(..., description="Amount to unstake", gt=0)
    subnet_id: Optional[int] = Field(default=None, description="Subnet ID")
    
    @field_validator('amount')
    @classmethod
    def validate_amount(cls, v):
        if v <= 0:
            raise ValueError("Unstake amount must be positive")
        return v


class RegisterTransaction(BaseTransaction):
    """Register a neuron on a subnet."""
    
    transaction_type: Literal[TransactionType.REGISTER] = TransactionType.REGISTER
    subnet_id: int = Field(..., description="Subnet ID to register on", ge=0)
    hotkey: str = Field(..., description="Hotkey for the neuron")
    metadata: Optional[Dict[str, Any]] = Field(default=None, description="Registration metadata")


class WeightTransaction(BaseTransaction):
    """Set weights for validator."""
    
    transaction_type: Literal[TransactionType.SET_WEIGHTS] = TransactionType.SET_WEIGHTS
    subnet_id: int = Field(..., description="Subnet ID", ge=0)
    uids: List[int] = Field(..., description="List of neuron UIDs")
    weights: List[float] = Field(..., description="List of weights (must sum to 1.0)")
    version_key: int = Field(..., description="Weight version key", ge=0)
    
    @field_validator('weights')
    @classmethod
    def validate_weights(cls, v, info: ValidationInfo):
        if 'uids' in info.data and len(v) != len(info.data['uids']):
            raise ValueError("Number of weights must match number of UIDs")
        
        weight_sum = sum(v)
        if not (0.99 <= weight_sum <= 1.01):  # Allow small floating point error
            raise ValueError(f"Weights must sum to 1.0, got {weight_sum}")
        
        if any(w < 0 for w in v):
            raise ValueError("All weights must be non-negative")
        
        return v


class ProposalTransaction(BaseTransaction):
    """Submit a governance proposal."""
    
    transaction_type: Literal[TransactionType.PROPOSE] = TransactionType.PROPOSE
    title: str = Field(..., description="Proposal title", min_length=1, max_length=200)
    description: str = Field(..., description="Proposal description", min_length=1)
    proposal_type: str = Field(..., description="Type of proposal")
    options: List[str] = Field(..., description="Voting options")
    duration_blocks: int = Field(..., description="Proposal duration in blocks", gt=0)
    
    @field_validator('options')
    @classmethod
    def validate_options(cls, v):
        if len(v) < 2:
            raise ValueError("Proposal must have at least 2 options")
        return v


class VoteTransaction(BaseTransaction):
    """Vote on a proposal."""
    
    transaction_type: Literal[TransactionType.VOTE] = TransactionType.VOTE
    proposal_id: int = Field(..., description="Proposal ID", ge=0)
    option: str = Field(..., description="Selected option")
    voting_power: Optional[float] = Field(default=None, description="Voting power to use")


class DelegateTransaction(BaseTransaction):
    """Delegate stake to another validator."""
    
    transaction_type: Literal[TransactionType.DELEGATE] = TransactionType.DELEGATE
    validator_hotkey: str = Field(..., description="Validator hotkey to delegate to")
    amount: float = Field(..., description="Amount to delegate", gt=0)
    
    @field_validator('amount')
    @classmethod
    def validate_amount(cls, v):
        if v <= 0:
            raise ValueError("Delegation amount must be positive")
        return v


class ServeAxonTransaction(BaseTransaction):
    """Update Axon serving information."""
    
    transaction_type: Literal[TransactionType.SERVE_AXON] = TransactionType.SERVE_AXON
    subnet_id: int = Field(..., description="Subnet ID", ge=0)
    ip: str = Field(..., description="IP address")
    port: int = Field(..., description="Port number", ge=1, le=65535)
    protocol: str = Field(default="http", description="Protocol (http/https)")
    version: int = Field(default=1, description="Version number", ge=1)


class SwapHotkeyTransaction(BaseTransaction):
    """Swap hotkey for a neuron."""
    
    transaction_type: Literal[TransactionType.SWAP_HOTKEY] = TransactionType.SWAP_HOTKEY
    subnet_id: int = Field(..., description="Subnet ID", ge=0)
    old_hotkey: str = Field(..., description="Current hotkey")
    new_hotkey: str = Field(..., description="New hotkey")


# Type alias for all transaction types
Transaction = (
    TransferTransaction |
    StakeTransaction |
    UnstakeTransaction |
    RegisterTransaction |
    WeightTransaction |
    ProposalTransaction |
    VoteTransaction |
    DelegateTransaction |
    ServeAxonTransaction |
    SwapHotkeyTransaction
)
