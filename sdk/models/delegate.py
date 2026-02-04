"""
Delegate Information Model

Represents delegation information.
"""

from typing import List
from pydantic import BaseModel, Field


class DelegateInfo(BaseModel):
    """Delegate information including stake and nominators."""
    
    # Identity
    hotkey: str = Field(..., description="Delegate hotkey address")
    
    # Stake Information
    total_stake: float = Field(default=0.0, description="Total delegated stake", ge=0)
    nominators: List[str] = Field(default_factory=list, description="List of nominator addresses")
    
    # Commission
    take: float = Field(default=0.18, description="Delegate commission rate", ge=0, le=1)
    
    # Performance
    owner: str = Field(..., description="Owner coldkey address")
    registrations: List[int] = Field(default_factory=list, description="List of registered subnet UIDs")
    
    # Validator Info
    validator_permits: List[int] = Field(default_factory=list, description="Subnet UIDs where delegate has validator permit")
    
    # Return
    return_per_1000: float = Field(default=0.0, description="Return per 1000 TAO staked", ge=0)
    total_daily_return: float = Field(default=0.0, description="Total daily return", ge=0)
    
    class Config:
        json_schema_extra = {
            "example": {
                "hotkey": "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                "total_stake": 50000.0,
                "nominators": [
                    "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                    "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
                ],
                "take": 0.18,
                "owner": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "registrations": [0, 1, 2, 3],
                "validator_permits": [1, 2],
                "return_per_1000": 12.5,
                "total_daily_return": 625.0
            }
        }
        
    def __str__(self) -> str:
        return f"DelegateInfo(hotkey={self.hotkey[:8]}..., total_stake={self.total_stake}, nominators={len(self.nominators)})"
    
    def __repr__(self) -> str:
        return self.__str__()
