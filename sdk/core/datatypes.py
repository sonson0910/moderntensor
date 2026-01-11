# sdk/core/datatypes.py
"""
ModernTensor Core Data Types

Defines core data structures used throughout the SDK.
Updated to align with LuxTensor Rust implementation.
"""

from typing import List, Dict, Any, Optional
from dataclasses import dataclass, field
import time

from pydantic import BaseModel, Field


# Status constants (matching LuxTensor)
STATUS_ACTIVE = 1
STATUS_INACTIVE = 0


@dataclass
class MinerInfo:
    """
    Miner state information (memory representation).

    Corresponds to NeuronData in LuxTensor metagraph_store.rs
    """
    uid: str
    address: str  # 0x prefixed hex address (20 bytes)
    api_endpoint: Optional[str] = None  # HTTP endpoint for API calls
    trust_score: float = 0.0
    weight: float = 0.0  # W_x - Miner weight
    stake: float = 0.0  # Stake amount (from u128)
    last_selected_time: int = -1  # Last cycle selected
    performance_history: List[float] = field(default_factory=list)
    status: int = STATUS_ACTIVE
    subnet_uid: int = 0  # Subnet ID this miner belongs to
    registration_slot: int = 0  # Block/slot registered
    hotkey: Optional[str] = None  # Hotkey address (0x...)
    coldkey: Optional[str] = None  # Coldkey address (0x...)
    rank: float = 0.0  # Fixed point rank / 65535
    incentive: float = 0.0
    dividends: float = 0.0
    emission: float = 0.0
    last_update: int = 0


@dataclass
class ValidatorInfo:
    """
    Validator state information (memory representation).

    Corresponds to NeuronData in LuxTensor metagraph_store.rs with validator flag.
    """
    uid: str
    address: str  # 0x prefixed hex address
    api_endpoint: Optional[str] = None
    trust_score: float = 0.0
    weight: float = 0.0  # W_v
    stake: float = 0.0
    last_performance: float = 0.0
    status: int = STATUS_ACTIVE
    subnet_uid: int = 0
    registration_slot: int = 0
    hotkey: Optional[str] = None
    coldkey: Optional[str] = None
    performance_history: List[float] = field(default_factory=list)
    active: bool = True


@dataclass
class SubnetInfo:
    """
    Subnet information.

    Corresponds to SubnetData in LuxTensor metagraph_store.rs
    """
    id: int
    name: str
    owner: str  # Owner address (0x...)
    emission_rate: int  # u128 emission rate
    created_at: int  # Block/timestamp created
    tempo: int = 100  # Tempo (u16)
    max_neurons: int = 256  # Max neurons allowed
    min_stake: int = 0  # Minimum stake required
    active: bool = True


@dataclass
class WeightEntry:
    """
    Weight entry for neuron weight matrix.

    Corresponds to WeightData in LuxTensor metagraph_store.rs
    """
    from_uid: int
    to_uid: int
    weight: int  # u16 weight value
    updated_at: int  # Timestamp


@dataclass
class AITask:
    """
    AI Task information.

    Corresponds to AITaskData in LuxTensor metagraph_store.rs
    """
    id: bytes  # 32-byte task ID
    model_hash: str
    input_hash: bytes  # 32-byte input hash
    requester: str  # Requester address (0x...)
    reward: int  # u128 reward amount
    status: int = 0  # 0=Pending, 1=Processing, 2=Completed, 3=Failed
    worker: Optional[str] = None  # Worker address if assigned
    result_hash: Optional[bytes] = None  # 32-byte result hash
    created_at: int = 0
    completed_at: Optional[int] = None


@dataclass
class TaskAssignment:
    """Task assignment to a miner."""
    task_id: str
    task_data: Any
    miner_uid: str
    validator_uid: str
    timestamp_sent: float
    expected_result_format: Any


@dataclass
class MinerResult:
    """Result from a miner for a task."""
    task_id: str
    miner_uid: str
    result_data: Any
    timestamp_received: float
    proof: Optional[str] = None  # zkML proof or hash
    signature: Optional[str] = None  # Miner signature
    execution_metadata: Dict[str, Any] = field(default_factory=dict)


@dataclass
class MinerCommitment:
    """Commitment hash from miner (commit-reveal scheme)."""
    task_id: str
    miner_uid: str
    commitment_hash: str  # SHA256(result_hash + salt)
    timestamp: float
    salt: Optional[str] = None


@dataclass
class ValidatorScore:
    """Score from a validator for a miner on a task."""
    task_id: str
    miner_uid: str
    validator_uid: str
    score: float  # P_miner,v score
    deviation: Optional[float] = None  # Deviation from consensus
    penalty: float = 0.0  # Penalty (0.0 = none, 1.0 = max)
    timestamp: float = field(default_factory=time.time)


class ScoreSubmissionPayload(BaseModel):
    """Score submission payload for P2P API."""
    scores: List[Any] = Field(..., description="List of ValidatorScore objects")
    submitter_validator_uid: str = Field(..., description="Submitting validator UID (hex)")
    cycle: int = Field(..., description="Consensus cycle number")
    timestamp: float = Field(..., description="Unix timestamp (replay protection)")
    submitter_vkey_hex: Optional[str] = Field(None, description="Submitter verification key (hex)")
    signature: Optional[str] = Field(None, description="Signature of hash(scores+cycle+timestamp)")


class MinerConsensusResult(BaseModel):
    """Consensus result for a miner in a cycle."""
    miner_uid: str = Field(..., description="Miner UID (hex)")
    p_adj: float = Field(..., description="Adjusted performance score")
    calculated_incentive: float = Field(..., description="Calculated incentive (unscaled)")


class CycleConsensusResults(BaseModel):
    """Complete consensus results for a cycle."""
    cycle: int = Field(..., description="Cycle number")
    results: Dict[str, MinerConsensusResult] = Field(..., description="Results by miner_uid")
    publisher_uid: Optional[str] = Field(None, description="Publishing validator UID")
    publish_timestamp: Optional[float] = Field(default_factory=time.time)
