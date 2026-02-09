"""
Subnet Information Models

Represents subnet metadata and configuration.
Aligned with SubnetData struct in luxtensor-storage/src/metagraph_store.rs
"""

from typing import Optional
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

    Matches LuxTensor RPC SubnetInfo:
    - id: u64
    - name: String
    - owner: String
    - emission_rate: u128
    - participant_count: usize
    - total_stake: u128
    - created_at: u64

    Note: subnet_rpc.rs may return camelCase keys (totalStake, registeredAt, emissionShare).
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
    participant_count: int = Field(default=0, description="Number of participants in subnet", ge=0)
    total_stake: int = Field(default=0, description="Total stake in subnet (u128)", ge=0)

    # Optional fields (may not be returned by the Rust RPC server)
    n: Optional[int] = Field(default=None, description="Current number of neurons")
    max_neurons: Optional[int] = Field(default=None, description="Maximum neurons allowed (u16)")

    # Economic Parameters (u128 in LuxTensor)
    emission_rate: int = Field(default=0, description="Emission rate (u128)", ge=0)
    min_stake: Optional[int] = Field(default=None, description="Minimum stake required (u128)")

    # Timing
    tempo: Optional[int] = Field(default=None, description="Tempo (epoch length)")
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
        """Create SubnetInfo from LuxTensor RPC response.

        Handles both snake_case (from types.rs) and camelCase (from subnet_rpc.rs) field names.
        """
        subnet_id = data.get("id", 0)

        # Handle both snake_case and camelCase for emission
        emission_rate = (
            data.get("emission_rate")
            or data.get("emissionShare")
            or data.get("emission_share")
            or 0
        )

        # Handle both snake_case and camelCase for created_at
        created_at = (
            data.get("created_at")
            or data.get("registeredAt")
            or data.get("registered_at")
            or 0
        )

        # Handle both snake_case and camelCase for total_stake
        total_stake = (
            data.get("total_stake")
            or data.get("totalStake")
            or 0
        )

        # Handle both snake_case and camelCase for participant_count
        participant_count = (
            data.get("participant_count")
            or data.get("participantCount")
            or 0
        )

        return cls(
            id=subnet_id,
            subnet_uid=subnet_id,
            netuid=subnet_id,
            name=data.get("name", ""),
            owner=data.get("owner", "0x" + "0" * 40),
            active=data.get("active", True),
            participant_count=participant_count,
            total_stake=total_stake,
            n=data.get("n"),
            max_neurons=data.get("max_neurons") or data.get("maxNeurons"),
            emission_rate=emission_rate,
            min_stake=data.get("min_stake") or data.get("minStake"),
            tempo=data.get("tempo"),
            created_at=created_at,
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
                "participant_count": 100,
                "total_stake": 5000000000,
                "emission_rate": 1000000,
                "created_at": 123456,
                "block": 200000
            }
        }

    def __str__(self) -> str:
        return f"SubnetInfo(id={self.id}, name='{self.name}', participants={self.participant_count})"

    def __repr__(self) -> str:
        return self.__str__()


# =============================================================================
# Subnet 0 (Root Subnet) Types
# Synced with luxtensor-core/src/subnet.rs
# =============================================================================

class RootConfig(BaseModel):
    """
    Configuration for Root Subnet (Subnet 0).

    Synced with Rust: luxtensor-core/src/subnet.rs::RootConfig
    """
    max_subnets: int = Field(default=32, description="Maximum number of subnets", ge=1)
    max_root_validators: int = Field(default=64, description="Top N stakers become root validators", ge=1)
    min_stake_for_root: int = Field(
        default=1_000_000_000_000_000_000_000,  # 1000 MDT in LTS
        description="Minimum stake to be root validator (LTS)"
    )
    subnet_registration_cost: int = Field(
        default=100_000_000_000_000_000_000,  # 100 tokens
        description="Cost to register subnet (burned)"
    )
    weight_update_interval: int = Field(default=100, description="Blocks between weight updates", ge=1)
    emission_tempo: int = Field(default=360, description="Blocks per emission cycle", ge=1)

    class Config:
        json_schema_extra = {
            "example": {
                "max_subnets": 32,
                "max_root_validators": 64,
                "min_stake_for_root": 1000000000000000000000,
                "subnet_registration_cost": 100000000000000000000,
                "weight_update_interval": 100,
                "emission_tempo": 360
            }
        }


class RootValidatorInfo(BaseModel):
    """
    Information about a Root Validator (top staker in Subnet 0).

    Synced with Rust: luxtensor-core/src/subnet.rs::RootValidatorInfo
    """
    address: str = Field(..., description="Validator address (0x...)")
    stake: int = Field(default=0, description="Total stake amount (LTS)", ge=0)
    rank: int = Field(default=0, description="Rank among root validators (1-64)", ge=0)
    is_active: bool = Field(default=True, description="Whether validator is active")
    last_weight_update: int = Field(default=0, description="Block of last weight update", ge=0)

    class Config:
        json_schema_extra = {
            "example": {
                "address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
                "stake": 10000000000000000000000,
                "rank": 1,
                "is_active": True,
                "last_weight_update": 12345
            }
        }


class SubnetWeights(BaseModel):
    """
    Weight votes from a root validator for subnets.

    Synced with Rust: luxtensor-core/src/subnet.rs::SubnetWeights
    """
    validator: str = Field(..., description="Validator address")
    weights: dict = Field(default_factory=dict, description="netuid -> weight (0.0-1.0)")
    block_updated: int = Field(default=0, description="Block when last updated", ge=0)

    def normalize(self) -> None:
        """Normalize weights to sum to 1.0"""
        total = sum(self.weights.values())
        if total > 0:
            self.weights = {k: v / total for k, v in self.weights.items()}

    def is_valid(self) -> bool:
        """Check if weights are valid (all values 0.0-1.0)"""
        return all(0.0 <= v <= 1.0 for v in self.weights.values())

    class Config:
        json_schema_extra = {
            "example": {
                "validator": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
                "weights": {"1": 0.4, "2": 0.35, "3": 0.25},
                "block_updated": 12345
            }
        }


class EmissionShare(BaseModel):
    """
    Computed emission share for a subnet after weight aggregation.

    Synced with Rust: luxtensor-node/src/root_subnet.rs::EmissionShare
    """
    netuid: int = Field(..., description="Subnet ID", ge=0)
    share: float = Field(default=0.0, description="Emission share (0.0-1.0)", ge=0, le=1)
    amount: int = Field(default=0, description="Actual token amount for epoch (wei)", ge=0)

    class Config:
        json_schema_extra = {
            "example": {
                "netuid": 1,
                "share": 0.35,
                "amount": 350000000000000000000
            }
        }


class SubnetRegistrationResult(BaseModel):
    """Result of subnet registration."""
    success: bool = Field(..., description="Whether registration succeeded")
    netuid: Optional[int] = Field(default=None, description="Assigned subnet ID")
    tx_hash: Optional[str] = Field(default=None, description="Transaction hash")
    error: Optional[str] = Field(default=None, description="Error message if failed")
    cost_burned: int = Field(default=0, description="Amount burned for registration", ge=0)

    class Config:
        json_schema_extra = {
            "example": {
                "success": True,
                "netuid": 5,
                "tx_hash": "0x1234...",
                "error": None,
                "cost_burned": 100000000000000000000
            }
        }
