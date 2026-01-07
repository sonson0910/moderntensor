"""
Stake Information Model

Represents staking information for neurons.
"""

from pydantic import BaseModel, Field


class StakeInfo(BaseModel):
    """Staking information for a neuron."""
    
    # Identity
    hotkey: str = Field(..., description="Hotkey address of the staked neuron")
    coldkey: str = Field(..., description="Coldkey address of the staker")
    
    # Stake Amount
    stake: float = Field(..., description="Amount of stake", ge=0)
    
    # Metadata
    block: int = Field(default=0, description="Block number when stake was recorded", ge=0)
    
    class Config:
        json_schema_extra = {
            "example": {
                "hotkey": "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                "coldkey": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "stake": 1000.0,
                "block": 12345
            }
        }
        
    def __str__(self) -> str:
        return f"StakeInfo(hotkey={self.hotkey[:8]}..., stake={self.stake})"
    
    def __repr__(self) -> str:
        return self.__str__()
