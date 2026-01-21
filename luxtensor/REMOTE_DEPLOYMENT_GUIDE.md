# 🌐 Luxtensor Multi-Node Remote Deployment Guide

## Tổng quan

Để triển khai Luxtensor trên nhiều máy từ xa, bạn cần:

1. **Build binary** trên mỗi máy
2. **Tạo validator keys** cho mỗi node
3. **Config bootstrap nodes** để các node tìm thấy nhau
4. **Mở ports** cho P2P và RPC

---

## Step 1: Chuẩn bị trên mỗi Server

### 1.1 Clone và Build

```bash
# SSH vào server
ssh user@your-server-ip

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone repo
git clone https://github.com/sonson0910/moderntensor.git
cd moderntensor/luxtensor

# Build release
cargo build --release

# Copy binary
sudo mkdir -p /opt/luxtensor
sudo cp target/release/luxtensor-node /opt/luxtensor/
```

### 1.2 Tạo Validator Key

```bash
# Tạo random 32-byte key
openssl rand -hex 32 | xxd -r -p > /opt/luxtensor/validator.key
chmod 600 /opt/luxtensor/validator.key

# Xem địa chỉ (lấy 40 ký tự cuối của hash)
cat /opt/luxtensor/validator.key | sha256sum | cut -c25-64
```

---

## Step 2: Configure

### 2.1 Config cho SEED NODE (Server 1)

```bash
cat > /opt/luxtensor/config.toml << 'EOF'
[node]
name = "seed-node"
chain_id = 1
data_dir = "./data"
is_validator = true
validator_key_path = "./validator.key"
validator_id = "validator-1"

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
bootstrap_nodes = []    # Seed node không cần bootstrap
max_peers = 50
enable_mdns = false     # QUAN TRỌNG: disable mDNS cho remote

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
EOF
```

### 2.2 Lấy Peer ID của Seed Node

```bash
# Start seed node
cd /opt/luxtensor
./luxtensor-node --config config.toml &

# Xem log để lấy Peer ID
grep "Local peer id" node.log
# Output: Local peer id: 12D3KooW...xyz
```

### 2.3 Config cho các NODE KHÁC (Server 2, 3, ...)

```bash
cat > /opt/luxtensor/config.toml << 'EOF'
[node]
name = "node-2"
chain_id = 1
data_dir = "./data"
is_validator = true
validator_key_path = "./validator.key"
validator_id = "validator-2"

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
# THAY BẰNG IP VÀ PEER_ID CỦA SEED NODE
bootstrap_nodes = [
    "/ip4/SEED_SERVER_IP/tcp/30303/p2p/SEED_PEER_ID"
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
EOF
```

**VÍ DỤ cụ thể** (nếu seed node IP = 203.0.113.10, peer ID = 12D3KooWHxU...):

```toml
bootstrap_nodes = [
    "/ip4/203.0.113.10/tcp/30303/p2p/12D3KooWHxUxbJpYmFt..."
]
```

---

## Step 3: Firewall

### Trên mỗi server

```bash
# Ubuntu/Debian
sudo ufw allow 30303/tcp   # P2P
sudo ufw allow 8545/tcp    # RPC (nên restrict IP)
sudo ufw enable

# Restrict RPC chỉ cho IPs cụ thể
sudo ufw allow from 192.168.1.0/24 to any port 8545
```

### Nếu dùng Cloud (AWS/GCP/Azure)

- Mở inbound rule port **30303 TCP** từ anywhere
- Port **8545** chỉ mở cho IPs cần thiết

---

## Step 4: Systemd Service

```bash
sudo cat > /etc/systemd/system/luxtensor.service << 'EOF'
[Unit]
Description=Luxtensor Blockchain Node
After=network.target

[Service]
Type=simple
User=luxtensor
WorkingDirectory=/opt/luxtensor
ExecStart=/opt/luxtensor/luxtensor-node --config config.toml
Restart=always
RestartSec=10
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

# Create user
sudo useradd -r -s /bin/false luxtensor
sudo chown -R luxtensor:luxtensor /opt/luxtensor

# Enable và start
sudo systemctl daemon-reload
sudo systemctl enable luxtensor
sudo systemctl start luxtensor

# Check status
sudo systemctl status luxtensor
sudo journalctl -u luxtensor -f
```

---

## Step 5: Verify Connections

### Trên mỗi node

```bash
# Check peers
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}'
# Kết quả: {"result":"0x2"} = 2 peers đã kết nối

# Check block sync
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
# Tất cả nodes phải có cùng block height
```

---

## Checklist Triển khai

| Step | Server 1 (Seed) | Server 2 | Server 3 |
|------|-----------------|----------|----------|
| Build binary | ✅ | ✅ | ✅ |
| Tạo validator key | ✅ | ✅ | ✅ |
| Config bootstrap | (empty) | seed IP | seed IP |
| Mở port 30303 | ✅ | ✅ | ✅ |
| Start node | ✅ FIRST | After seed | After seed |
| Verify peers | Check | Check | Check |

---

## Troubleshooting

### Nodes không connect được

1. Check firewall: `sudo ufw status`
2. Check port listening: `netstat -tlnp | grep 30303`
3. Check bootstrap format correct
4. Check logs: `sudo journalctl -u luxtensor -n 100`

### Block không sync

1. Verify chain_id giống nhau
2. Check validators list giống nhau
3. Check genesis config
