#!/bin/bash
# ============================================================
# Luxtensor Testnet Deployment Script
# ============================================================
#
# Deploy a local testnet with multiple nodes for testing
#
# Usage:
#   ./deploy_testnet.sh --nodes 10 --validators 7
#   ./deploy_testnet.sh --start
#   ./deploy_testnet.sh --stop
#   ./deploy_testnet.sh --clean
# ============================================================

set -e

# Default configuration
NODE_COUNT=10
VALIDATOR_COUNT=7
BASE_RPC_PORT=8545
BASE_P2P_PORT=30303
TESTNET_DIR="./testnet_data"
CHAIN_ID=9999

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --nodes)
            NODE_COUNT="$2"
            shift 2
            ;;
        --validators)
            VALIDATOR_COUNT="$2"
            shift 2
            ;;
        --start)
            START_ONLY=true
            shift
            ;;
        --stop)
            STOP_ONLY=true
            shift
            ;;
        --clean)
            CLEAN_ONLY=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --nodes N        Number of nodes (default: 10)"
            echo "  --validators N   Number of validators (default: 7)"
            echo "  --start          Start existing testnet"
            echo "  --stop           Stop running testnet"
            echo "  --clean          Clean testnet data"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Stop all nodes
stop_testnet() {
    log_info "Stopping testnet..."

    for pid_file in "$TESTNET_DIR"/node_*/node.pid; do
        if [[ -f "$pid_file" ]]; then
            pid=$(cat "$pid_file")
            if ps -p "$pid" > /dev/null 2>&1; then
                log_info "Stopping node (PID: $pid)"
                kill "$pid" 2>/dev/null || true
            fi
            rm -f "$pid_file"
        fi
    done

    log_info "All nodes stopped"
}

# Clean testnet data
clean_testnet() {
    log_info "Cleaning testnet data..."
    rm -rf "$TESTNET_DIR"
    log_info "Testnet data cleaned"
}

# Create node configuration
create_node_config() {
    local node_id=$1
    local is_validator=$2
    local node_dir="$TESTNET_DIR/node_$node_id"
    local rpc_port=$((BASE_RPC_PORT + node_id - 1))
    local p2p_port=$((BASE_P2P_PORT + node_id - 1))

    mkdir -p "$node_dir/db"

    # Generate validator list
    local validator_list=""
    for i in $(seq 1 $VALIDATOR_COUNT); do
        if [[ -n "$validator_list" ]]; then
            validator_list="$validator_list, "
        fi
        validator_list="$validator_list\"validator-$i\""
    done

    # Bootstrap nodes (point to first node if not the first)
    local bootstrap=""
    if [[ $node_id -gt 1 ]]; then
        bootstrap="[\"/ip4/127.0.0.1/tcp/$BASE_P2P_PORT\"]"
    else
        bootstrap="[]"
    fi

    cat > "$node_dir/config.toml" << EOF
[node]
name = "testnet-node-$node_id"
chain_id = $CHAIN_ID
data_dir = "$node_dir"
is_validator = $is_validator
validator_id = "validator-$node_id"
dao_address = "0xDAO0000000000000000000000000000000000001"

[consensus]
block_time = 3
epoch_length = 10
min_stake = "1000000000000000000"
max_validators = 100
validators = [$validator_list]

[network]
listen_addr = "0.0.0.0"
listen_port = $p2p_port
bootstrap_nodes = $bootstrap
max_peers = 25
enable_mdns = true

[storage]
db_path = "$node_dir/db"

[rpc]
enabled = true
listen_addr = "127.0.0.1"
listen_port = $rpc_port
threads = 4

[logging]
level = "info"
log_to_file = true
log_file = "$node_dir/node.log"
EOF
}

# Setup testnet
setup_testnet() {
    log_info "Setting up testnet with $NODE_COUNT nodes ($VALIDATOR_COUNT validators)"

    mkdir -p "$TESTNET_DIR"

    for i in $(seq 1 $NODE_COUNT); do
        if [[ $i -le $VALIDATOR_COUNT ]]; then
            is_validator="true"
        else
            is_validator="false"
        fi

        log_info "Creating node $i (validator: $is_validator)"
        create_node_config "$i" "$is_validator"
    done

    log_info "Testnet setup complete"
}

# Start a single node
start_node() {
    local node_id=$1
    local node_dir="$TESTNET_DIR/node_$node_id"
    local config_file="$node_dir/config.toml"
    local log_file="$node_dir/stdout.log"
    local pid_file="$node_dir/node.pid"

    if [[ ! -f "$config_file" ]]; then
        log_error "Config not found for node $node_id"
        return 1
    fi

    log_info "Starting node $node_id..."

    # Start node in background
    cargo run --release -p luxtensor-node -- --config "$config_file" > "$log_file" 2>&1 &
    local pid=$!

    echo "$pid" > "$pid_file"

    # Wait and check if started
    sleep 2

    if ps -p "$pid" > /dev/null 2>&1; then
        log_info "Node $node_id started (PID: $pid)"
        return 0
    else
        log_error "Node $node_id failed to start"
        return 1
    fi
}

# Start all nodes
start_testnet() {
    log_info "Starting testnet..."

    local started=0
    for i in $(seq 1 $NODE_COUNT); do
        if start_node "$i"; then
            ((started++))
        fi
        sleep 1  # Stagger starts
    done

    log_info "Started $started/$NODE_COUNT nodes"

    # Wait for network to stabilize
    log_info "Waiting for network to stabilize..."
    sleep 10

    # Check status
    check_testnet_status
}

# Check testnet status
check_testnet_status() {
    log_info "Checking testnet status..."

    echo ""
    printf "%-10s %-12s %-12s %-10s %-15s\n" "Node" "RPC Port" "P2P Port" "Status" "Block Height"
    printf "%-10s %-12s %-12s %-10s %-15s\n" "----" "--------" "--------" "------" "------------"

    for i in $(seq 1 $NODE_COUNT); do
        local rpc_port=$((BASE_RPC_PORT + i - 1))
        local p2p_port=$((BASE_P2P_PORT + i - 1))
        local pid_file="$TESTNET_DIR/node_$i/node.pid"

        local status="DOWN"
        local height="N/A"

        if [[ -f "$pid_file" ]]; then
            local pid=$(cat "$pid_file")
            if ps -p "$pid" > /dev/null 2>&1; then
                status="UP"

                # Get block height
                height=$(curl -s -X POST "http://127.0.0.1:$rpc_port" \
                    -H "Content-Type: application/json" \
                    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
                    2>/dev/null | jq -r '.result // "N/A"' 2>/dev/null || echo "N/A")

                if [[ "$height" != "N/A" && "$height" != "null" ]]; then
                    height=$((16#${height#0x}))
                fi
            fi
        fi

        printf "%-10s %-12s %-12s %-10s %-15s\n" "Node $i" "$rpc_port" "$p2p_port" "$status" "$height"
    done
    echo ""
}

# Print RPC endpoints
print_endpoints() {
    log_info "RPC Endpoints:"
    for i in $(seq 1 $NODE_COUNT); do
        local rpc_port=$((BASE_RPC_PORT + i - 1))
        echo "  Node $i: http://127.0.0.1:$rpc_port"
    done
}

# Main
main() {
    echo "============================================================"
    echo "        LUXTENSOR TESTNET DEPLOYMENT"
    echo "============================================================"

    if [[ "$STOP_ONLY" == "true" ]]; then
        stop_testnet
        exit 0
    fi

    if [[ "$CLEAN_ONLY" == "true" ]]; then
        stop_testnet
        clean_testnet
        exit 0
    fi

    if [[ "$START_ONLY" == "true" ]]; then
        start_testnet
        print_endpoints
        exit 0
    fi

    # Full deployment
    stop_testnet 2>/dev/null || true
    clean_testnet
    setup_testnet
    start_testnet
    print_endpoints

    log_info "Testnet deployment complete!"
    log_info "Run './deploy_testnet.sh --stop' to stop the testnet"
}

main
