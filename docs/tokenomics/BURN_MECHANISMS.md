# ModernTensor Burn Mechanisms

## Overview

ModernTensor implements four distinct burn mechanisms to create deflationary pressure and maintain long-term token value. Unlike networks with only emission (inflation), MDT has structural burns that reduce supply over time.

---

## Why Burns Matter

```
Traditional Crypto Economics:
┌───────────────────────────────────────────────────────────────┐
│  EMISSION ────────────────────────────────────▶ SELL PRESSURE │
│  (constant)                                                    │
└───────────────────────────────────────────────────────────────┘

ModernTensor Economics:
┌───────────────────────────────────────────────────────────────┐
│  EMISSION (adaptive) ──▶ NETWORK ACTIVITY ──▶ BURNS           │
│         │                      │                  │            │
│         ▼                      ▼                  ▼            │
│  Less when idle          More when active    Reduces supply   │
└───────────────────────────────────────────────────────────────┘
```

**Result:** Network activity creates value, not just sell pressure.

---

## The Four Burn Types

### 1. Transaction Fee Burns (50%)

**How it works:**
Half of all base transaction fees are permanently burned.

| Transaction | Typical Fee | Amount Burned |
|-------------|-------------|---------------|
| Simple Transfer | 0.001 MDT | 0.0005 MDT |
| Contract Call | 0.01 MDT | 0.005 MDT |
| AI Task | 0.5 MDT | 0.25 MDT |

**Impact:** Higher network usage = more burns

```python
# Example calculation
daily_transactions = 100_000
avg_fee = 0.01  # MDT
burn_rate = 0.5

daily_burn = daily_transactions * avg_fee * burn_rate
# daily_burn = 100,000 * 0.01 * 0.5 = 500 MDT/day
# yearly_burn = 182,500 MDT/year
```

---

### 2. Subnet Registration Burns (100%)

**How it works:**
When someone registers a new subnet, the registration fee is 100% burned.

| Action | Fee | Burned |
|--------|-----|--------|
| New Subnet Registration | 100 MDT | 100 MDT |

**Impact:** Ecosystem growth = permanent supply reduction

**Example scenario:**
- 50 subnets registered in Year 1
- Total burned: 5,000 MDT

---

### 3. Unmet Quota Burns

**How it works:**
Miners who fail to meet quality thresholds face quota burns.

| Condition | Burn Amount |
|-----------|-------------|
| Quality score < threshold | 100 MDT |
| Consecutive failures | Progressive |

**Purpose:**
- Incentivizes quality over quantity
- Removes low-quality actors
- Maintains network standards

**Example:**
```
Miner submits low-quality output
  → Quality score: 0.3 (threshold: 0.5)
  → Penalty: 100 MDT burned from stake
  → Repeated offenses: escalating penalties
```

---

### 4. Slashing Burns (50%)

**How it works:**
When validators are slashed for misbehavior, 50% of slashed tokens are burned.

| Offense | Slash Amount | Amount Burned |
|---------|--------------|---------------|
| Double Signing | 5% stake | 2.5% stake |
| Long Downtime | 1% stake | 0.5% stake |
| Invalid Validation | 2% stake | 1% stake |

**Example:**
```
Validator with 100,000 MDT stake double-signs
  → Slashed: 5,000 MDT (5%)
  → Burned: 2,500 MDT (50% of slash)
  → To Treasury: 2,500 MDT (50%)
```

---

## Combined Burn Projections

### Conservative Scenario (Year 1)

| Burn Type | Assumption | Annual Burn |
|-----------|------------|-------------|
| Transaction Fees | 50K tx/day avg | 91,250 MDT |
| Subnet Registration | 20 subnets | 2,000 MDT |
| Unmet Quotas | 10 incidents/wk | 52,000 MDT |
| Slashing | 5 major events | 12,500 MDT |
| **Total** | | **157,750 MDT** |

### Optimistic Scenario (Year 3)

| Burn Type | Assumption | Annual Burn |
|-----------|------------|-------------|
| Transaction Fees | 500K tx/day | 912,500 MDT |
| Subnet Registration | 100 subnets | 10,000 MDT |
| Unmet Quotas | Reduced (better quality) | 25,000 MDT |
| Slashing | Fewer (network mature) | 5,000 MDT |
| **Total** | | **952,500 MDT** |

---

## Net Supply Calculation

```
Net New Supply = Gross Emission - Total Burns
```

### Example Year 1

```
Gross Emission:   1,000,000 MDT (if high utility)
Total Burns:      - 157,750 MDT
─────────────────────────────────────────
Net New Supply:    842,250 MDT

Inflation Rate:    842,250 / 21,000,000 = 4.01%
(Much lower than many PoS networks at 8-15%)
```

### Long-Term Projection

```
Year    Gross Emission    Burns    Net Supply    Total Supply
────────────────────────────────────────────────────────────────
Y1      1,000,000        157,750     842,250      842,250
Y2        800,000        250,000     550,000    1,392,250
Y3        600,000        450,000     150,000    1,542,250
Y4        400,000        500,000    -100,000    1,442,250  ← Deflationary!
Y5        300,000        600,000    -300,000    1,142,250
```

**Note:** As network matures, burns can exceed emission = deflationary.

---

## Comparison to Other Networks

| Network | Burn Mechanisms | Emission Type |
|---------|-----------------|---------------|
| **ModernTensor** | 4 types | Adaptive |
| Bittensor | None | Fixed 7,200/day |
| Ethereum | EIP-1559 (fees) | PoS issuance |
| Bitcoin | None | Fixed halving |

**ModernTensor Advantage:** Multiple burn sources + adaptive emission = most sustainable economics.

---

## Technical Implementation

### Fee Burn

```rust
// From luxtensor-consensus/src/burn_manager.rs
pub fn burn_transaction_fee(fee: u128) -> BurnResult {
    let burn_amount = fee * BURN_RATE / 100;  // 50%
    supply_manager.burn(burn_amount);
    emit_burn_event(BurnType::Fee, burn_amount);
    BurnResult::Success(burn_amount)
}
```

### Slash Burn

```rust
pub fn slash_validator(validator: &Address, offense: SlashType) -> SlashResult {
    let slash_amount = calculate_slash(validator.stake, offense);
    let burn_amount = slash_amount / 2;  // 50%

    supply_manager.burn(burn_amount);
    treasury.deposit(slash_amount - burn_amount);

    SlashResult::Success { slashed: slash_amount, burned: burn_amount }
}
```

---

## Summary

| Mechanism | Rate | Purpose |
|-----------|------|---------|
| Transaction Fees | 50% burned | Align usage with value |
| Subnet Registration | 100% burned | Prevent spam, add friction |
| Unmet Quotas | 100 MDT | Quality enforcement |
| Slashing | 50% burned | Security + deflation |

---

*Four burns, one goal: sustainable tokenomics.*
