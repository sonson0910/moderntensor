"""
Miner Information Model

Represents miner-specific information.
"""

from typing import Optional
from pydantic import BaseModel, Field


class MinerInfo(BaseModel):
    """Miner-specific information and metrics."""
    
    # Identity
    uid: int = Field(..., description="Miner UID", ge=0)
    hotkey: str = Field(..., description="Miner hotkey address")
    coldkey: str = Field(..., description="Miner coldkey address")
    
    # Performance Metrics
    rank: float = Field(default=0.0, description="Rank score", ge=0, le=1)
    trust: float = Field(default=0.0, description="Trust score", ge=0, le=1)
    consensus: float = Field(default=0.0, description="Consensus weight", ge=0, le=1)
    incentive: float = Field(default=0.0, description="Incentive score", ge=0, le=1)
    emission: float = Field(default=0.0, description="Token emission rate", ge=0)
    
    # Stake
    stake: float = Field(default=0.0, description="Stake amount", ge=0)
    
    # Activity
    active: bool = Field(default=True, description="Is active")
    last_update: int = Field(default=0, description="Last update block", ge=0)
    
    # Axon Info
    axon_info: Optional[dict] = Field(default=None, description="Axon server information")
    
    # Prometheus Info
    prometheus_info: Optional[dict] = Field(default=None, description="Prometheus metrics endpoint")
    
    class Config:
        json_json_schema_extra = {
            "example": {
                "uid": 100,
                "hotkey": "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                "coldkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "rank": 0.95,
                "trust": 0.92,
                "consensus": 0.90,
                "incentive": 0.88,
                "emission": 50.5,
                "stake": 100.0,
                "active": True,
                "last_update": 12345
            }
        }
        
    def __str__(self) -> str:
        return f"MinerInfo(uid={self.uid}, rank={self.rank:.3f}, incentive={self.incentive:.3f})"
    
    def __repr__(self) -> str:
        return self.__str__()
