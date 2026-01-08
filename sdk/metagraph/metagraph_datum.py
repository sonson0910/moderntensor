# sdk/metagraph/metagraph_datum.py
"""
Định nghĩa cấu trúc Datum cho các thành phần trong Metagraph (Miner, Validator, Subnet)

**DEPRECATION NOTICE:**
This file uses Cardano/Plutus concepts (PlutusData, Datum) which are incompatible with Luxtensor.
Luxtensor uses an account-based model, not UTXO/Datum model.

These data structures are kept for backward compatibility during migration but should be
replaced with Pydantic models that serialize to JSON for Luxtensor state storage.

TODO: Migrate to pure Pydantic models without PlutusData inheritance
TODO: Use LuxtensorClient for state queries instead of UTXO/Datum pattern
"""
from sdk.compat.luxtensor_types import PlutusData, Redeemer
from dataclasses import dataclass, field
from typing import Optional, List, Union

# --- Import settings để lấy divisor ---
try:
    from sdk.config.settings import settings

    DATUM_INT_DIVISOR = settings.METAGRAPH_DATUM_INT_DIVISOR
except ImportError:
    print(
        "Warning: Could not import settings for DATUM_INT_DIVISOR. Using default 1_000_000.0"
    )
    DATUM_INT_DIVISOR = 1_000_000.0

# --- Định nghĩa các hằng số trạng thái ---
STATUS_INACTIVE = 0  # Chưa đăng ký hoặc đã hủy đăng ký
STATUS_ACTIVE = 1  # Đang hoạt động
STATUS_JAILED = 2  # Bị phạt, tạm khóa hoạt động


@dataclass
class MinerDatum(PlutusData):
    """Datum lưu trữ trạng thái của một Miner trên blockchain."""

    CONSTR_ID = 0 # type: ignore
    uid: bytes
    subnet_uid: int
    stake: int
    scaled_last_performance: int  # Đã scale (x DIVISOR)
    scaled_trust_score: int  # Đã scale (x DIVISOR)
    accumulated_rewards: int
    last_update_slot: int  # Slot cuối cùng Datum này được cập nhật
    performance_history_hash: bytes
    wallet_addr_hash: bytes
    status: int  # 0: Inactive, 1: Active, 2: Jailed
    registration_slot: int
    api_endpoint: bytes

    @property
    def trust_score(self) -> float:
        """Trả về trust score dạng float."""
        return (
            self.scaled_trust_score / DATUM_INT_DIVISOR
        )  # <<<--- Sử dụng divisor từ settings

    @property
    def last_performance(self) -> float:
        """Trả về performance dạng float."""
        return (
            self.scaled_last_performance / DATUM_INT_DIVISOR
        )  # <<<--- Sử dụng divisor từ settings


@dataclass
class ValidatorDatum(PlutusData):
    """Datum lưu trữ trạng thái của một Validator trên blockchain."""

    CONSTR_ID = 0 # type: ignore
    uid: bytes
    subnet_uid: int
    stake: int
    scaled_last_performance: int  # Đã scale (x DIVISOR)
    scaled_trust_score: int  # Đã scale (x DIVISOR)
    accumulated_rewards: int
    last_update_slot: int
    performance_history_hash: bytes
    wallet_addr_hash: bytes
    status: int  # 0: Inactive, 1: Active, 2: Jailed
    registration_slot: int
    api_endpoint: bytes

    @property
    def trust_score(self) -> float:
        """Trả về trust score dạng float."""
        return (
            self.scaled_trust_score / DATUM_INT_DIVISOR
        )  # <<<--- Sử dụng divisor từ settings

    @property
    def last_performance(self) -> float:
        """Trả về performance dạng float."""
        return (
            self.scaled_last_performance / DATUM_INT_DIVISOR
        )  # <<<--- Sử dụng divisor từ settings


@dataclass
class SubnetStaticDatum(PlutusData):
    """Datum lưu trữ thông tin tĩnh, ít thay đổi của một Subnet."""

    CONSTR_ID = 0 # type: ignore
    net_uid: int
    name: bytes
    owner_addr_hash: bytes
    max_miners: int
    max_validators: int
    immunity_period_slots: int
    creation_slot: int
    description: bytes
    version: int
    min_stake_miner: int
    min_stake_validator: int
    # governance_mechanism: int
    # consensus_params_hash: Optional[bytes]


@dataclass
class SubnetDynamicDatum(PlutusData):
    """Datum lưu trữ thông tin động, thường xuyên thay đổi của một Subnet."""

    CONSTR_ID = 0 # type: ignore
    net_uid: int
    scaled_weight: int  # Đã scale (x DIVISOR)
    scaled_performance: int  # Đã scale (x DIVISOR)
    current_epoch: int
    registration_open: int
    reg_cost: int
    scaled_incentive_ratio: int  # Đã scale (x DIVISOR)
    last_update_slot: int
    total_stake: int
    validator_count: int
    miner_count: int
    # emission_per_epoch: int
    # activity_score: int

    @property
    def weight(self) -> float:
        """Trả về weight dạng float."""
        return (
            self.scaled_weight / DATUM_INT_DIVISOR
        )  # <<<--- Sử dụng divisor từ settings

    @property
    def performance(self) -> float:
        """Trả về performance dạng float."""
        return (
            self.scaled_performance / DATUM_INT_DIVISOR
        )  # <<<--- Sử dụng divisor từ settings


# --- Redeemer Definitions (Ví dụ) ---
# @dataclass
# class UpdateMinerRedeemer(Redeemer):
#     # Redeemer tag (0, 1, 2...) for different actions
#     tag: int = 0
#     # Data needed by the script for this action
#     new_performance: int
#     new_trust_score: int
