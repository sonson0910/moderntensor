"""
ModernTensor Tokenomics Configuration.

Implements Option A1 token distribution:
- 45% Emission Rewards (miners, validators, stakers)
- 12% Ecosystem Grants
- 10% Team & Core Dev
- 8% Private Investors
- 8% Community Sale
- 5% Research Fund
- 5% Strategic Partners
- 5% DAO Treasury
- 2% Foundation Reserve
"""

from dataclasses import dataclass, field
from typing import Optional, Dict
from enum import Enum


class VestingSchedule(Enum):
    """Token vesting schedules."""
    NONE = "none"
    LINEAR_6M = "6_month_linear"
    CLIFF_6M_LINEAR_3Y = "6mo_cliff_3yr_linear"
    CLIFF_1Y_LINEAR_4Y = "1yr_cliff_4yr_linear"
    DAO_CONTROLLED = "dao_controlled"
    FOUNDATION_MANAGED = "foundation_managed"


@dataclass
class AllocationInfo:
    """Token allocation information."""
    name: str
    percentage: float
    amount: int
    vesting: VestingSchedule
    description: str


@dataclass
class TokenomicsConfig:
    """
    Configuration for tokenomics system (Option A1).

    Key Parameters:
        - max_supply: 21,000,000 MDT (fixed, like Bitcoin)
        - base_reward: Daily base reward before adjustments
        - halving_interval: Epochs between halvings (~2 years)
        - min_emission_floor: Minimum daily emission to ensure incentives
    """
    # Core token parameters
    max_supply: int = 21_000_000
    base_reward: int = 1000  # Base reward per epoch
    halving_interval: int = 210_000  # ~2 years at 10min/epoch

    # Adaptive emission parameters
    max_expected_tasks: int = 10_000
    utility_weights: tuple = (0.4, 0.3, 0.3)  # (task, difficulty, participation)

    # Minimum emission floor (prevents death spiral)
    min_daily_emission: int = 100  # Minimum 100 MDT/day even if network idle

    # Token allocation percentages (Option A1)
    allocations: Dict[str, float] = field(default_factory=lambda: {
        "emission_rewards": 0.45,      # 45% - Miners, Validators, Stakers
        "ecosystem_grants": 0.12,      # 12% - dApp builders, integrations
        "team_core_dev": 0.10,         # 10% - Founders, developers, advisors
        "private_investors": 0.08,     # 8% - Seed/Series A
        "community_sale": 0.08,        # 8% - Public distribution
        "research_fund": 0.05,         # 5% - AI/Crypto research
        "strategic_partners": 0.05,    # 5% - Exchanges, Labs
        "dao_treasury": 0.05,          # 5% - Operating expenses
        "foundation_reserve": 0.02,    # 2% - Emergency fund
    })

    def __post_init__(self):
        """Validate configuration."""
        # Validate utility weights sum to 1.0
        w1, w2, w3 = self.utility_weights
        if not abs(w1 + w2 + w3 - 1.0) < 0.001:
            raise ValueError(f"Utility weights must sum to 1.0, got {w1 + w2 + w3}")

        # Validate allocations sum to 1.0
        total = sum(self.allocations.values())
        if not abs(total - 1.0) < 0.001:
            raise ValueError(f"Allocations must sum to 1.0, got {total}")

    def get_allocation_amount(self, category: str) -> int:
        """Get token amount for an allocation category."""
        if category not in self.allocations:
            raise ValueError(f"Unknown allocation category: {category}")
        return int(self.max_supply * self.allocations[category])

    def get_all_allocations(self) -> Dict[str, AllocationInfo]:
        """Get detailed allocation information."""
        return {
            "emission_rewards": AllocationInfo(
                name="Emission Rewards",
                percentage=45.0,
                amount=9_450_000,
                vesting=VestingSchedule.NONE,
                description="Ongoing rewards for miners, validators, and stakers"
            ),
            "ecosystem_grants": AllocationInfo(
                name="Ecosystem Grants",
                percentage=12.0,
                amount=2_520_000,
                vesting=VestingSchedule.DAO_CONTROLLED,
                description="Funding for dApp builders and integrations"
            ),
            "team_core_dev": AllocationInfo(
                name="Team & Core Dev",
                percentage=10.0,
                amount=2_100_000,
                vesting=VestingSchedule.CLIFF_1Y_LINEAR_4Y,
                description="Founders, developers, advisors"
            ),
            "private_investors": AllocationInfo(
                name="Private Investors",
                percentage=8.0,
                amount=1_680_000,
                vesting=VestingSchedule.CLIFF_1Y_LINEAR_4Y,
                description="Seed and Series A investors"
            ),
            "community_sale": AllocationInfo(
                name="Community Sale",
                percentage=8.0,
                amount=1_680_000,
                vesting=VestingSchedule.LINEAR_6M,
                description="Public token distribution"
            ),
            "research_fund": AllocationInfo(
                name="Research Fund",
                percentage=5.0,
                amount=1_050_000,
                vesting=VestingSchedule.FOUNDATION_MANAGED,
                description="AI and cryptographic research"
            ),
            "strategic_partners": AllocationInfo(
                name="Strategic Partners",
                percentage=5.0,
                amount=1_050_000,
                vesting=VestingSchedule.CLIFF_6M_LINEAR_3Y,
                description="Exchanges, Labs, strategic VCs"
            ),
            "dao_treasury": AllocationInfo(
                name="DAO Treasury",
                percentage=5.0,
                amount=1_050_000,
                vesting=VestingSchedule.DAO_CONTROLLED,
                description="Operating expenses and community initiatives"
            ),
            "foundation_reserve": AllocationInfo(
                name="Foundation Reserve",
                percentage=2.0,
                amount=420_000,
                vesting=VestingSchedule.FOUNDATION_MANAGED,
                description="Emergency fund and black swan protection"
            ),
        }


@dataclass
class DistributionConfig:
    """
    Configuration for epoch reward distribution (FINAL v3).

    IMPORTANT: This config distributes the EMISSION REWARDS portion (45%)
    among network participants on each epoch.

    This is a SUB-DIVISION of the 45% emission_rewards allocation:

        45% Emission Rewards (from Allocations)
            ├── 35% → Miners (performance-based)
            ├── 30% → Validators (stake-based)
            ├── 12% → Delegators (stake-based + lock bonus)
            ├── 10% → Subnet Owners (emission-based)
            └── 13% → DAO Treasury

    In absolute terms (of total 21M supply):
        - Miners get: 45% × 35% = 15.75% of total supply
        - Validators get: 45% × 30% = 13.5% of total supply
        - Delegators get: 45% × 12% = 5.4% of total supply
        - Subnet Owners get: 45% × 10% = 4.5% of total supply
        - DAO gets: 45% × 13% = 5.85% of total supply

    v3 CHANGES:
        - Gộp Staking Bonus Pool vào Delegators (lock bonus in formula)
        - Delegators: 12% (với lock bonus up to 2x)
        - DAO: 13% (tăng từ 10% để có thêm runway)
    """
    miner_share: float = 0.35          # 35% - Core compute providers
    validator_share: float = 0.30      # 30% - Quality assurance
    delegator_share: float = 0.12      # 12% - Passive stakers (with lock bonus)
    subnet_owner_share: float = 0.10   # 10% - Subnet creators
    dao_share: float = 0.13            # 13% - Protocol treasury (increased)

    # Delegator lock bonus (replaces separate staking bonus pool)
    lock_bonus_30d: float = 0.10       # +10% for 30-day lock
    lock_bonus_90d: float = 0.25       # +25% for 90-day lock
    lock_bonus_180d: float = 0.50      # +50% for 180-day lock
    lock_bonus_365d: float = 1.00      # +100% (2x) for 1-year lock

    def __post_init__(self):
        """Validate configuration."""
        total = (
            self.miner_share +
            self.validator_share +
            self.delegator_share +
            self.subnet_owner_share +
            self.dao_share
        )
        if not abs(total - 1.0) < 0.001:
            raise ValueError(f"Distribution shares must sum to 1.0, got {total}")


@dataclass
class BurnConfig:
    """
    Configuration for token burn mechanisms (v3 - simplified).

    4 burn types:
    1. Transaction fees: 50% burned
    2. Subnet registration: 50% burned, 50% recycled to grants
    3. Unmet quota: 100% burned
    4. Slashing: 80% burned

    REMOVED: Inactive account burn (controversial, user trust issue)
    """
    transaction_fee_burn_rate: float = 0.50      # 50% of tx fees burned
    subnet_registration_burn_rate: float = 0.50  # 50% burned (was 100%)
    subnet_registration_recycle_rate: float = 0.50  # 50% to grants (NEW)
    unmet_quota_burn_rate: float = 1.00          # 100% of unmet quota burned
    slashing_burn_rate: float = 0.80             # 80% of slashed stake burned
    # inactive_account_burn_rate: REMOVED (controversial)


@dataclass
class BuybackConfig:
    """
    Buyback and burn configuration (NEW).

    Protocol revenue is used to buy MDT from market and burn.
    """
    buyback_percentage: float = 0.15  # 15% of protocol revenue
    min_buyback_amount: int = 1000    # Minimum 1000 MDT per buyback
    buyback_frequency: str = "weekly"  # Weekly buyback events
    max_price_impact: float = 0.02    # Max 2% slippage
    use_twap: bool = True             # Time-weighted average price


@dataclass
class RevenueShareConfig:
    """
    Revenue sharing configuration (NEW).

    Non-burned transaction fees are distributed as real yield.
    """
    staker_share: float = 0.60       # 60% to stakers
    validator_share: float = 0.30    # 30% to validators
    dao_share: float = 0.10          # 10% to DAO
    min_stake_for_share: int = 100   # Min 100 MDT staked
    distribution_frequency: str = "daily"


@dataclass
class ReferralConfig:
    """Referral program configuration (NEW)."""
    referee_bonus: float = 0.05      # 5% bonus for new user
    referrer_reward: float = 0.02    # 2% of referee's rewards
    duration_days: int = 30          # 30-day program
    max_referrals_per_user: int = 50 # Cap to prevent abuse
    min_stake_to_qualify: int = 100  # Min 100 MDT stake


@dataclass
class BuilderIncentiveConfig:
    """Builder incentive program - Year 1 only (NEW)."""
    free_registration_until: str = "2027-01-01"
    subnet_grant_pool: int = 100_000  # 100K MDT per subnet
    max_subsidized_subnets: int = 10
    bug_bounty_pool: int = 500_000
    integration_bonus: int = 10_000


@dataclass
class GrantConfig:
    """Configuration for ecosystem grants."""
    micro_grant_min: int = 1_000
    micro_grant_max: int = 10_000
    standard_grant_min: int = 10_000
    standard_grant_max: int = 100_000
    major_grant_min: int = 100_000
    major_grant_max: int = 500_000
    annual_budget_percentage: float = 0.20


# Default configurations
DEFAULT_TOKENOMICS_CONFIG = TokenomicsConfig()
DEFAULT_DISTRIBUTION_CONFIG = DistributionConfig()
DEFAULT_BURN_CONFIG = BurnConfig()
DEFAULT_GRANT_CONFIG = GrantConfig()
DEFAULT_BUYBACK_CONFIG = BuybackConfig()
DEFAULT_REVENUE_SHARE_CONFIG = RevenueShareConfig()
DEFAULT_REFERRAL_CONFIG = ReferralConfig()
DEFAULT_BUILDER_INCENTIVE_CONFIG = BuilderIncentiveConfig()
