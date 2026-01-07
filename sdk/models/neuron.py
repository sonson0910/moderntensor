"""
Neuron Information Model

Represents a neuron in the ModernTensor network.
"""

from typing import Optional
from pydantic import BaseModel, Field


class NeuronInfo(BaseModel):
    """Complete neuron information including state, stake, and performance metrics."""
    
    # Identity
    uid: int = Field(..., description="Unique neuron identifier", ge=0)
    hotkey: str = Field(..., description="Neuron hotkey address")
    coldkey: str = Field(..., description="Neuron coldkey address")
    
    # Network State
    active: bool = Field(default=True, description="Whether neuron is active")
    subnet_uid: int = Field(..., description="Subnet this neuron belongs to", ge=0)
    
    # Stake Information
    stake: float = Field(default=0.0, description="Total stake amount", ge=0)
    total_stake: float = Field(default=0.0, description="Total stake including delegated", ge=0)
    
    # Performance Metrics
    rank: float = Field(default=0.0, description="Neuron rank score", ge=0, le=1)
    trust: float = Field(default=0.0, description="Trust score", ge=0, le=1)
    consensus: float = Field(default=0.0, description="Consensus weight", ge=0, le=1)
    incentive: float = Field(default=0.0, description="Incentive score", ge=0, le=1)
    dividends: float = Field(default=0.0, description="Dividends earned", ge=0, le=1)
    emission: float = Field(default=0.0, description="Token emission rate", ge=0)
    
    # Validator Information
    validator_permit: bool = Field(default=False, description="Has validator permit")
    validator_trust: float = Field(default=0.0, description="Validator trust score", ge=0, le=1)
    
    # Metadata
    last_update: int = Field(default=0, description="Last update block number", ge=0)
    priority: float = Field(default=0.0, description="Priority score", ge=0)
    
    # Axon Information
    axon_info: Optional[dict] = Field(default=None, description="Axon server information")
    
    # Prometheus Information  
    prometheus_info: Optional[dict] = Field(default=None, description="Prometheus metrics endpoint")
    
    class Config:
        json_json_schema_extra = {
            "example": {
                "uid": 0,
                "hotkey": "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                "coldkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "active": True,
                "subnet_uid": 1,
                "stake": 1000.0,
                "total_stake": 1500.0,
                "rank": 0.95,
                "trust": 0.98,
                "consensus": 0.92,
                "incentive": 0.90,
                "dividends": 0.88,
                "emission": 100.5,
                "validator_permit": True,
                "validator_trust": 0.99,
                "last_update": 12345,
                "priority": 0.87
            }
        }
        
    def __str__(self) -> str:
        return f"NeuronInfo(uid={self.uid}, hotkey={self.hotkey[:8]}..., stake={self.stake})"
    
    def __repr__(self) -> str:
        return self.__str__()
