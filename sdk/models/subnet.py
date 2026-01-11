"""
Subnet Information Models

Represents subnet metadata and configuration.
Aligned with SubnetData struct in luxtensor-storage/src/metagraph_store.rs
"""

from typing import List, Optional
from pydantic import BaseModel, Field


class SubnetHyperparameters(BaseModel):
    """
    Subnet hyperparameters and configuration.

    These parameters control subnet behavior.
    """

    # Network Parameters
    rho: float = Field(default=10.0, description="Rho parameter", ge=0)
    kappa: float = Field(default=10.0, description="Kappa parameter", ge=0)
    immunity_period: int = Field(default=7200, description="Immunity period in blocks", ge=0)

    # Validator Parameters
    min_allowed_weights: int = Field(default=0, description="Minimum allowed weights", ge=0)
    max_weights_limit: int = Field(default=65535, description="Maximum weights limit", ge=0)
    tempo: int = Field(default=100, description="Tempo (epoch length in blocks)", ge=1)

    # Stake Parameters (u128 in LuxTensor)
    min_stake: int = Field(default=0, description="Minimum stake required", ge=0)
    max_stake: Optional[int] = Field(default=None, description="Maximum stake allowed")

    # Weight Parameters
    weights_version: int = Field(default=0, description="Weights version", ge=0)
    weights_rate_limit: int = Field(default=100, description="Weights rate limit", ge=0)

    # Adjustment Parameters
    adjustment_interval: int = Field(default=100, description="Adjustment interval in blocks", ge=1)
    activity_cutoff: int = Field(default=5000, description="Activity cutoff in blocks", ge=0)

    # Max neurons (u16 in LuxTensor)
    max_neurons: int = Field(default=256, description="Maximum neurons allowed", ge=0)

    class Config:
        json_schema_extra = {
            "example": {
                "rho": 10.0,
                "kappa": 10.0,
                "immunity_period": 7200,
                "tempo": 100,
                "min_stake": 1000,
                "max_neurons": 256,
                "weights_rate_limit": 100,
                "adjustment_interval": 100,
                "activity_cutoff": 5000
            }
        }


class SubnetInfo(BaseModel):
    """
    Subnet metadata and state information.

    Matches LuxTensor SubnetData:
    - id: u64
    - name: String
    - owner: [u8; 20]
    - emission_rate: u128
    - created_at: u64
    - tempo: u16
    - max_neurons: u16
    - min_stake: u128
    - active: bool
    """

    # Identity (u64 in LuxTensor)
    id: int = Field(..., description="Unique subnet identifier", ge=0)
    subnet_uid: int = Field(default=0, description="Alias for id", ge=0)
    netuid: int = Field(default=0, description="Alias for id", ge=0)

    # Metadata
    name: str = Field(default="", description="Subnet name")
    owner: str = Field(..., description="Subnet owner address (0x...)")

    # Network State
    active: bool = Field(default=True, description="Whether subnet is active")
    n: int = Field(default=0, description="Current number of neurons", ge=0)
    max_neurons: int = Field(default=256, description="Maximum neurons allowed (u16)", ge=0)

    # Economic Parameters (u128 in LuxTensor)
    emission_rate: int = Field(default=0, description="Emission rate (u128)", ge=0)
    min_stake: int = Field(default=0, description="Minimum stake required (u128)", ge=0)

    # Timing (u16 in LuxTensor)
    tempo: int = Field(default=100, description="Tempo (epoch length)", ge=1)
    created_at: int = Field(default=0, description="Creation block/timestamp", ge=0)

    # Hyperparameters
    hyperparameters: Optional[SubnetHyperparameters] = Field(
        default=None,
        description="Subnet hyperparameters"
    )

    # Block Information
    block: int = Field(default=0, description="Current block number", ge=0)

    def __init__(self, **data):
        super().__init__(**data)
        # Ensure subnet_uid and netuid are synced with id
        if self.subnet_uid == 0:
            object.__setattr__(self, 'subnet_uid', self.id)
        if self.netuid == 0:
            object.__setattr__(self, 'netuid', self.id)

    @classmethod
    def from_rust_data(cls, data: dict) -> "SubnetInfo":
        """Create SubnetInfo from LuxTensor RPC response."""
        subnet_id = data.get("id", 0)
        return cls(
            id=subnet_id,
            subnet_uid=subnet_id,
            netuid=subnet_id,
            name=data.get("name", ""),
            owner=data.get("owner", "0x" + "0" * 40),
            active=data.get("active", True),
            max_neurons=data.get("max_neurons", 256),
            emission_rate=data.get("emission_rate", 0),
            min_stake=data.get("min_stake", 0),
            tempo=data.get("tempo", 100),
            created_at=data.get("created_at", 0),
        )

    class Config:
        json_schema_extra = {
            "example": {
                "id": 1,
                "subnet_uid": 1,
                "netuid": 1,
                "name": "AI Compute",
                "owner": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
                "active": True,
                "n": 100,
                "max_neurons": 256,
                "emission_rate": 1000000,
                "min_stake": 1000,
                "tempo": 100,
                "created_at": 123456,
                "block": 200000
            }
        }

    def __str__(self) -> str:
        return f"SubnetInfo(id={self.id}, name='{self.name}', neurons={self.n}/{self.max_neurons})"

    def __repr__(self) -> str:
        return self.__str__()
