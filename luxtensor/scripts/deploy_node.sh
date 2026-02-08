#!/bin/bash
# Quick deployment script for remote servers
# Run this on each server after cloning the repo

set -e

echo "=== Luxtensor Node Deployment ==="

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (sudo ./deploy_node.sh)"
    exit 1
fi

# Arguments
NODE_NAME=${1:-"node-1"}
VALIDATOR_ID=${2:-"validator-1"}
SEED_NODE=${3:-""}  # Format: /ip4/IP/tcp/30303/p2p/PEER_ID

echo "Node Name: $NODE_NAME"
echo "Validator ID: $VALIDATOR_ID"
echo "Seed Node: $SEED_NODE"

# Install dependencies
echo "==> Installing dependencies..."
apt update
apt install -y build-essential pkg-config libssl-dev

# Install Rust if not present
if ! command -v cargo &> /dev/null; then
    echo "==> Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Build
echo "==> Building Luxtensor..."
cargo build --release

# Setup directory
echo "==> Setting up /opt/luxtensor..."
mkdir -p /opt/luxtensor/data
cp target/release/luxtensor-node /opt/luxtensor/

# Generate validator key
echo "==> Generating validator key..."
openssl rand 32 > /opt/luxtensor/validator.key
chmod 600 /opt/luxtensor/validator.key

# Create config
echo "==> Creating config..."
if [ -z "$SEED_NODE" ]; then
    BOOTSTRAP="bootstrap_nodes = []"
else
    BOOTSTRAP="bootstrap_nodes = [\"$SEED_NODE\"]"
fi

cat > /opt/luxtensor/config.toml << EOF
[node]
name = "$NODE_NAME"
chain_id = 8898  # LuxTensor devnet (use 8899 for mainnet, 9999 for testnet)
data_dir = "./data"
is_validator = true
validator_key_path = "./validator.key"
validator_id = "$VALIDATOR_ID"

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
$BOOTSTRAP
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

# Create systemd service
echo "==> Creating systemd service..."
cat > /etc/systemd/system/luxtensor.service << EOF
[Unit]
Description=Luxtensor Blockchain Node
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/luxtensor
ExecStart=/opt/luxtensor/luxtensor-node --config config.toml
Restart=always
RestartSec=10
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

# Open firewall
echo "==> Configuring firewall..."
ufw allow 30303/tcp
ufw allow 8545/tcp
ufw --force enable

# Start service
echo "==> Starting node..."
systemctl daemon-reload
systemctl enable luxtensor
systemctl start luxtensor

echo ""
echo "=== Deployment Complete ==="
echo "Check status: systemctl status luxtensor"
echo "View logs: journalctl -u luxtensor -f"
echo ""
echo "To get Peer ID (for bootstrap):"
echo "  grep 'Local peer id' /opt/luxtensor/node.log"
