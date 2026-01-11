"""
Neuron Information Model

Represents a neuron in the LuxTensor network.
Aligned with NeuronData struct in luxtensor-storage/src/metagraph_store.rs
"""

from typing import Optional, Dict, Any
from pydantic import BaseModel, Field


class NeuronInfo(BaseModel):
    """
    Complete neuron information including state, stake, and performance metrics.

    Matches LuxTensor NeuronData:
    - uid: u64
    - subnet_id: u64
    - hotkey: [u8; 20]
    - coldkey: [u8; 20]
    - stake: u128
    - trust: u32 (fixed point / 65535)
    - rank: u32
    - incentive: u32
    - dividends: u32
    - emission: u128
    - last_update: u64
    - active: bool
    - endpoint: String
    """

    # Identity
    uid: int = Field(..., description="Unique neuron identifier (u64)", ge=0)
    subnet_id: int = Field(..., description="Subnet this neuron belongs to", ge=0)
    hotkey: str = Field(..., description="Neuron hotkey address (0x...)")
    coldkey: str = Field(..., description="Neuron coldkey address (0x...)")

    # Network State
    active: bool = Field(default=True, description="Whether neuron is active")
    endpoint: str = Field(default="", description="HTTP endpoint for API calls")

    # Stake Information (u128 represented as int)
    stake: int = Field(default=0, description="Stake amount in base units", ge=0)

    # Performance Metrics (u32 fixed point, normalized to float)
    # LuxTensor stores as u32 where value = score * 65535
    rank: float = Field(default=0.0, description="Rank score (0-1)", ge=0, le=1)
    trust: float = Field(default=0.0, description="Trust score (0-1)", ge=0, le=1)
    incentive: float = Field(default=0.0, description="Incentive score (0-1)", ge=0, le=1)
    dividends: float = Field(default=0.0, description="Dividends score (0-1)", ge=0, le=1)
    consensus: float = Field(default=0.0, description="Consensus weight (0-1)", ge=0, le=1)

    # Emission (u128)
    emission: int = Field(default=0, description="Token emission in base units", ge=0)

    # Validator Information
    validator_permit: bool = Field(default=False, description="Has validator permit")
    validator_trust: float = Field(default=0.0, description="Validator trust (0-1)", ge=0, le=1)

    # Metadata
    last_update: int = Field(default=0, description="Last update block/timestamp", ge=0)

    # Optional extended info
    axon_info: Optional[Dict[str, Any]] = Field(default=None, description="Axon server info")
    prometheus_info: Optional[Dict[str, Any]] = Field(default=None, description="Prometheus endpoint")

    class Config:
        json_schema_extra = {
            "example": {
                "uid": 0,
                "subnet_id": 1,
                "hotkey": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
                "coldkey": "0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3",
                "active": True,
                "endpoint": "http://neuron.example.com:8080",
                "stake": 1000000000,
                "rank": 0.95,
                "trust": 0.98,
                "incentive": 0.90,
                "dividends": 0.88,
                "consensus": 0.92,
                "emission": 100,
                "validator_permit": True,
                "validator_trust": 0.99,
                "last_update": 12345
            }
        }

    @classmethod
    def from_rust_data(cls, data: dict) -> "NeuronInfo":
        """
        Create NeuronInfo from LuxTensor RPC response.

        Converts u32 fixed-point values to floats.
        """
        return cls(
            uid=data.get("uid", 0),
            subnet_id=data.get("subnet_id", 0),
            hotkey=data.get("hotkey", "0x" + "0" * 40),
            coldkey=data.get("coldkey", "0x" + "0" * 40),
            active=data.get("active", True),
            endpoint=data.get("endpoint", ""),
            stake=data.get("stake", 0),
            rank=data.get("rank", 0) / 65535.0 if isinstance(data.get("rank"), int) else data.get("rank", 0),
            trust=data.get("trust", 0) / 65535.0 if isinstance(data.get("trust"), int) else data.get("trust", 0),
            incentive=data.get("incentive", 0) / 65535.0 if isinstance(data.get("incentive"), int) else data.get("incentive", 0),
            dividends=data.get("dividends", 0) / 65535.0 if isinstance(data.get("dividends"), int) else data.get("dividends", 0),
            emission=data.get("emission", 0),
            last_update=data.get("last_update", 0),
        )

    def __str__(self) -> str:
        return f"NeuronInfo(uid={self.uid}, subnet={self.subnet_id}, hotkey={self.hotkey[:10]}..., stake={self.stake})"

    def __repr__(self) -> str:
        return self.__str__()
