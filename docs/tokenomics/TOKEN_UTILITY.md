# ModernTensor Token Utility Guide

## Overview

The ModernTensor Token (MDT) is the native cryptocurrency powering the ModernTensor network. This guide explains all the ways MDT can be used within the ecosystem.

---

## Token Summary

| Parameter | Value |
|-----------|-------|
| **Token Name** | ModernTensor Token |
| **Symbol** | MDT |
| **Max Supply** | 21,000,000 |
| **Initial Supply** | 0 (emission-based) |
| **Decimals** | 18 |

---

## Primary Use Cases

### 1. Staking

**Purpose:** Secure the network and earn rewards

```
┌─────────────────────────────────────────────────────────────┐
│                     STAKING TIERS                            │
├─────────────────────────┬───────────────────────────────────┤
│ Light Node              │ 0 MDT (participate only)          │
├─────────────────────────┼───────────────────────────────────┤
│ Full Node               │ 10,000 MDT → 6-12% APY            │
├─────────────────────────┼───────────────────────────────────┤
│ Validator               │ 100,000 MDT → 10-18% APY          │
├─────────────────────────┼───────────────────────────────────┤
│ Super Validator         │ 500,000 MDT → 15-25% APY          │
└─────────────────────────┴───────────────────────────────────┘
```

**Benefits:**
- Earn passive income
- Participate in governance
- Secure the network
- Lock bonuses up to +40%

---

### 2. Transaction Fees

**Purpose:** Pay for network operations

| Transaction Type | Typical Fee |
|------------------|-------------|
| Simple Transfer | ~0.001 MDT |
| Smart Contract Call | ~0.01-0.1 MDT |
| AI Task Submission | ~0.1-1.0 MDT |
| Subnet Registration | 100 MDT |

**Note:** 50% of transaction fees are burned, creating deflationary pressure.

---

### 3. AI Service Payments

**Purpose:** Access decentralized AI compute

```python
from moderntensor import LuxtensorClient

client = LuxtensorClient()

# Submit AI task and pay in MDT
result = client.submit_ai_task({
    "model": "text-generation",
    "input": "Explain blockchain in simple terms",
    "max_tokens": 500,
    "payment": 0.5  # MDT to pay for task
})
```

**Use cases:**
- LLM inference
- Image generation
- Code completion
- Custom AI tasks

---

### 4. Governance

**Purpose:** Vote on protocol decisions

| Governance Action | Voting Power |
|-------------------|--------------|
| Protocol Upgrades | 1 MDT = 1 Vote |
| Parameter Changes | 1 MDT = 1 Vote |
| Treasury Spending | 1 MDT = 1 Vote |
| Subnet Management | 1 MDT = 1 Vote |

**Governance scope:**
- Emission parameters
- Burn rates
- Staking requirements
- Network upgrades

---

### 5. Subnet Registration

**Purpose:** Create specialized AI subnets

| Action | Cost |
|--------|------|
| Register new subnet | 100 MDT (burned) |
| Modify subnet params | 10 MDT fee |
| Transfer subnet ownership | 50 MDT fee |

---

## Economic Flows

### Value Creation Loop

```
┌──────────────────────────────────────────────────────────────────┐
│                      MDT VALUE FLOW                               │
├──────────────────────────────────────────────────────────────────┤
│                                                                    │
│    ┌─────────┐      ┌─────────┐      ┌─────────┐                 │
│    │  USERS  │ ───▶ │ NETWORK │ ───▶ │ MINERS  │                 │
│    │  (Pay)  │      │  (Fees) │      │ (Earn)  │                 │
│    └────┬────┘      └────┬────┘      └────┬────┘                 │
│         │                │                │                        │
│         │           ┌────┴────┐           │                        │
│         │           │  BURN   │           │                        │
│         │           │  (50%)  │           │                        │
│         │           └─────────┘           │                        │
│         │                                 │                        │
│         └────────────────┬────────────────┘                        │
│                          ▼                                         │
│                   ┌─────────────┐                                  │
│                   │  VALIDATORS │                                  │
│                   │  (Secure)   │                                  │
│                   └──────┬──────┘                                  │
│                          │                                         │
│                          ▼                                         │
│                   ┌─────────────┐                                  │
│                   │   STAKERS   │                                  │
│                   │   (Yield)   │                                  │
│                   └─────────────┘                                  │
│                                                                    │
└──────────────────────────────────────────────────────────────────┘
```

---

## Revenue Sharing

Non-burned fees are distributed as follows:

| Recipient | Share | Purpose |
|-----------|-------|---------|
| Validators | 40% | Block production rewards |
| Stakers | 30% | Passive staking yield |
| Treasury | 20% | Ecosystem development |
| DAO | 10% | Community governance |

---

## Token Mechanics

### Emission (Creation)

Tokens are minted through adaptive emission:

```python
MintAmount = max(
    BaseReward × UtilityScore × QualityMultiplier × HalvingFactor,
    MinEmissionFloor × HalvingFactor
)
```

### Burns (Destruction)

Four mechanisms reduce supply:

1. **Transaction Fees** — 50% burned
2. **Subnet Registration** — 100% burned
3. **Unmet Quotas** — 100 MDT burned
4. **Slashing** — 50% of slashed amount burned

---

## Acquiring MDT

### During Token Generation Event (TGE)
- Private sale allocation
- Public sale / IDO

### After Launch
- Buy on exchanges
- Earn through staking
- Earn as miner/validator
- Ecosystem grants

---

## Summary

| Use Case | Who Benefits |
|----------|--------------|
| Staking | All token holders |
| Fees | Network users |
| AI Services | Developers, enterprises |
| Governance | Community |
| Subnet Registration | Innovators |

---

*MDT: The fuel for decentralized intelligence*
