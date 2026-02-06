"""
Luxtensor Validator Model

Pydantic model for Luxtensor validators matching the RPC schema from
`staking_getValidators` endpoint.

This model is designed to match the Luxtensor PoS architecture, NOT the
Bittensor-style schema (uid, hotkey, coldkey).
"""

from typing import Optional
from pydantic import BaseModel, Field, field_validator


class LuxtensorValidator(BaseModel):
    """
    Validator info matching Luxtensor RPC `staking_getValidators` response.

    Luxtensor RPC returns:
    ```json
    {
        "address": "0x...",
        "stake": "0x...",
        "stakeDecimal": "...",
        "stakedAt": 1234567890
    }
    ```
    """

    address: str = Field(
        ...,
        description="Validator address in 0x-prefixed hex format"
    )
    stake: int = Field(
        default=0,
        ge=0,
        description="Stake amount in base units (wei)"
    )
    stake_decimal: str = Field(
        default="0",
        description="Human-readable stake amount"
    )
    staked_at: int = Field(
        default=0,
        ge=0,
        description="Unix timestamp when stake was made"
    )
    is_validator: bool = Field(
        default=False,
        description="Whether stake meets minimum validator threshold"
    )

    # Luxtensor-specific fields (from Rust Validator struct)
    activation_epoch: Optional[int] = Field(
        default=None,
        ge=0,
        description="Epoch when validator becomes eligible (epoch delay)"
    )
    active: bool = Field(
        default=True,
        description="Whether the validator is currently active"
    )
    rewards: int = Field(
        default=0,
        ge=0,
        description="Accumulated rewards in base units"
    )
    last_active_slot: int = Field(
        default=0,
        ge=0,
        description="Last slot this validator was active"
    )

    @field_validator('address')
    @classmethod
    def validate_address(cls, v: str) -> str:
        """Ensure address is properly formatted."""
        if not v.startswith('0x'):
            v = f'0x{v}'
        if len(v) != 42:  # 0x + 40 hex chars
            raise ValueError(f"Invalid address length: {len(v)}, expected 42")
        return v.lower()

    @classmethod
    def from_rpc_response(cls, data: dict) -> "LuxtensorValidator":
        """
        Create from Luxtensor RPC response.

        Handles both hex and decimal stake formats.
        """
        address = data.get("address", "0x" + "0" * 40)

        # Parse stake (hex or decimal)
        stake_raw = data.get("stake", "0")
        if isinstance(stake_raw, str) and stake_raw.startswith("0x"):
            stake = int(stake_raw, 16)
        else:
            stake = int(stake_raw) if stake_raw else 0

        return cls(
            address=address,
            stake=stake,
            stake_decimal=data.get("stakeDecimal", str(stake)),
            staked_at=data.get("stakedAt", 0),
            is_validator=data.get("isValidator", False),
            activation_epoch=data.get("activationEpoch"),
            active=data.get("active", True),
            rewards=data.get("rewards", 0),
            last_active_slot=data.get("lastActiveSlot", 0),
        )

    def to_rpc_format(self) -> dict:
        """Convert to RPC-compatible format."""
        return {
            "address": self.address,
            "stake": f"0x{self.stake:x}",
            "stakeDecimal": self.stake_decimal,
            "stakedAt": self.staked_at,
            "isValidator": self.is_validator,
        }

    @property
    def stake_lts(self) -> float:
        """Get stake in LTS tokens (18 decimals)."""
        return self.stake / 10**18


class LuxtensorValidatorSet(BaseModel):
    """Collection of validators from RPC response."""

    validators: list[LuxtensorValidator] = Field(default_factory=list)
    count: int = Field(default=0)

    @classmethod
    def from_rpc_response(cls, data: dict) -> "LuxtensorValidatorSet":
        """Create from staking_getValidators RPC response."""
        validators_data = data.get("validators", [])
        validators = [
            LuxtensorValidator.from_rpc_response(v)
            for v in validators_data
        ]
        return cls(
            validators=validators,
            count=data.get("count", len(validators))
        )

    @property
    def total_stake(self) -> int:
        """Get total stake across all validators."""
        return sum(v.stake for v in self.validators)

    @property
    def active_validators(self) -> list[LuxtensorValidator]:
        """Get only active validators."""
        return [v for v in self.validators if v.active]
