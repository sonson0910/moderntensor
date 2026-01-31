# ADR-004: Token Economics (Tokenomics)

**Status**: Accepted
**Date**: 2026-01-31
**Authors**: Luxtensor Team

---

## Context

MDT (ModernTensor Token) is the native token powering the Luxtensor network. The tokenomics must:

1. Incentivize validators and miners fairly
2. Prevent inflation death spiral
3. Discourage whale concentration
4. Fund ongoing development via DAO treasury

## Decision

### Token Supply

| Parameter | Value |
|-----------|-------|
| **Total Supply** | 21,000,000 MDT |
| **Initial Emission** | 24% (5,040,000 MDT) |
| **Block Emission** | 45% (9,450,000 MDT) |
| **DAO Treasury** | 10% (2,100,000 MDT) |
| **Team/Advisors** | 15% (3,150,000 MDT) |
| **Community/Ecosystem** | 6% (1,260,000 MDT) |

### Block Rewards

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| **Initial Reward** | 2 MDT/block | Sustainable emission rate |
| **Block Time** | 3 seconds | ~10,512,000 blocks/year |
| **Halving Interval** | 1,051,200 blocks | ~3.3 years |
| **Max Halvings** | 10 | 33+ years of emission |

### Halving Schedule

| Halving # | Reward | Cumulative Years |
|-----------|--------|------------------|
| 0 | 2.000 MDT | 0 |
| 1 | 1.000 MDT | 3.3 |
| 2 | 0.500 MDT | 6.6 |
| 3 | 0.250 MDT | 9.9 |
| ... | ... | ... |
| 10 | 0.002 MDT | 33 |

### Staking & Vesting

| Category | Cliff | Vesting | Lock Bonus |
|----------|-------|---------|------------|
| Team | 1 year | 4 years linear | N/A |
| Validators | None | None | 0.5%/month (max 6%) |
| Delegators | None | None | 0.25%/month (max 3%) |

### Anti-Whale Mechanics

1. **Progressive Stake Decay**: Large stakes receive diminishing returns
2. **Delegation Limits**: Max 10% of network stake per validator
3. **Lock Bonus Cap**: Maximum 6% bonus prevents infinite lockup incentive

### Slashing Economics

| Offense | Slash % | Burn % | Treasury % |
|---------|---------|--------|------------|
| Double Signing | 50% | 80% | 20% |
| Offline | 10% | 80% | 20% |
| No Reveal | 5% | 80% | 20% |

## Consequences

### Positive

- Predictable supply curve (Bitcoin-like)
- Sustainable long-term emission
- Treasury funding for development

### Negative

- Initial inflation (~20%/year)
- Lock bonus favors long-term holders

### Risks

- Token velocity issues (mitigated by staking incentives)
- DAO treasury governance (future improvement)

## Implementation

- `luxtensor-consensus/src/block_reward.rs` - Reward calculation
- `luxtensor-consensus/src/halving.rs` - Halving schedule
- `sdk/tokenomics/` - Python tokenomics utilities
