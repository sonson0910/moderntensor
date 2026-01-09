"""
Lightweight Neuron Information Model

Provides a lightweight version of NeuronInfo for efficient queries
when full neuron details are not needed.
"""

from typing import Optional
from pydantic import BaseModel, Field


class NeuronInfoLite(BaseModel):
    """
    Lightweight neuron information for efficient queries.
    
    This model contains only essential neuron data, reducing
    network and memory overhead for operations that don't need
    full neuron details.
    """
    
    # Identity (minimal)
    uid: int = Field(..., description="Unique neuron identifier", ge=0)
    hotkey: str = Field(..., description="Neuron hotkey address")
    
    # Network State (essential only)
    active: bool = Field(default=True, description="Whether neuron is active")
    subnet_uid: int = Field(..., description="Subnet this neuron belongs to", ge=0)
    
    # Stake (simplified)
    stake: float = Field(default=0.0, description="Total stake amount", ge=0)
    
    # Performance (key metrics only)
    rank: float = Field(default=0.0, description="Neuron rank score", ge=0, le=1)
    trust: float = Field(default=0.0, description="Trust score", ge=0, le=1)
    incentive: float = Field(default=0.0, description="Incentive score", ge=0, le=1)
    
    # Validator flag
    validator_permit: bool = Field(default=False, description="Has validator permit")
    
    class Config:
        json_schema_extra = {
            "example": {
                "uid": 0,
                "hotkey": "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                "active": True,
                "subnet_uid": 1,
                "stake": 1000.0,
                "rank": 0.95,
                "trust": 0.98,
                "incentive": 0.90,
                "validator_permit": True
            }
        }
    
    def __str__(self) -> str:
        return f"NeuronInfoLite(uid={self.uid}, stake={self.stake}, rank={self.rank})"
    
    def __repr__(self) -> str:
        return self.__str__()
