# 🚀 Luxtensor Production Deployment Guide

## Prerequisites

- Rust 1.75+
- 4GB RAM, 50GB SSD
- Open ports: 30303 (P2P), 8545 (RPC)

---

## Quick Setup

### 1. Build

```bash
git clone https://github.com/sonson0910/moderntensor.git
cd moderntensor/luxtensor
cargo build --release
```

### 2. Create Config

```bash
mkdir -p /opt/luxtensor
cp target/release/luxtensor-node /opt/luxtensor/
```

Create `/opt/luxtensor/config.toml`:

```toml
[node]
name = "my-node"
chain_id = 1
data_dir = "./data"
is_validator = true
validator_id = "validator-N"  # Change N

[consensus]
block_time = 3
epoch_length = 100
min_stake = "1000000000000000000"
max_validators = 100
gas_limit = 30000000
validators = ["validator-1", "validator-2", "validator-3"]

[network]
listen_addr = "0.0.0.0"
listen_port = 30303
bootstrap_nodes = [
    "/ip4/SEED_IP/tcp/30303/p2p/PEER_ID"
]
max_peers = 50
enable_mdns = false

[storage]
db_path = "./data/db"
enable_compression = true
max_open_files = 1000
cache_size = 512

[rpc]
enabled = true
listen_addr = "0.0.0.0"
listen_port = 8545
threads = 4
cors_origins = ["*"]

[logging]
level = "info"
log_to_file = true
log_file = "./node.log"
json_format = false
```

### 3. Run

```bash
cd /opt/luxtensor
./luxtensor-node --config config.toml
```

---

## Systemd Service

```ini
# /etc/systemd/system/luxtensor.service
[Unit]
Description=Luxtensor Node
After=network.target

[Service]
Type=simple
User=luxtensor
WorkingDirectory=/opt/luxtensor
ExecStart=/opt/luxtensor/luxtensor-node --config config.toml
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable luxtensor
sudo systemctl start luxtensor
```

---

## Firewall

```bash
sudo ufw allow 30303/tcp  # P2P
sudo ufw allow 8545/tcp   # RPC (restrict in production)
```

---

## Health Check

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

---

## Security Recommendations

1. **RPC Access**: Bind to 127.0.0.1 in production, use nginx reverse proxy
2. **Validator Key**: Store securely, use hardware security module if possible
3. **Firewall**: Restrict RPC access, allow P2P from specific IPs
4. **Monitoring**: Use Prometheus + Grafana for metrics
5. **Backups**: Regular backups of `data/` directory
