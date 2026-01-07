"""
Subnet Information Models

Represents subnet metadata and configuration.
"""

from typing import List, Optional
from pydantic import BaseModel, Field


class SubnetHyperparameters(BaseModel):
    """Subnet hyperparameters and configuration."""
    
    # Network Parameters
    rho: float = Field(default=10.0, description="Rho parameter", ge=0)
    kappa: float = Field(default=32767.0, description="Kappa parameter", ge=0)
    immunity_period: int = Field(default=7200, description="Immunity period in blocks", ge=0)
    
    # Validator Parameters
    min_allowed_weights: int = Field(default=0, description="Minimum allowed weights", ge=0)
    max_weights_limit: int = Field(default=65535, description="Maximum weights limit", ge=0)
    tempo: int = Field(default=99, description="Tempo (epoch length in blocks)", ge=1)
    
    # Stake Parameters
    min_stake: float = Field(default=0.0, description="Minimum stake required", ge=0)
    max_stake: Optional[float] = Field(default=None, description="Maximum stake allowed")
    
    # Weight Parameters
    weights_version: int = Field(default=0, description="Weights version", ge=0)
    weights_rate_limit: int = Field(default=100, description="Weights rate limit", ge=0)
    
    # Adjustment Parameters
    adjustment_interval: int = Field(default=112, description="Adjustment interval in blocks", ge=1)
    adjustment_alpha: float = Field(default=0.0, description="Adjustment alpha", ge=0, le=1)
    
    # Consensus Parameters
    bonds_moving_avg: float = Field(default=900000.0, description="Bonds moving average", ge=0)
    max_regs_per_block: int = Field(default=1, description="Max registrations per block", ge=0)
    
    # Serving Parameters
    serving_rate_limit: int = Field(default=100, description="Serving rate limit", ge=0)
    
    # Target Parameters
    target_regs_per_interval: int = Field(default=2, description="Target registrations per interval", ge=0)
    
    # Difficulty Parameters
    difficulty: float = Field(default=10000000.0, description="Registration difficulty", ge=0)
    
    class Config:
        json_schema_extra = {
            "example": {
                "rho": 10.0,
                "kappa": 32767.0,
                "immunity_period": 7200,
                "min_allowed_weights": 0,
                "max_weights_limit": 65535,
                "tempo": 99,
                "min_stake": 1000.0,
                "weights_version": 0,
                "weights_rate_limit": 100,
                "adjustment_interval": 112,
                "adjustment_alpha": 0.0,
                "bonds_moving_avg": 900000.0,
                "max_regs_per_block": 1,
                "serving_rate_limit": 100,
                "target_regs_per_interval": 2,
                "difficulty": 10000000.0
            }
        }


class SubnetInfo(BaseModel):
    """Subnet metadata and state information."""
    
    # Identity
    subnet_uid: int = Field(..., description="Unique subnet identifier", ge=0)
    netuid: int = Field(..., description="Network UID (alias for subnet_uid)", ge=0)
    
    # Metadata
    name: str = Field(default="", description="Subnet name")
    owner: str = Field(..., description="Subnet owner address")
    
    # Network State
    n: int = Field(default=0, description="Number of neurons in subnet", ge=0)
    max_n: int = Field(default=4096, description="Maximum neurons allowed", ge=0)
    
    # Registration
    emission_value: float = Field(default=0.0, description="Emission value", ge=0)
    tempo: int = Field(default=99, description="Tempo (epoch length)", ge=1)
    
    # Hyperparameters
    hyperparameters: Optional[SubnetHyperparameters] = Field(
        default=None,
        description="Subnet hyperparameters"
    )
    
    # Block Information
    block: int = Field(default=0, description="Current block number", ge=0)
    
    # Economic Parameters
    burn: float = Field(default=0.0, description="Registration burn amount", ge=0)
    
    # Connection Info
    connect: List[int] = Field(default_factory=list, description="Connected subnet UIDs")
    
    class Config:
        json_schema_extra = {
            "example": {
                "subnet_uid": 1,
                "netuid": 1,
                "name": "Text Prompting",
                "owner": "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                "n": 256,
                "max_n": 4096,
                "emission_value": 1000000.0,
                "tempo": 99,
                "block": 123456,
                "burn": 1.0,
                "connect": [0, 2, 3]
            }
        }
        
    def __str__(self) -> str:
        return f"SubnetInfo(uid={self.subnet_uid}, name='{self.name}', n={self.n}/{self.max_n})"
    
    def __repr__(self) -> str:
        return self.__str__()
