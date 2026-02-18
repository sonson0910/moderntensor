"""
Token Allocation and Vesting Module for ModernTensor SDK.

Implements the hybrid fundraising tokenomics:
- Pre-mint 55% at TGE (locked in vesting contracts)
- Emission 45% gradual over 10+ years
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional
from datetime import datetime, timedelta


# Total supply: 21,000,000 MDT (18 decimals)
TOTAL_SUPPLY = 21_000_000_000_000_000_000_000_000
DECIMALS = 18

# Maximum valid token amount (prevents integer overflow in arithmetic)
# Set to 2x total supply to allow headroom for intermediate calculations
MAX_TOKEN_AMOUNT = TOTAL_SUPPLY * 2


def validate_token_amount(amount: int, label: str = "amount") -> int:
    """
    Validate token amount is within safe bounds.

    Args:
        amount: Token amount to validate
        label: Human-readable label for error messages

    Returns:
        The validated amount

    Raises:
        ValueError: If amount is negative or exceeds MAX_TOKEN_AMOUNT
    """
    if amount < 0:
        raise ValueError(f"{label} cannot be negative: {amount}")
    if amount > MAX_TOKEN_AMOUNT:
        raise ValueError(
            f"{label} exceeds maximum ({amount} > {MAX_TOKEN_AMOUNT}). "
            "Possible integer overflow."
        )
    return amount


class AllocationCategory(Enum):
    """Token allocation categories."""

    EMISSION_REWARDS = "emission_rewards"      # 45% - Miners, Validators, Infrastructure
    ECOSYSTEM_GRANTS = "ecosystem_grants"      # 12% - Subnet grants, dApp builders
    TEAM_CORE_DEV = "team_core_dev"            # 10% - Founders and developers
    PRIVATE_SALE = "private_sale"              # 8% - VCs, Angels, Strategic
    IDO = "ido"                                # 5% - Community launchpad
    DAO_TREASURY = "dao_treasury"              # 10% - Operations, Marketing
    INITIAL_LIQUIDITY = "initial_liquidity"    # 5% - DEX/CEX pairs
    FOUNDATION_RESERVE = "foundation_reserve"  # 5% - Research, Emergency


# Allocation percentages (must sum to 100)
ALLOCATION_PERCENTAGES: Dict[AllocationCategory, int] = {
    AllocationCategory.EMISSION_REWARDS: 45,
    AllocationCategory.ECOSYSTEM_GRANTS: 12,
    AllocationCategory.TEAM_CORE_DEV: 10,
    AllocationCategory.PRIVATE_SALE: 8,
    AllocationCategory.IDO: 5,
    AllocationCategory.DAO_TREASURY: 10,
    AllocationCategory.INITIAL_LIQUIDITY: 5,
    AllocationCategory.FOUNDATION_RESERVE: 5,
}


@dataclass
class VestingSchedule:
    """
    Vesting schedule configuration.

    Attributes:
        cliff_days: Days before any tokens unlock
        linear_days: Days for linear vesting after cliff
        tge_percent: Percentage unlocked at TGE (0-100)
        description: Human-readable description
    """
    cliff_days: int = 0
    linear_days: int = 0
    tge_percent: int = 0
    description: str = ""

    def vested_amount(self, total: int, days_since_tge: int) -> int:
        """Calculate vested amount at a given day since TGE."""
        validate_token_amount(total, "total vesting amount")
        # TGE unlock
        tge_amount = total * self.tge_percent // 100
        vesting_amount = total - tge_amount

        if days_since_tge == 0:
            return tge_amount

        # During cliff
        if days_since_tge < self.cliff_days:
            return tge_amount

        # After cliff, calculate linear vesting
        days_after_cliff = days_since_tge - self.cliff_days

        if self.linear_days == 0:
            # No linear vesting, all unlocked after cliff
            return total

        # Linear vesting calculation
        if days_after_cliff >= self.linear_days:
            vested = vesting_amount  # Fully vested
        else:
            vested = vesting_amount * days_after_cliff // self.linear_days

        return tge_amount + vested

    def vesting_percentage(self, days_since_tge: int) -> float:
        """Get vesting percentage at a given day."""
        if days_since_tge == 0:
            return self.tge_percent / 100

        if days_since_tge < self.cliff_days:
            return self.tge_percent / 100

        days_after_cliff = days_since_tge - self.cliff_days

        if self.linear_days == 0:
            return 1.0

        remaining_percent = (100 - self.tge_percent) / 100
        if days_after_cliff >= self.linear_days:
            return 1.0
        else:
            linear_vested = remaining_percent * days_after_cliff / self.linear_days
            return self.tge_percent / 100 + linear_vested


# Vesting schedules for each category
VESTING_SCHEDULES: Dict[AllocationCategory, VestingSchedule] = {
    AllocationCategory.EMISSION_REWARDS: VestingSchedule(
        cliff_days=0,
        linear_days=3650,  # 10 years
        tge_percent=0,
        description="Emission over 10+ years"
    ),
    AllocationCategory.ECOSYSTEM_GRANTS: VestingSchedule(
        cliff_days=0,
        linear_days=0,  # DAO controlled
        tge_percent=0,
        description="DAO controlled release"
    ),
    AllocationCategory.TEAM_CORE_DEV: VestingSchedule(
        cliff_days=365,   # 1 year cliff
        linear_days=1460,  # 4 years linear
        tge_percent=0,
        description="1yr cliff + 4yr linear"
    ),
    AllocationCategory.PRIVATE_SALE: VestingSchedule(
        cliff_days=365,   # 1 year cliff
        linear_days=730,  # 2 years linear
        tge_percent=0,
        description="1yr cliff + 2yr linear"
    ),
    AllocationCategory.IDO: VestingSchedule(
        cliff_days=0,
        linear_days=180,  # 6 months
        tge_percent=25,   # 25% at TGE
        description="25% TGE + 6mo linear"
    ),
    AllocationCategory.DAO_TREASURY: VestingSchedule(
        cliff_days=0,
        linear_days=0,  # Multi-sig controlled
        tge_percent=0,
        description="Multi-sig controlled"
    ),
    AllocationCategory.INITIAL_LIQUIDITY: VestingSchedule(
        cliff_days=0,
        linear_days=0,
        tge_percent=100,  # Fully liquid for DEX
        description="Locked in liquidity pool"
    ),
    AllocationCategory.FOUNDATION_RESERVE: VestingSchedule(
        cliff_days=0,
        linear_days=0,  # Multi-sig controlled
        tge_percent=0,
        description="Multi-sig controlled"
    ),
}


@dataclass
class VestingEntry:
    """Individual vesting entry for a beneficiary."""

    beneficiary: str  # Address
    category: AllocationCategory
    total_amount: int
    claimed_amount: int = 0
    tge_timestamp: int = 0  # Unix timestamp
    revoked: bool = False

    def claimable(self, current_timestamp: int) -> int:
        """Calculate claimable amount."""
        if self.revoked:
            return 0

        days_since_tge = (current_timestamp - self.tge_timestamp) // 86400
        schedule = VESTING_SCHEDULES[self.category]
        vested = schedule.vested_amount(self.total_amount, days_since_tge)

        return max(0, vested - self.claimed_amount)


@dataclass
class TgeResult:
    """TGE execution result."""

    tge_timestamp: int
    pre_minted: Dict[AllocationCategory, int] = field(default_factory=dict)
    total_pre_minted: int = 0
    emission_reserved: int = 0


@dataclass
class AllocationStats:
    """Allocation statistics."""

    total_supply: int = TOTAL_SUPPLY
    total_pre_minted: int = 0
    emission_remaining: int = 0
    allocations: Dict[AllocationCategory, int] = field(default_factory=dict)


class TokenAllocation:
    """
    Token Allocation Manager.

    Handles pre-minting at TGE and vesting release schedules.
    """

    def __init__(self, tge_timestamp: Optional[int] = None):
        """
        Initialize token allocation.

        Args:
            tge_timestamp: Unix timestamp of TGE (Token Generation Event)
        """
        self.tge_timestamp = tge_timestamp or int(datetime.now().timestamp())
        self.minted: Dict[AllocationCategory, int] = {}
        self.vesting_entries: List[VestingEntry] = []
        self.total_minted = 0
        self.emission_pool = 0

    def execute_tge(self) -> TgeResult:
        """
        Execute Token Generation Event (TGE).

        Pre-mints tokens for all categories except EmissionRewards.

        Returns:
            TgeResult with pre-minted amounts
        """
        result = TgeResult(tge_timestamp=self.tge_timestamp)

        # Pre-mint for each category (except emission)
        for category in AllocationCategory:
            if category == AllocationCategory.EMISSION_REWARDS:
                continue

            amount = self.get_allocation_amount(category)
            self.minted[category] = amount
            self.total_minted += amount
            result.pre_minted[category] = amount

        # EmissionRewards are NOT pre-minted
        self.emission_pool = self.get_allocation_amount(AllocationCategory.EMISSION_REWARDS)
        result.emission_reserved = self.emission_pool
        result.total_pre_minted = self.total_minted

        return result

    def add_vesting(
        self,
        beneficiary: str,
        category: AllocationCategory,
        amount: int
    ) -> bool:
        """
        Add vesting entry for a beneficiary.

        Args:
            beneficiary: Wallet address
            category: Allocation category
            amount: Token amount to vest

        Returns:
            True if successful, False if insufficient balance
        """
        available = self.minted.get(category, 0)
        validate_token_amount(amount, "vesting amount")

        if amount > available:
            return False

        # Deduct from category pool
        self.minted[category] = available - amount

        # Create vesting entry
        entry = VestingEntry(
            beneficiary=beneficiary,
            category=category,
            total_amount=amount,
            claimed_amount=0,
            tge_timestamp=self.tge_timestamp,
            revoked=False
        )

        self.vesting_entries.append(entry)
        return True

    def claim(self, beneficiary: str, current_timestamp: Optional[int] = None) -> int:
        """
        Claim vested tokens.

        Args:
            beneficiary: Wallet address
            current_timestamp: Current timestamp (defaults to now)

        Returns:
            Total amount claimed
        """
        if current_timestamp is None:
            current_timestamp = int(datetime.now().timestamp())

        total_claimed = 0

        for entry in self.vesting_entries:
            if entry.beneficiary == beneficiary:
                claimable = entry.claimable(current_timestamp)
                if claimable > 0:
                    entry.claimed_amount += claimable
                    total_claimed += claimable

        return total_claimed

    def get_claimable(self, beneficiary: str, current_timestamp: Optional[int] = None) -> int:
        """Get total claimable amount for beneficiary."""
        if current_timestamp is None:
            current_timestamp = int(datetime.now().timestamp())

        return sum(
            entry.claimable(current_timestamp)
            for entry in self.vesting_entries
            if entry.beneficiary == beneficiary
        )

    def get_vested(self, beneficiary: str, current_timestamp: Optional[int] = None) -> int:
        """Get total vested amount for beneficiary."""
        if current_timestamp is None:
            current_timestamp = int(datetime.now().timestamp())

        total = 0
        for entry in self.vesting_entries:
            if entry.beneficiary == beneficiary:
                days = (current_timestamp - entry.tge_timestamp) // 86400
                schedule = VESTING_SCHEDULES[entry.category]
                total += schedule.vested_amount(entry.total_amount, days)

        return total

    def mint_emission(self, amount: int) -> int:
        """
        Mint from emission pool (for rewards).

        Args:
            amount: Amount to mint

        Returns:
            Amount actually minted

        Raises:
            ValueError if emission pool exhausted
        """
        validate_token_amount(amount, "emission amount")
        if amount > self.emission_pool:
            raise ValueError("Emission pool exhausted")

        self.emission_pool -= amount
        self.total_minted += amount
        return amount

    def remaining_emission(self) -> int:
        """Get remaining emission pool."""
        return self.emission_pool

    def stats(self) -> AllocationStats:
        """Get allocation statistics."""
        return AllocationStats(
            total_supply=TOTAL_SUPPLY,
            total_pre_minted=self.total_minted,
            emission_remaining=self.emission_pool,
            allocations=dict(self.minted)
        )

    @staticmethod
    def get_allocation_amount(category: AllocationCategory) -> int:
        """Get allocation amount for a category."""
        percentage = ALLOCATION_PERCENTAGES[category]
        return TOTAL_SUPPLY * percentage // 100

    @staticmethod
    def get_vesting_schedule(category: AllocationCategory) -> VestingSchedule:
        """Get vesting schedule for a category."""
        return VESTING_SCHEDULES[category]

    @staticmethod
    def format_amount(amount: int) -> str:
        """Format amount for display (18 decimals)."""
        whole = amount // (10 ** DECIMALS)
        frac = (amount % (10 ** DECIMALS)) // (10 ** (DECIMALS - 4))
        return f"{whole:,}.{frac:04d}"


# Default instance
DEFAULT_TOKEN_ALLOCATION = TokenAllocation()


# Utility functions
def calculate_vesting_timeline(
    category: AllocationCategory,
    total_amount: int,
    tge_date: datetime
) -> List[Dict]:
    """
    Calculate vesting timeline for an allocation.

    Args:
        category: Allocation category
        total_amount: Total token amount
        tge_date: TGE date

    Returns:
        List of milestone dicts with date, amount, percentage
    """
    schedule = VESTING_SCHEDULES[category]
    milestones = []

    # TGE
    milestones.append({
        "date": tge_date,
        "days": 0,
        "amount": schedule.vested_amount(total_amount, 0),
        "percentage": schedule.tge_percent
    })

    # Cliff end
    if schedule.cliff_days > 0:
        cliff_date = tge_date + timedelta(days=schedule.cliff_days)
        amount = schedule.vested_amount(total_amount, schedule.cliff_days)
        milestones.append({
            "date": cliff_date,
            "days": schedule.cliff_days,
            "amount": amount,
            "percentage": round(amount / total_amount * 100, 2)
        })

    # Monthly milestones during linear vesting
    if schedule.linear_days > 0:
        for month in range(1, (schedule.linear_days // 30) + 1):
            days = schedule.cliff_days + (month * 30)
            if days <= schedule.cliff_days + schedule.linear_days:
                date = tge_date + timedelta(days=days)
                amount = schedule.vested_amount(total_amount, days)
                milestones.append({
                    "date": date,
                    "days": days,
                    "amount": amount,
                    "percentage": round(amount / total_amount * 100, 2)
                })

    # Full vest
    total_days = schedule.cliff_days + max(schedule.linear_days, 1)
    full_vest_date = tge_date + timedelta(days=total_days)
    milestones.append({
        "date": full_vest_date,
        "days": total_days,
        "amount": total_amount,
        "percentage": 100
    })

    return milestones


# ============================================================================
# RPC Client Integration - Calls on-chain Luxtensor node
# ============================================================================

class TokenAllocationRPC:
    """
    Token Allocation RPC Client.

    Calls on-chain Luxtensor node RPC endpoints for allocation operations.
    Use this for production; use TokenAllocation (above) for local testing.
    """

    def __init__(self, rpc_url: str = "http://localhost:8545"):
        """
        Initialize RPC client.

        Args:
            rpc_url: Luxtensor node RPC endpoint
        """
        self.rpc_url = rpc_url
        self._request_id = 0

    def _call_rpc(self, method: str, params: Optional[Dict] = None) -> Dict:
        """Make JSON-RPC call."""
        import httpx

        self._request_id += 1
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or {},
            "id": self._request_id
        }

        response = httpx.post(self.rpc_url, json=payload)
        result = response.json()

        if "error" in result:
            raise Exception(f"RPC Error: {result['error']}")

        return result.get("result", {})

    def execute_tge(self) -> Dict:
        """Execute TGE on-chain."""
        return self._call_rpc("allocation_executeTge")

    def add_vesting(self, beneficiary: str, category: str, amount: str) -> Dict:
        """Add vesting entry on-chain."""
        return self._call_rpc("allocation_addVesting", {
            "beneficiary": beneficiary,
            "category": category,
            "amount": amount
        })

    def claim(self, beneficiary: str, timestamp: Optional[int] = None) -> Dict:
        """Claim vested tokens on-chain."""
        params = {"beneficiary": beneficiary}
        if timestamp:
            params["timestamp"] = timestamp
        return self._call_rpc("allocation_claim", params)

    def get_claimable(self, beneficiary: str, timestamp: Optional[int] = None) -> Dict:
        """Get claimable amount from on-chain."""
        params = {"beneficiary": beneficiary}
        if timestamp:
            params["timestamp"] = timestamp
        return self._call_rpc("allocation_getClaimable", params)

    def get_vested(self, beneficiary: str, timestamp: Optional[int] = None) -> Dict:
        """Get vested amount from on-chain."""
        params = {"beneficiary": beneficiary}
        if timestamp:
            params["timestamp"] = timestamp
        return self._call_rpc("allocation_getVested", params)

    def get_stats(self) -> Dict:
        """Get allocation stats from on-chain."""
        return self._call_rpc("allocation_getStats")

    def mint_emission(self, amount: str) -> Dict:
        """Mint from emission pool on-chain."""
        return self._call_rpc("allocation_mintEmission", {"amount": amount})

    def get_vesting_schedule(self, category: str) -> Dict:
        """Get vesting schedule for category."""
        return self._call_rpc("allocation_getVestingSchedule", {"category": category})

    def get_all_categories(self) -> List[Dict]:
        """Get all allocation categories with details."""
        return self._call_rpc("allocation_getAllCategories")


class NodeTierRPC:
    """
    Node Tier RPC Client.

    Calls on-chain Luxtensor node RPC endpoints for progressive staking.
    """

    def __init__(self, rpc_url: str = "http://localhost:8545"):
        """Initialize RPC client."""
        self.rpc_url = rpc_url
        self._request_id = 0

    def _call_rpc(self, method: str, params: Optional[Dict] = None) -> Dict:
        """Make JSON-RPC call."""
        import httpx

        self._request_id += 1
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or {},
            "id": self._request_id
        }

        response = httpx.post(self.rpc_url, json=payload)
        result = response.json()

        if "error" in result:
            raise Exception(f"RPC Error: {result['error']}")

        return result.get("result", {})

    def register(self, address: str, stake: str, block_height: Optional[int] = None) -> Dict:
        """Register node on-chain."""
        params = {"address": address, "stake": stake}
        if block_height:
            params["block_height"] = block_height
        return self._call_rpc("node_register", params)

    def update_stake(self, address: str, new_stake: str) -> Dict:
        """Update node stake on-chain."""
        return self._call_rpc("node_updateStake", {
            "address": address,
            "new_stake": new_stake
        })

    def get_tier(self, address: str) -> Dict:
        """Get node tier from on-chain."""
        return self._call_rpc("node_getTier", {"address": address})

    def get_info(self, address: str) -> Dict:
        """Get node info from on-chain."""
        return self._call_rpc("node_getInfo", {"address": address})

    def unregister(self, address: str) -> Dict:
        """Unregister node from on-chain."""
        return self._call_rpc("node_unregister", {"address": address})

    def get_validators(self) -> List[Dict]:
        """Get all validators from on-chain."""
        return self._call_rpc("node_getValidators")

    def get_infrastructure_nodes(self) -> List[Dict]:
        """Get all infrastructure nodes from on-chain."""
        return self._call_rpc("node_getInfrastructureNodes")

    def get_stats(self) -> Dict:
        """Get node stats from on-chain."""
        return self._call_rpc("node_getStats")

    def get_tier_requirements(self) -> Dict:
        """Get tier stake requirements."""
        return self._call_rpc("node_getTierRequirements")

