# ModernTensor Staking Guide

## Overview

Staking is a core mechanism in ModernTensor that allows token holders to secure the network and earn rewards. This guide covers everything you need to know about staking MDT.

---

## Why Stake?

| Benefit | Description |
|---------|-------------|
| **Earn Rewards** | 6-25% APY depending on tier |
| **Secure Network** | Help maintain consensus |
| **Governance Rights** | Vote on proposals |
| **Lock Bonuses** | Up to +40% extra rewards |

---

## Staking Tiers

```
┌─────────────────────────────────────────────────────────────────┐
│                       STAKING TIERS                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  SUPER VALIDATOR          500,000+ MDT     15-25% APY           │
│  ████████████████████████████████████████████████████           │
│                                                                   │
│  VALIDATOR                100,000+ MDT     10-18% APY           │
│  ██████████████████████████████████████                         │
│                                                                   │
│  FULL NODE                 10,000+ MDT      6-12% APY           │
│  ████████████████████████                                        │
│                                                                   │
│  LIGHT NODE                    0 MDT         0% APY             │
│  ████████                                                        │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

### Tier Details

| Tier | Minimum Stake | APY Range | Voting Power | Lock Bonus |
|------|---------------|-----------|--------------|------------|
| Light Node | 0 | 0% | None | None |
| Full Node | 10,000 | 6-12% | 1x | +20% (90d) |
| Validator | 100,000 | 10-18% | 2x | +30% (180d) |
| Super Validator | 500,000 | 15-25% | 5x | +40% (365d) |

---

## How to Stake

### Option 1: Direct Staking (Self-Stake)

```python
from moderntensor import LuxtensorClient

client = LuxtensorClient("http://localhost:8545")

# Stake tokens
tx = client.stake(
    amount=100000,
    lock_period_days=180  # Optional lock for bonus
)
print(f"Staking TX: {tx}")

# Check stake
stake = client.get_stake("your_address")
print(f"Staked: {stake.amount} MDT")
print(f"APY: {stake.estimated_apy}%")
```

### Option 2: Delegation

Delegate to an existing validator without running your own node:

```python
# Delegate to validator
tx = client.delegate(
    validator_address="validator_address_here",
    amount=10000,
    lock_period_days=90
)
print(f"Delegation TX: {tx}")
```

**Delegation benefits:**
- No hardware required
- Earn staking rewards
- Validator takes small commission

---

## Lock Bonuses

Lock your stake for bonus rewards:

| Lock Period | Bonus Multiplier | Effective APY (Base 10%) |
|-------------|------------------|--------------------------|
| No lock | 1.0x | 10% |
| 90 days | 1.2x | 12% |
| 180 days | 1.3x | 13% |
| 365 days | 1.4x | 14% |

### Lock Mechanics

```
┌───────────────────────────────────────────────────────────────┐
│                     LOCK TIMELINE                              │
├───────────────────────────────────────────────────────────────┤
│                                                                 │
│    STAKE         LOCKED PERIOD           UNLOCK                │
│    └────────────────────────────────────────────────────►      │
│    │                                     │                      │
│    │  Earning bonus rewards              │  Withdraw            │
│    │  Cannot withdraw                    │  available           │
│    │                                     │                      │
│                                                                 │
└───────────────────────────────────────────────────────────────┘
```

**Notes:**
- Early unlock forfeits bonus rewards
- Base rewards still earned
- Lock resets if more tokens added

---

## Reward Calculation

### Formula

```
DailyReward = StakedAmount × BaseAPY × LockBonus × NetworkMultiplier
```

### Example

```python
staked_amount = 100000  # MDT
base_apy = 0.12  # 12%
lock_bonus = 1.3  # 180-day lock
network_multiplier = 1.0  # Full network activity

daily_rate = base_apy / 365
daily_reward = staked_amount * daily_rate * lock_bonus * network_multiplier

# daily_reward = 100000 * 0.000329 * 1.3 * 1.0
# daily_reward ≈ 42.74 MDT per day
# yearly_reward ≈ 15,600 MDT
```

### Reward Sources

| Source | Description |
|--------|-------------|
| **Block Rewards** | New tokens from emission |
| **Transaction Fees** | 30% of non-burned fees |
| **AI Task Fees** | Portion of AI service payments |

---

## Claiming Rewards

### View Pending Rewards

```python
rewards = client.get_pending_rewards("your_address")
print(f"Pending: {rewards.amount} MDT")
print(f"Claimable at: {rewards.epoch}")
```

### Claim Rewards

```python
# Claim all pending rewards
tx = client.claim_rewards()
print(f"Claimed: {tx['amount']} MDT")

# Or restake automatically
tx = client.restake_rewards()
print(f"Restaked: {tx['amount']} MDT")
```

**Claim Timing:**
- Rewards calculated per epoch (15 minutes)
- Claimable after epoch ends
- No minimum claim amount

---

## Unstaking

### Standard Unstake

```python
tx = client.unstake(amount=50000)
print(f"Unstake TX: {tx}")
```

**Unbonding Period:**
- 7 days for Full Node tier
- 14 days for Validator tier
- 21 days for Super Validator tier

### Early Unlock (Locked Stakes)

```python
# Early unlock with penalty
tx = client.force_unlock(
    amount=50000,
    accept_penalty=True
)
print(f"Penalty: {tx['penalty']} MDT")
```

**Penalty:** Forfeits all bonus rewards earned during lock period.

---

## Risk Management

### Slashing Conditions

| Offense | Penalty |
|---------|---------|
| Validator downtime (>24h) | 1% stake |
| Double signing | 5% stake |
| Invalid validation | 2% stake |

**Delegators:** Also affected by validator slashing proportionally.

### Mitigation

- Choose reliable validators
- Monitor validator performance
- Diversify across validators
- Check slashing history

---

## Staking Strategies

### Conservative
- Delegate to top validators
- Long lock periods
- Focus on stability

### Balanced
- Mix self-stake and delegation
- Medium lock periods
- Regular reward claims

### Active
- Run own validator
- Maximum lock bonus
- Compound rewards

---

## Best Practices

- [ ] Research validators before delegating
- [ ] Consider lock periods vs liquidity needs
- [ ] Monitor reward rates regularly
- [ ] Compound rewards for maximum growth
- [ ] Diversify across multiple validators
- [ ] Keep some liquid for opportunities

---

## FAQ

**Q: Can I stake and be a miner?**
A: Yes, roles are not mutually exclusive.

**Q: When do rewards start?**
A: From the first completed epoch after staking.

**Q: Is there a minimum stake?**
A: 10,000 MDT for Full Node tier rewards.

**Q: Can I add to existing stake?**
A: Yes, but lock timer resets if locked.

---

*Start earning today — stake your MDT!*
