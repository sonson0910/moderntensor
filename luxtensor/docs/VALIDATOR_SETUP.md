# Luxtensor Validator Setup Guide

This guide explains how to set up and run a Luxtensor validator node.

---

## Prerequisites

### Hardware Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 4 cores | 8+ cores |
| RAM | 8 GB | 16+ GB |
| Storage | 100 GB SSD | 500+ GB NVMe |
| Network | 50 Mbps | 100+ Mbps |

### Software Requirements

- Ubuntu 22.04 LTS (or equivalent Linux)
- Rust 1.75+ (for building from source)
- Docker 24+ (for containerized deployment)

---

## Rewards & Tokenomics

Validators on ModernTensor earn from multiple sources:

1. **Block Rewards**: Inflationary rewards for proposing blocks (Standard PoS).
2. **Transaction Fees**: Gas fees from normal transactions.
3. **AI Compute Fees**: 1% protocol fee from all AI inference tasks processed by the network.

*Note: AI Miners earn 99% of the inference fee, Validators ensure the ledger's integrity.*

---

## Option 1: Docker Deployment (Recommended)

### 1. Pull the Image

```bash
docker pull moderntensor/luxtensor-node:latest
```

### 2. Create Configuration

```bash
mkdir -p ~/.luxtensor/config
cat > ~/.luxtensor/config/node.toml << 'EOF'
[node]
name = "my-validator"
role = "validator"

[network]
listen_addr = "/ip4/0.0.0.0/tcp/30333"
external_addr = "/ip4/YOUR_PUBLIC_IP/tcp/30333"

[rpc]
enabled = true
addr = "0.0.0.0:8545"
ws_addr = "0.0.0.0:8546"

[consensus]
validator_key_path = "/data/keys/validator.key"

[storage]
data_dir = "/data/db"
EOF
```

### 3. Generate Validator Keys

```bash
docker run --rm -v ~/.luxtensor:/data moderntensor/luxtensor-node:latest \
    luxtensor-cli keys generate --output /data/keys/validator.key
```

**⚠️ CRITICAL: Back up `/data/keys/validator.key` securely!**

### 4. Run the Node

```bash
docker run -d \
    --name luxtensor-validator \
    --restart unless-stopped \
    -v ~/.luxtensor:/data \
    -p 30333:30333 \
    -p 8545:8545 \
    -p 8546:8546 \
    moderntensor/luxtensor-node:latest \
    --config /data/config/node.toml
```

### 5. Check Status

```bash
docker logs -f luxtensor-validator
```

---

## Option 2: Build from Source

### 1. Clone Repository

```bash
git clone https://github.com/moderntensor/luxtensor.git
cd luxtensor
```

### 2. Build

```bash
cargo build --release
```

### 3. Run

```bash
./target/release/luxtensor-node --config config/validator.toml
```

---

## Staking & Registration

### 1. Fund Your Validator Account

Send at least **10,000 MDT** to your validator address for the minimum stake.

### 2. Register as Validator

Using the SDK:

```python
from moderntensor.sdk import LuxtensorClient

client = LuxtensorClient("http://localhost:8545")

# Register validator
tx = client.staking.register_validator(
    stake_amount=10000 * 10**18,  # 10,000 MDT in wei
    private_key="YOUR_PRIVATE_KEY"
)
print(f"Registered! TX: {tx}")
```

Or via RPC:

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "staking_registerValidator",
    "params": ["10000000000000000000000"],
    "id": 1
  }'
```

### 3. Verify Registration

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "staking_getActiveValidators",
    "params": [],
    "id": 1
  }'
```

---

## Monitoring

### Enable Prometheus Metrics

Add to your `node.toml`:

```toml
[metrics]
enabled = true
addr = "0.0.0.0:9090"
```

### Grafana Dashboard

Import the dashboard from `monitoring/grafana-dashboard.json`.

### Key Metrics to Watch

| Metric | Healthy Value |
|--------|---------------|
| `luxtensor_block_height` | Increasing |
| `luxtensor_peer_count` | ≥ 5 |
| `luxtensor_missed_blocks` | < 10 |
| `luxtensor_consensus_rounds` | Stable |

---

## Troubleshooting

### Node Not Syncing

```bash
# Check peer connections
curl -X POST http://localhost:8545 \
  -d '{"jsonrpc":"2.0","method":"system_peers","params":[],"id":1}'

# Force resync
rm -rf ~/.luxtensor/db
docker restart luxtensor-validator
```

### Validator Not Producing Blocks

1. Verify stake: `staking_getValidator`
2. Check epoch: Validators activate after 1 epoch delay
3. Review logs: `docker logs luxtensor-validator | grep -i error`

### High Missed Blocks

1. Check network connectivity
2. Increase `--max-peers` if isolated
3. Verify clock sync: `timedatectl status`

---

## Security Best Practices

1. **Firewall**: Only expose ports 30333 (P2P), 8545/8546 (RPC if needed)
2. **Key Management**: Use HSM or KMS for validator keys in production
3. **Updates**: Subscribe to security announcements
4. **Monitoring**: Set alerts for missed blocks and peer drops

---

## Validator Exit

To gracefully exit:

```python
client.staking.exit_validator(private_key="YOUR_PRIVATE_KEY")
```

Note: Exit takes effect after the `exit_delay_epochs` (default: 2 epochs).
