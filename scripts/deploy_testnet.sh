#!/bin/bash
# LuxTensor Testnet Deployment Script
# This script helps deploy a local or cloud testnet

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
TESTNET_NAME="luxtensor-testnet"
NUM_VALIDATORS=3
NUM_FULL_NODES=2
CHAIN_ID=9999

# Directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LUXTENSOR_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DATA_DIR="${DATA_DIR:-$LUXTENSOR_DIR/testnet-data}"
KEYS_DIR="$DATA_DIR/keys"
CONFIGS_DIR="$DATA_DIR/configs"

# Functions
print_header() {
    echo -e "${GREEN}================================================${NC}"
    echo -e "${GREEN}$1${NC}"
    echo -e "${GREEN}================================================${NC}"
}

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_prerequisites() {
    print_header "Checking Prerequisites"
    
    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        print_error "Rust is not installed. Please install Rust first."
        exit 1
    fi
    print_info "✓ Rust $(rustc --version | cut -d' ' -f2)"
    
    # Check if Docker is installed (optional)
    if command -v docker &> /dev/null; then
        print_info "✓ Docker $(docker --version | cut -d' ' -f3 | tr -d ',')"
    else
        print_warning "Docker not found. Skipping Docker deployment."
    fi
    
    # Check if jq is installed
    if ! command -v jq &> /dev/null; then
        print_warning "jq not found. Installing..."
        if [[ "$OSTYPE" == "linux-gnu"* ]]; then
            sudo apt-get install -y jq
        elif [[ "$OSTYPE" == "darwin"* ]]; then
            brew install jq
        fi
    fi
    print_info "✓ jq installed"
    
    echo ""
}

build_node() {
    print_header "Building LuxTensor Node"
    
    cd "$LUXTENSOR_DIR/luxtensor"
    
    print_info "Building release binary..."
    cargo build --release -p luxtensor-node
    
    if [ -f "target/release/luxtensor-node" ]; then
        print_info "✓ Binary built successfully: $(du -h target/release/luxtensor-node | cut -f1)"
    else
        print_error "Failed to build binary"
        exit 1
    fi
    
    echo ""
}

setup_directories() {
    print_header "Setting Up Directories"
    
    print_info "Creating directory structure..."
    mkdir -p "$DATA_DIR"
    mkdir -p "$KEYS_DIR"
    mkdir -p "$CONFIGS_DIR"
    
    for i in $(seq 1 $NUM_VALIDATORS); do
        mkdir -p "$DATA_DIR/validator$i/db"
    done
    
    for i in $(seq 1 $NUM_FULL_NODES); do
        mkdir -p "$DATA_DIR/fullnode$i/db"
    done
    
    print_info "✓ Directories created at $DATA_DIR"
    echo ""
}

generate_keys() {
    print_header "Generating Validator Keys"
    
    NODE_BIN="$LUXTENSOR_DIR/luxtensor/target/release/luxtensor-node"
    
    for i in $(seq 1 $NUM_VALIDATORS); do
        KEY_FILE="$KEYS_DIR/validator$i.key"
        
        if [ -f "$KEY_FILE" ]; then
            print_warning "Key already exists: $KEY_FILE (skipping)"
        else
            print_info "Generating key for validator $i..."
            # Note: This command needs to be implemented in the actual node
            # For now, we'll create a placeholder
            echo "{\"private_key\":\"0x$(openssl rand -hex 32)\"}" > "$KEY_FILE"
            print_info "✓ Generated key: validator$i.key"
        fi
    done
    
    echo ""
}

generate_genesis() {
    print_header "Generating Genesis Configuration"
    
    GENESIS_FILE="$DATA_DIR/genesis.json"
    
    # Copy template
    cp "$LUXTENSOR_DIR/luxtensor/genesis.testnet.json" "$GENESIS_FILE"
    
    print_info "Genesis configuration:"
    print_info "  Chain ID: $CHAIN_ID"
    print_info "  Network: $TESTNET_NAME"
    print_info "  Validators: $NUM_VALIDATORS"
    
    # Update validator addresses in genesis
    # This would need actual implementation based on generated keys
    
    print_info "✓ Genesis file: $GENESIS_FILE"
    echo ""
}

generate_configs() {
    print_header "Generating Node Configurations"
    
    BASE_P2P_PORT=30303
    BASE_RPC_PORT=8545
    BASE_METRICS_PORT=9090
    
    # Generate validator configs
    for i in $(seq 1 $NUM_VALIDATORS); do
        CONFIG_FILE="$CONFIGS_DIR/validator$i.toml"
        P2P_PORT=$((BASE_P2P_PORT + i - 1))
        RPC_PORT=$((BASE_RPC_PORT + i - 1))
        METRICS_PORT=$((BASE_METRICS_PORT + i - 1))
        
        cat > "$CONFIG_FILE" << EOF
[node]
name = "validator-$i"
chain_id = $CHAIN_ID
data_dir = "$DATA_DIR/validator$i"
is_validator = true
validator_key_path = "$KEYS_DIR/validator$i.key"

[consensus]
block_time = 3
epoch_length = 100
min_stake = 10000000000000000000
max_validators = 21

[network]
listen_addr = "0.0.0.0"
listen_port = $P2P_PORT
max_peers = 50
enable_mdns = true

[storage]
db_path = "$DATA_DIR/validator$i/db"
enable_compression = true
cache_size = 256

[rpc]
enabled = true
listen_addr = "0.0.0.0"
listen_port = $RPC_PORT
threads = 4
cors_origins = ["*"]

[logging]
level = "info"
format = "human"

[metrics]
enabled = true
listen_addr = "0.0.0.0"
listen_port = $METRICS_PORT
EOF
        
        print_info "✓ Created config: validator$i.toml (P2P:$P2P_PORT, RPC:$RPC_PORT)"
    done
    
    # Generate full node configs
    for i in $(seq 1 $NUM_FULL_NODES); do
        CONFIG_FILE="$CONFIGS_DIR/fullnode$i.toml"
        OFFSET=$((NUM_VALIDATORS + i - 1))
        P2P_PORT=$((BASE_P2P_PORT + OFFSET))
        RPC_PORT=$((BASE_RPC_PORT + OFFSET))
        METRICS_PORT=$((BASE_METRICS_PORT + OFFSET))
        
        cat > "$CONFIG_FILE" << EOF
[node]
name = "fullnode-$i"
chain_id = $CHAIN_ID
data_dir = "$DATA_DIR/fullnode$i"
is_validator = false

[consensus]
block_time = 3
epoch_length = 100
min_stake = 10000000000000000000
max_validators = 21

[network]
listen_addr = "0.0.0.0"
listen_port = $P2P_PORT
max_peers = 50
enable_mdns = true

[storage]
db_path = "$DATA_DIR/fullnode$i/db"
enable_compression = true
cache_size = 256

[rpc]
enabled = true
listen_addr = "0.0.0.0"
listen_port = $RPC_PORT
threads = 4
cors_origins = ["*"]

[logging]
level = "info"
format = "human"

[metrics]
enabled = true
listen_addr = "0.0.0.0"
listen_port = $METRICS_PORT
EOF
        
        print_info "✓ Created config: fullnode$i.toml (P2P:$P2P_PORT, RPC:$RPC_PORT)"
    done
    
    echo ""
}

start_nodes() {
    print_header "Starting Testnet Nodes"
    
    NODE_BIN="$LUXTENSOR_DIR/luxtensor/target/release/luxtensor-node"
    
    # Start validators
    for i in $(seq 1 $NUM_VALIDATORS); do
        CONFIG_FILE="$CONFIGS_DIR/validator$i.toml"
        LOG_FILE="$DATA_DIR/validator$i.log"
        
        print_info "Starting validator $i..."
        nohup "$NODE_BIN" start --config "$CONFIG_FILE" > "$LOG_FILE" 2>&1 &
        echo $! > "$DATA_DIR/validator$i.pid"
        
        print_info "✓ Validator $i started (PID: $(cat $DATA_DIR/validator$i.pid))"
        sleep 2
    done
    
    # Start full nodes
    for i in $(seq 1 $NUM_FULL_NODES); do
        CONFIG_FILE="$CONFIGS_DIR/fullnode$i.toml"
        LOG_FILE="$DATA_DIR/fullnode$i.log"
        
        print_info "Starting full node $i..."
        nohup "$NODE_BIN" start --config "$CONFIG_FILE" > "$LOG_FILE" 2>&1 &
        echo $! > "$DATA_DIR/fullnode$i.pid"
        
        print_info "✓ Full node $i started (PID: $(cat $DATA_DIR/fullnode$i.pid))"
        sleep 2
    done
    
    echo ""
}

stop_nodes() {
    print_header "Stopping Testnet Nodes"
    
    # Stop validators
    for i in $(seq 1 $NUM_VALIDATORS); do
        PID_FILE="$DATA_DIR/validator$i.pid"
        if [ -f "$PID_FILE" ]; then
            PID=$(cat "$PID_FILE")
            if ps -p $PID > /dev/null; then
                print_info "Stopping validator $i (PID: $PID)..."
                kill $PID
                rm "$PID_FILE"
            fi
        fi
    done
    
    # Stop full nodes
    for i in $(seq 1 $NUM_FULL_NODES); do
        PID_FILE="$DATA_DIR/fullnode$i.pid"
        if [ -f "$PID_FILE" ]; then
            PID=$(cat "$PID_FILE")
            if ps -p $PID > /dev/null; then
                print_info "Stopping full node $i (PID: $PID)..."
                kill $PID
                rm "$PID_FILE"
            fi
        fi
    done
    
    print_info "✓ All nodes stopped"
    echo ""
}

check_status() {
    print_header "Testnet Status"
    
    # Check validators
    print_info "Validators:"
    for i in $(seq 1 $NUM_VALIDATORS); do
        PID_FILE="$DATA_DIR/validator$i.pid"
        if [ -f "$PID_FILE" ]; then
            PID=$(cat "$PID_FILE")
            if ps -p $PID > /dev/null; then
                RPC_PORT=$((8545 + i - 1))
                BLOCK_HEIGHT=$(curl -s http://localhost:$RPC_PORT -X POST -H "Content-Type: application/json" \
                    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | jq -r '.result' 2>/dev/null || echo "N/A")
                echo "  ✓ Validator $i - Running (PID: $PID, RPC: $RPC_PORT, Block: $BLOCK_HEIGHT)"
            else
                echo "  ✗ Validator $i - Stopped"
            fi
        else
            echo "  ✗ Validator $i - Not started"
        fi
    done
    
    # Check full nodes
    print_info "Full Nodes:"
    for i in $(seq 1 $NUM_FULL_NODES); do
        PID_FILE="$DATA_DIR/fullnode$i.pid"
        if [ -f "$PID_FILE" ]; then
            PID=$(cat "$PID_FILE")
            if ps -p $PID > /dev/null; then
                OFFSET=$((NUM_VALIDATORS + i - 1))
                RPC_PORT=$((8545 + OFFSET))
                BLOCK_HEIGHT=$(curl -s http://localhost:$RPC_PORT -X POST -H "Content-Type: application/json" \
                    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | jq -r '.result' 2>/dev/null || echo "N/A")
                echo "  ✓ Full Node $i - Running (PID: $PID, RPC: $RPC_PORT, Block: $BLOCK_HEIGHT)"
            else
                echo "  ✗ Full Node $i - Stopped"
            fi
        else
            echo "  ✗ Full Node $i - Not started"
        fi
    done
    
    echo ""
}

show_logs() {
    NODE_TYPE=$1
    NODE_NUM=$2
    
    if [ -z "$NODE_TYPE" ] || [ -z "$NODE_NUM" ]; then
        print_error "Usage: $0 logs <validator|fullnode> <number>"
        exit 1
    fi
    
    LOG_FILE="$DATA_DIR/${NODE_TYPE}${NODE_NUM}.log"
    
    if [ -f "$LOG_FILE" ]; then
        tail -f "$LOG_FILE"
    else
        print_error "Log file not found: $LOG_FILE"
        exit 1
    fi
}

clean() {
    print_header "Cleaning Testnet Data"
    
    print_warning "This will delete all testnet data!"
    read -p "Are you sure? (yes/no): " -r
    
    if [ "$REPLY" = "yes" ]; then
        stop_nodes
        rm -rf "$DATA_DIR"
        print_info "✓ Testnet data cleaned"
    else
        print_info "Cancelled"
    fi
    
    echo ""
}

# Main script
case "${1:-}" in
    init)
        check_prerequisites
        build_node
        setup_directories
        generate_keys
        generate_genesis
        generate_configs
        print_header "Testnet Initialized Successfully!"
        print_info "Next step: $0 start"
        ;;
    start)
        start_nodes
        sleep 5
        check_status
        print_info "Testnet is running!"
        print_info "To check status: $0 status"
        print_info "To stop: $0 stop"
        ;;
    stop)
        stop_nodes
        ;;
    status)
        check_status
        ;;
    logs)
        show_logs "$2" "$3"
        ;;
    restart)
        stop_nodes
        sleep 2
        start_nodes
        sleep 5
        check_status
        ;;
    clean)
        clean
        ;;
    *)
        echo "LuxTensor Testnet Deployment Script"
        echo ""
        echo "Usage: $0 <command>"
        echo ""
        echo "Commands:"
        echo "  init      - Initialize testnet (build, generate keys, configs)"
        echo "  start     - Start all nodes"
        echo "  stop      - Stop all nodes"
        echo "  restart   - Restart all nodes"
        echo "  status    - Check node status"
        echo "  logs      - Show logs (usage: logs <validator|fullnode> <number>)"
        echo "  clean     - Clean all testnet data"
        echo ""
        exit 1
        ;;
esac
