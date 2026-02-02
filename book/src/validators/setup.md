# Validator Setup

## Prerequisites

- **OS**: Ubuntu 22.04 LTS
- **Ports**: 30333 (P2P), 8545 (RPC - Optional)
- **Token Balance**: >10,000 MDT + Gas

## Option 1: Docker (Recommended)

```bash
# 1. Pull Image
docker pull moderntensor/luxtensor-node:latest

# 2. Init Config
mkdir -p ~/.luxtensor/config
# (See config/validator.toml template)

# 3. Generate Keys
docker run --rm -v ~/.luxtensor:/data moderntensor/luxtensor-node \
    luxtensor-cli keys generate --output /data/keys/validator.key

# 4. Run
docker run -d --name validator \
    -v ~/.luxtensor:/data \
    -p 30333:30333 \
    moderntensor/luxtensor-node \
    --config /data/config/node.toml
```

## Option 2: Build from Source

```bash
git clone https://github.com/moderntensor/luxtensor
cargo build --release
./target/release/luxtensor-node --config config/validator.toml
```

## Staking Registration

Once your node is syncing, you must register blindly on-chain.

```bash
# Register with 10,000 MDT
curl -X POST http://localhost:8545 \
  -d '{"jsonrpc":"2.0","method":"staking_registerValidator","params":["10000000000000000000000"],"id":1}'
```

*Wait 1 epoch (approx 24h) for activation.*
