"""
Validator Information Model

Represents validator-specific information.
"""

from typing import Optional
from pydantic import BaseModel, Field


class ValidatorInfo(BaseModel):
    """Validator-specific information and metrics."""
    
    # Identity
    uid: int = Field(..., description="Validator UID", ge=0)
    hotkey: str = Field(..., description="Validator hotkey address")
    coldkey: str = Field(..., description="Validator coldkey address")
    
    # Validator Status
    validator_permit: bool = Field(default=True, description="Has validator permit")
    
    # Performance Metrics
    validator_trust: float = Field(default=0.0, description="Validator trust score", ge=0, le=1)
    total_stake: float = Field(default=0.0, description="Total stake including delegated", ge=0)
    own_stake: float = Field(default=0.0, description="Own stake amount", ge=0)
    delegated_stake: float = Field(default=0.0, description="Delegated stake amount", ge=0)
    
    # Activity
    last_update: int = Field(default=0, description="Last update block", ge=0)
    dividends: float = Field(default=0.0, description="Dividends from validation", ge=0, le=1)
    
    # Weights
    weights_set: bool = Field(default=False, description="Has set weights")
    weights_version: int = Field(default=0, description="Weights version", ge=0)
    
    # Axon Info
    axon_info: Optional[dict] = Field(default=None, description="Axon server information")
    
    class Config:
        json_json_schema_extra = {
            "example": {
                "uid": 0,
                "hotkey": "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                "coldkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "validator_permit": True,
                "validator_trust": 0.99,
                "total_stake": 5000.0,
                "own_stake": 1000.0,
                "delegated_stake": 4000.0,
                "last_update": 12345,
                "dividends": 0.88,
                "weights_set": True,
                "weights_version": 1
            }
        }
        
    def __str__(self) -> str:
        return f"ValidatorInfo(uid={self.uid}, stake={self.total_stake}, trust={self.validator_trust:.3f})"
    
    def __repr__(self) -> str:
        return self.__str__()
