#!/bin/bash

# Script to check the status of all running LuxTensor nodes
# Usage: ./check-nodes.sh

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  LuxTensor Multi-Node Status Check${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}\n"

# Check if nodes are running
echo -e "${YELLOW}Checking running processes...${NC}"
if pgrep -f "luxtensor-node" > /dev/null; then
    echo -e "${GREEN}✓ LuxTensor nodes are running${NC}"
    pgrep -af "luxtensor-node"
else
    echo -e "${RED}✗ No LuxTensor nodes are running${NC}"
fi
echo ""

# Check tmux session
echo -e "${YELLOW}Checking tmux session...${NC}"
if tmux has-session -t luxtensor 2>/dev/null; then
    echo -e "${GREEN}✓ Tmux session 'luxtensor' is active${NC}"
    echo "  Attach with: tmux attach -t luxtensor"
else
    echo -e "${RED}✗ No tmux session 'luxtensor' found${NC}"
fi
echo ""

# Function to check node status via RPC
check_node_rpc() {
    local node_num=$1
    local port=$2
    
    echo -e "${YELLOW}Node ${node_num} (http://localhost:${port}):${NC}"
    
    # Check if port is open
    if ! nc -z localhost $port 2>/dev/null; then
        echo -e "  ${RED}✗ RPC port ${port} is not accessible${NC}"
        return
    fi
    
    # Try to get block number
    response=$(curl -s -X POST http://localhost:${port} \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' 2>/dev/null)
    
    curl_status=$?
    
    if [[ $curl_status -eq 0 && -n "$response" ]]; then
        # Extract block number (it's in hex format)
        block_hex=$(echo $response | grep -o '"result":"0x[0-9a-fA-F]*"' | cut -d'"' -f4)
        if [ ! -z "$block_hex" ]; then
            # Convert hex to decimal
            block_dec=$((16#${block_hex#0x}))
            echo -e "  ${GREEN}✓ RPC is responding${NC}"
            echo -e "  ${GREEN}✓ Current block: ${block_dec}${NC}"
        else
            echo -e "  ${YELLOW}⚠ RPC responded but no block number found${NC}"
        fi
    else
        echo -e "  ${RED}✗ RPC is not responding${NC}"
    fi
    
    # Try to get peer count
    peer_response=$(curl -s -X POST http://localhost:${port} \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}' 2>/dev/null)
    
    if [[ -n "$peer_response" ]]; then
        peer_hex=$(echo $peer_response | grep -o '"result":"0x[0-9a-fA-F]*"' | cut -d'"' -f4)
        if [ ! -z "$peer_hex" ]; then
            peer_count=$((16#${peer_hex#0x}))
            echo -e "  ${GREEN}✓ Connected peers: ${peer_count}${NC}"
        fi
    fi
    echo ""
}

# Check if curl and nc are available
check_dependencies() {
    local missing_deps=()
    
    if ! command -v curl &> /dev/null; then
        missing_deps+=("curl")
    fi
    
    if ! command -v nc &> /dev/null; then
        missing_deps+=("nc (netcat)")
    fi
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        echo -e "${RED}Missing required tools: ${missing_deps[*]}${NC}"
        echo "Please install the missing tools and try again."
        exit 1
    fi
}

check_dependencies

# Check each node's RPC endpoint
echo -e "${YELLOW}Checking node RPC endpoints...${NC}\n"
check_node_rpc 1 8545
check_node_rpc 2 8555
check_node_rpc 3 8565

echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo -e "${GREEN}Status check complete!${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
