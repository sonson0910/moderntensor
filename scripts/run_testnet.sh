#!/usr/bin/env bash
# =============================================================================
# LuxTensor Testnet Boot Script
# =============================================================================
# Starts a local 3-validator testnet for development & integration testing.
#
# Usage:
#   ./scripts/run_testnet.sh           # Build + start testnet
#   ./scripts/run_testnet.sh --no-build # Skip build, reuse existing binary
#
# Validators:
#   Node 0  →  JSON-RPC on http://127.0.0.1:8545  (data: /tmp/luxtensor-testnet/node0)
#   Node 1  →  JSON-RPC on http://127.0.0.1:8546  (data: /tmp/luxtensor-testnet/node1)
#   Node 2  →  JSON-RPC on http://127.0.0.1:8547  (data: /tmp/luxtensor-testnet/node2)
#
# Chain ID: 9999 (LuxTensor Testnet)
#
# To stop the testnet:
#   ./scripts/run_testnet.sh --stop
# =============================================================================
set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LUXTENSOR_DIR="$PROJECT_ROOT/luxtensor"
BINARY="$LUXTENSOR_DIR/target/release/luxtensor-node"
DATA_ROOT="/tmp/luxtensor-testnet"
CHAIN_ID=9999
NUM_VALIDATORS=3
BASE_RPC_PORT=8545
BASE_P2P_PORT=30303
LOG_LEVEL="${LOG_LEVEL:-info}"
PIDS_FILE="$DATA_ROOT/pids"

# ── Color helpers ──────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# ── Stop helper ────────────────────────────────────────────────────────────────
stop_testnet() {
    info "Stopping testnet nodes..."
    if [[ -f "$PIDS_FILE" ]]; then
        while IFS= read -r pid; do
            if kill -0 "$pid" 2>/dev/null; then
                kill "$pid" && info "Stopped node (PID $pid)"
            fi
        done < "$PIDS_FILE"
        rm -f "$PIDS_FILE"
    else
        warn "No PID file found at $PIDS_FILE — nothing to stop."
    fi
    exit 0
}

# ── Parse arguments ────────────────────────────────────────────────────────────
SKIP_BUILD=false
for arg in "$@"; do
    case "$arg" in
        --no-build)  SKIP_BUILD=true ;;
        --stop)      stop_testnet ;;
        --help|-h)
            head -n 19 "$0" | tail -n +2 | sed 's/^# //' | sed 's/^#//'
            exit 0
            ;;
        *)
            error "Unknown argument: $arg"
            exit 1
            ;;
    esac
done

# ── Step 1: Build in release mode ─────────────────────────────────────────────
if [[ "$SKIP_BUILD" == false ]]; then
    info "Building luxtensor-node in release mode..."
    cd "$LUXTENSOR_DIR"
    cargo build --release --bin luxtensor-node 2>&1 | tail -5
    info "Build complete."
else
    info "Skipping build (--no-build)."
fi

if [[ ! -x "$BINARY" ]]; then
    error "Binary not found at $BINARY — run without --no-build first."
    exit 1
fi

# ── Step 2: Prepare data directories ──────────────────────────────────────────
info "Preparing data directories under $DATA_ROOT ..."
mkdir -p "$DATA_ROOT"
for i in $(seq 0 $((NUM_VALIDATORS - 1))); do
    mkdir -p "$DATA_ROOT/node$i"
done

# ── Step 3: Generate testnet genesis config ───────────────────────────────────
GENESIS_FILE="$DATA_ROOT/genesis.json"
if [[ ! -f "$GENESIS_FILE" ]]; then
    info "Generating testnet genesis config → $GENESIS_FILE"
    cat > "$GENESIS_FILE" <<'GENESIS_EOF'
{
  "config": {
    "chainId": 9999,
    "homesteadBlock": 0,
    "eip150Block": 0,
    "eip155Block": 0,
    "eip158Block": 0,
    "byzantiumBlock": 0,
    "constantinopleBlock": 0,
    "petersburgBlock": 0,
    "istanbulBlock": 0,
    "muirGlacierBlock": 0,
    "berlinBlock": 0,
    "londonBlock": 0
  },
  "difficulty": "0x1",
  "gasLimit": "0x1c9c380",
  "extraData": "0x4c7578546e736f72205465736e657420476e65736973",
  "timestamp": "0x0",
  "alloc": {
    "0000000000000000000000000000000000000001": {
      "balance": "0x3635c9adc5dea00000"
    }
  }
}
GENESIS_EOF
    info "Genesis written."
else
    info "Reusing existing genesis at $GENESIS_FILE"
fi

# ── Step 4: Start validator nodes ─────────────────────────────────────────────
> "$PIDS_FILE"   # truncate PID file

for i in $(seq 0 $((NUM_VALIDATORS - 1))); do
    RPC_PORT=$((BASE_RPC_PORT + i))
    P2P_PORT=$((BASE_P2P_PORT + i))
    DATA_DIR="$DATA_ROOT/node$i"
    LOG_FILE="$DATA_DIR/node.log"

    info "Starting validator node $i  (RPC :$RPC_PORT, P2P :$P2P_PORT) ..."

    "$BINARY" \
        --chain-id "$CHAIN_ID" \
        --rpc-port "$RPC_PORT" \
        --p2p-port "$P2P_PORT" \
        --data-dir "$DATA_DIR" \
        --genesis "$GENESIS_FILE" \
        --validator \
        --log-level "$LOG_LEVEL" \
        > "$LOG_FILE" 2>&1 &

    NODE_PID=$!
    echo "$NODE_PID" >> "$PIDS_FILE"
    info "  → PID $NODE_PID  |  Log: $LOG_FILE"
done

# ── Step 5: Wait briefly and verify ───────────────────────────────────────────
sleep 2
RUNNING=0
while IFS= read -r pid; do
    if kill -0 "$pid" 2>/dev/null; then
        RUNNING=$((RUNNING + 1))
    else
        warn "Node PID $pid exited early — check logs."
    fi
done < "$PIDS_FILE"

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  LuxTensor Testnet  —  $RUNNING / $NUM_VALIDATORS nodes running${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "  Chain ID:  ${GREEN}$CHAIN_ID${NC}"
echo -e "  Network:   ${GREEN}LuxTensor Testnet${NC}"
echo ""
for i in $(seq 0 $((NUM_VALIDATORS - 1))); do
    RPC_PORT=$((BASE_RPC_PORT + i))
    echo -e "  Node $i RPC: ${GREEN}http://127.0.0.1:$RPC_PORT${NC}"
done
echo ""
echo -e "  ${YELLOW}Faucet usage (curl):${NC}"
echo -e "    curl -X POST http://127.0.0.1:8545 \\"
echo -e "      -H 'Content-Type: application/json' \\"
echo -e '      -d '"'"'{"jsonrpc":"2.0","method":"dev_faucet","params":["0xYOUR_ADDRESS"],"id":1}'"'"
echo ""
echo -e "  ${YELLOW}Faucet web UI:${NC}"
echo -e "    python3 scripts/faucet_server.py   # then open http://127.0.0.1:8080"
echo ""
echo -e "  Stop:  ${RED}./scripts/run_testnet.sh --stop${NC}"
echo -e "  Logs:  tail -f $DATA_ROOT/node0/node.log"
echo ""
