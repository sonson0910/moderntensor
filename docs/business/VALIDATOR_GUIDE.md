# ModernTensor Validator Guide

## Overview

Validators are the backbone of the ModernTensor network. They secure the blockchain through staking, validate transactions, and evaluate AI model quality. This guide covers everything you need to become a successful validator.

---

## Why Become a Validator?

| Benefit | Description |
|---------|-------------|
| **Earn Rewards** | 10-25% APY on staked tokens |
| **Governance Power** | Vote on protocol upgrades |
| **Network Security** | Help secure decentralized AI |
| **Early Access** | First to try new features |

---

## Requirements

### Hardware Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| **CPU** | 4 cores | 8+ cores |
| **RAM** | 16 GB | 32+ GB |
| **Storage** | 500 GB SSD | 1+ TB NVMe |
| **Network** | 100 Mbps | 1 Gbps |

### Software Requirements

- **OS:** Linux (Ubuntu 22.04 recommended)
- **Rust:** 1.70+
- **Python:** 3.10+ (for SDK)

### Stake Requirements

| Tier | MDT Required | APY Range |
|------|--------------|-----------|
| **Validator** | 100,000 MDT | 10-18% |
| **Super Validator** | 500,000 MDT | 15-25% |

---

## Getting Started

### Step 1: Install Dependencies

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install additional dependencies
sudo apt install build-essential pkg-config libssl-dev -y
```

### Step 2: Clone and Build

```bash
# Clone repository
git clone https://github.com/sonson0910/moderntensor.git
cd moderntensor/luxtensor

# Build the node
cargo build --release

# Verify installation
./target/release/luxtensor-node --version
```

### Step 3: Configure Node

```bash
# Create config directory
mkdir -p ~/.luxtensor

# Generate validator keys
./target/release/luxtensor-node key generate --output ~/.luxtensor/validator.key

# Create configuration file
cat > ~/.luxtensor/config.toml << EOF
[node]
name = "my-validator"
role = "validator"

[network]
listen_address = "/ip4/0.0.0.0/tcp/30333"
bootnodes = [
    # Testnet bootnodes
]

[consensus]
stake_amount = 100000

[rpc]
enabled = true
port = 8545
EOF
```

### Step 4: Start Node

```bash
# Start in background
./target/release/luxtensor-node \
    --config ~/.luxtensor/config.toml \
    --validator \
    --name "my-validator"
```

### Step 5: Register as Validator

```python
from moderntensor import LuxtensorClient

client = LuxtensorClient("http://localhost:8545")

# Register validator
tx = client.register_validator(
    stake=100000,
    commission_rate=0.05  # 5% commission
)
print(f"Registration TX: {tx}")
```

---

## Validator Operations

### Monitoring

```bash
# Check node status
curl http://localhost:8545/health

# Check sync status
./target/release/luxtensor-node status

# View logs
tail -f ~/.luxtensor/logs/node.log
```

### Key Commands

```bash
# Check validator status
./target/release/luxtensor-cli validator status

# View rewards
./target/release/luxtensor-cli validator rewards

# Withdraw rewards
./target/release/luxtensor-cli validator withdraw
```

---

## AI Validation Duties

Validators also evaluate AI model quality. Here's how:

### Quality Evaluation Process

```
1. RECEIVE TASK      2. RUN INFERENCE      3. SCORE OUTPUT      4. SUBMIT
   └─────────────────►   └─────────────────►   └────────────────►   └─────►

   Get random AI task    Execute on miner     Rate quality 0-1    Record on-chain
```

### Scoring Criteria

| Criterion | Weight | Description |
|-----------|--------|-------------|
| **Accuracy** | 40% | Correctness of output |
| **Latency** | 20% | Response time |
| **Availability** | 20% | Uptime percentage |
| **Consistency** | 20% | Reproducibility |

---

## Rewards & Economics

### Reward Sources

1. **Block Rewards** — For producing blocks
2. **Validation Fees** — From AI task evaluation
3. **Delegation Commission** — % of delegator rewards

### Reward Formula

```
ValidatorReward = (BlockReward × StakeShare) + (TaskFees × CommissionRate)
```

### Lock Bonuses

| Lock Period | Bonus |
|-------------|-------|
| No lock | 0% |
| 90 days | +20% |
| 180 days | +30% |
| 365 days | +40% |

---

## Slashing Conditions

⚠️ **Avoid these behaviors to keep your stake safe:**

| Offense | Penalty |
|---------|---------|
| **Double Signing** | 5% stake slashed |
| **Downtime (>24h)** | 1% stake slashed |
| **Invalid Validation** | 2% stake slashed |
| **Malicious Behavior** | Up to 100% slashed |

### Prevention Tips
- Run on reliable infrastructure
- Monitor uptime continuously
- Keep software updated
- Use secure key management

---

## Best Practices

### Security
- [ ] Use HSM for key storage
- [ ] Enable firewall
- [ ] Use separate monitoring node
- [ ] Regular security audits

### Operations
- [ ] Set up alerting (Grafana + PagerDuty)
- [ ] Automate updates
- [ ] Maintain backup node
- [ ] Document procedures

### Community
- [ ] Join validator Discord channel
- [ ] Participate in governance
- [ ] Share knowledge
- [ ] Report issues

---

## Troubleshooting

### Common Issues

**Node won't sync:**
```bash
# Check peer connections
./target/release/luxtensor-cli network peers

# Force resync
./target/release/luxtensor-node --force-resync
```

**Low block production:**
- Check network latency
- Verify stake is active
- Confirm sufficient peers

**Missing rewards:**
- Verify validator is in active set
- Check for slashing events
- Confirm blocks are being produced

---

## Support

- **Documentation:** /docs in repository
- **GitHub Issues:** Technical support
- **Discord:** Community help (coming soon)

---

*Thank you for helping secure the ModernTensor network!*
