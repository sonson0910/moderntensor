# ModernTensor Tokenomics v3.0 ($MDT)

**The Superior Decentralized AI Economic Model**

---

## Executive Summary

ModernTensor ($MDT) implements a next-generation tokenomics model designed to surpass Bittensor's fixed emission approach. Our **Adaptive Emission + Multi-Layer Deflation** system creates sustainable value through:

1. **Utility-based minting** (vs. Bittensor's fixed 7,200 TAO/day)
2. **Recycling pool** before new minting
3. **4 burn mechanisms** for long-term scarcity
4. **Performance-based rewards** for quality
5. **DAO treasury** for sustainable development

---

## 1. Core Token Metrics

| Metric | ModernTensor (MDT) | Bittensor (TAO) | Advantage |
|--------|-------------------|-----------------|-----------|
| **Max Supply** | 21,000,000 | 21,000,000 | Equal |
| **Emission Type** | Adaptive (0-100%) | Fixed | ✅ Superior |
| **Daily Max Emission** | 0 - 2,876 | 7,200 fixed | ✅ Deflationary |
| **Halving Trigger** | Epochs + Supply | Supply only | ✅ Faster |
| **Burn Mechanisms** | 4 types | None | ✅ Deflationary |
| **Recycling Pool** | Yes | Partial | ✅ Efficient |

---

## 2. Token Distribution (Final)

### 2.1 Total Supply Allocation

| Category | % | Amount (MDT) | Vesting | Purpose |
|----------|---|--------------|---------|---------|
| **Emission Rewards** | 45% | 9,450,000 | 10+ years | Miners, Validators, Stakers |
| **Ecosystem Grants** | 12% | 2,520,000 | DAO controlled | dApp builders |
| **Team & Core Dev** | 10% | 2,100,000 | 1yr cliff + 4yr | Founders, devs |
| **Private Investors** | 8% | 1,680,000 | 1yr cliff + 4yr | Seed funding |
| **Community Sale** | 8% | 1,680,000 | 6mo linear | Public |
| **Research Fund** | 5% | 1,050,000 | Foundation | AI research |
| **Strategic Partners** | 5% | 1,050,000 | 6mo cliff + 3yr | Exchanges, Labs |
| **DAO Treasury** | 5% | 1,050,000 | DAO votes | Operations |
| **Foundation Reserve** | 2% | 420,000 | Multi-sig | Emergency |

### 2.2 Epoch Reward Distribution (v3 Final)

The 45% Emission Rewards are distributed each epoch:

| Recipient | % of Emission | % of Total Supply | Description |
|-----------|---------------|-------------------|-------------|
| **Miners** | 35% | 15.75% | Performance-based |
| **Validators** | 30% | 13.5% | Stake-based |
| **Delegators** | 12% | 5.4% | Passive stakers + lock bonus |
| **Subnet Owners** | 10% | 4.5% | Subnet creators |
| **DAO Treasury** | 13% | 5.85% | Protocol operations |

### 2.3 Delegator Lock Bonuses

| Lock Period | Bonus | Effective Rate |
|-------------|-------|----------------|
| No lock | 0% | 1.0x |
| 30 days | +10% | 1.1x |
| 90 days | +25% | 1.25x |
| 180 days | +50% | 1.5x |
| 365 days | +100% | 2.0x |

---

## 3. Adaptive Emission Model

### 3.1 Core Formula

```
MintAmount = max(
    BaseReward × UtilityScore × QualityMultiplier × HalvingFactor,
    MinEmissionFloor × HalvingFactor
)
```

### 3.2 Components

| Component | Description | Range |
|-----------|-------------|-------|
| **BaseReward** | Initial base per epoch | 1,000 MDT |
| **UtilityScore** | Network activity score | 0.0 - 1.0 |
| **QualityMultiplier** | Consensus quality | 0.6 - 1.4 |
| **HalvingFactor** | 0.5^n per halving | 1.0 → 0.5 → 0.25... |
| **MinEmissionFloor** | Prevents death spiral | 100 MDT/day |

### 3.3 Utility Score Formula

```
UtilityScore = α₁×TaskVolume + α₂×Difficulty + α₃×Participation

Where: α₁=0.4, α₂=0.3, α₃=0.3
```

---

## 4. Burn Mechanisms (4 Types)

### 4.1 Overview

| Burn Type | Rate | Impact |
|-----------|------|--------|
| **Transaction Fees** | 50% burned | ~100K MDT/year |
| **Subnet Registration** | 50% burned, 50% recycled | ~25K MDT/year |
| **Unmet Quota** | 100% burned | Variable |
| **Slashing** | 80% burned | ~20K MDT/year |

**Note:** Inactive account burn removed (controversial, user trust issue).

### 4.2 Estimated Annual Burns

```
Conservative: 100,000 MDT/year
Moderate:     175,000 MDT/year
Aggressive:   250,000 MDT/year
```

---

## 5. New Economic Mechanisms (v3)

### 5.1 Buyback & Burn

| Parameter | Value |
|-----------|-------|
| Source | 15% of protocol revenue |
| Frequency | Weekly |
| Max Slippage | 2% |
| Execution | TWAP orders |

### 5.2 Revenue Sharing (Real Yield)

Non-burned tx fees distributed:

| Recipient | Share |
|-----------|-------|
| Stakers | 60% |
| Validators | 30% |
| DAO | 10% |

### 5.3 Referral Program

| Reward | Amount | Duration |
|--------|--------|----------|
| New user bonus | +5% rewards | 30 days |
| Referrer reward | 2% of referee | 30 days |

### 5.4 Builder Incentives (Year 1)

| Incentive | Details |
|-----------|---------|
| Free subnet registration | Until 2027-01-01 |
| Subnet grant pool | 100K MDT per subnet |
| Bug bounties | 500K MDT total |
| Integration bonus | 10K MDT per integration |

---

## 6. Comparison with Bittensor

| Feature | Bittensor | ModernTensor | Winner |
|---------|-----------|--------------|--------|
| **Daily Emission** | 7,200 fixed | 0-2,876 adaptive | ✅ MDT |
| **Burn Mechanisms** | 0 | 4 types | ✅ MDT |
| **Delegator Rewards** | 0% | 12% | ✅ MDT |
| **DAO Treasury** | Limited | 13% of emission | ✅ MDT |
| **Lock Bonuses** | No | Up to 2x | ✅ MDT |
| **Builder Incentives** | Limited | Comprehensive | ✅ MDT |
| **Revenue Sharing** | No | Yes (real yield) | ✅ MDT |

---

## 7. Long-term Projections

### 7.1 Emission Schedule

| Year | Max Daily | Min Daily | Bittensor | Reduction |
|------|-----------|-----------|-----------|-----------|
| 1 | 2,000 | 100 | 7,200 | 72-99% |
| 5 | 500 | 50 | 1,800 | 72-97% |
| 10 | 250 | 25 | 450 | 44-94% |

### 7.2 Supply Trajectory

With burns and adaptive emission:

- **Year 5:** ~8-10M circulating
- **Year 10:** ~12-15M circulating
- **Year 20:** ~18-19M circulating (approaching max)

---

## 8. Governance

All tokenomics parameters are governable by DAO:

- Emission rates
- Burn rates
- Distribution percentages
- Lock bonus tiers
- Revenue share ratios

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| v1.0 | 2025-Q4 | Initial design |
| v2.0 | 2026-01 | Adaptive emission, 5 burns |
| v3.0 | 2026-01 | Simplified to 4 burns, lock bonus, revenue sharing, builder incentives |

---

*ModernTensor Foundation - Building the Future of Decentralized AI*
