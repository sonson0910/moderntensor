"""
Neuron Information Model

Represents a neuron in the LuxTensor network.
Aligned with NeuronInfo struct in luxtensor-rpc/src/types.rs
"""

from typing import Optional, Dict, Any
from pydantic import BaseModel, Field


class NeuronInfo(BaseModel):
    """
    Complete neuron information including state, stake, and performance metrics.

    Matches LuxTensor RPC NeuronInfo:
    - uid: u64
    - address: String
    - subnet_id: u64
    - stake: u128
    - trust: f64
    - consensus: f64
    - rank: u64
    - incentive: f64
    - dividends: f64
    - active: bool
    - endpoint: Option<String>
    """

    # Identity
    uid: int = Field(..., description="Unique neuron identifier (u64)", ge=0)
    subnet_id: int = Field(..., description="Subnet this neuron belongs to", ge=0)
    address: str = Field(..., description="Neuron address (0x...)")
    hotkey: Optional[str] = Field(default=None, description="Neuron hotkey address (0x...), alias for address")
    coldkey: Optional[str] = Field(default=None, description="Neuron coldkey address (0x...)")

    # Network State
    active: bool = Field(default=True, description="Whether neuron is active")
    endpoint: Optional[str] = Field(default=None, description="HTTP endpoint for API calls")

    # Stake Information (u128 represented as int)
    stake: int = Field(default=0, description="Stake amount in base units", ge=0)

    # Performance Metrics (f64 from server, no upper bound constraint)
    rank: int = Field(default=0, description="Rank (u64)", ge=0)
    trust: float = Field(default=0.0, description="Trust score (f64)", ge=0)
    incentive: float = Field(default=0.0, description="Incentive score (f64)", ge=0)
    dividends: float = Field(default=0.0, description="Dividends score (f64)", ge=0)
    consensus: float = Field(default=0.0, description="Consensus weight (f64)", ge=0)

    # Optional fields (not returned by the Rust RPC server)
    emission: Optional[int] = Field(default=None, description="Token emission in base units")
    validator_permit: Optional[bool] = Field(default=None, description="Has validator permit")
    validator_trust: Optional[float] = Field(default=None, description="Validator trust")
    last_update: Optional[int] = Field(default=None, description="Last update block/timestamp")
    axon_info: Optional[Dict[str, Any]] = Field(default=None, description="Axon server info")
    prometheus_info: Optional[Dict[str, Any]] = Field(default=None, description="Prometheus endpoint")

    class Config:
        json_schema_extra = {
            "example": {
                "uid": 0,
                "subnet_id": 1,
                "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
                "hotkey": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
                "active": True,
                "endpoint": "http://neuron.example.com:8080",
                "stake": 1000000000,
                "rank": 42,
                "trust": 0.98,
                "incentive": 0.90,
                "dividends": 0.88,
                "consensus": 0.92
            }
        }

    @classmethod
    def from_rust_data(cls, data: dict) -> "NeuronInfo":
        """
        Create NeuronInfo from LuxTensor RPC response.

        Handles the `address` field from the Rust NeuronInfo struct,
        mapping it to both `address` and `hotkey` for compatibility.
        """
        # The Rust server returns "address", not "hotkey"
        addr = data.get("address", data.get("hotkey", "0x" + "0" * 40))
        return cls(
            uid=data.get("uid", 0),
            subnet_id=data.get("subnet_id", 0),
            address=addr,
            hotkey=data.get("hotkey", addr),
            coldkey=data.get("coldkey"),
            active=data.get("active", True),
            endpoint=data.get("endpoint"),
            stake=data.get("stake", 0),
            rank=data.get("rank", 0),
            trust=float(data.get("trust", 0.0)),
            incentive=float(data.get("incentive", 0.0)),
            dividends=float(data.get("dividends", 0.0)),
            consensus=float(data.get("consensus", 0.0)),
            emission=data.get("emission"),
            validator_permit=data.get("validator_permit"),
            validator_trust=data.get("validator_trust"),
            last_update=data.get("last_update"),
        )

    def __str__(self) -> str:
        display_addr = self.address[:10] if self.address else "unknown"
        return f"NeuronInfo(uid={self.uid}, subnet={self.subnet_id}, addr={display_addr}..., stake={self.stake})"

    def __repr__(self) -> str:
        return self.__str__()
