#!/bin/bash

# Script to stop all LuxTensor nodes
# Usage: ./stop-nodes.sh

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Stopping all LuxTensor nodes...${NC}"

# Check if tmux session exists
if tmux has-session -t luxtensor 2>/dev/null; then
    echo "Killing tmux session 'luxtensor'..."
    tmux kill-session -t luxtensor
    echo -e "${GREEN}✓ Tmux session terminated${NC}"
fi

# Kill any remaining luxtensor-node processes
if pgrep -f "luxtensor-node" > /dev/null; then
    echo "Stopping remaining luxtensor-node processes..."
    pkill -SIGTERM -f "luxtensor-node"
    
    # Wait a bit for graceful shutdown
    sleep 2
    
    # Force kill if still running
    if pgrep -f "luxtensor-node" > /dev/null; then
        echo "Force killing remaining processes..."
        pkill -SIGKILL -f "luxtensor-node"
    fi
    
    echo -e "${GREEN}✓ All node processes stopped${NC}"
else
    echo "No running luxtensor-node processes found"
fi

echo -e "${GREEN}All nodes stopped successfully!${NC}"
