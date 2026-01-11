#!/bin/bash

# Script to start 3 LuxTensor nodes in separate tmux windows
# Usage: ./start-nodes.sh

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  LuxTensor Multi-Node Startup Script${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}\n"

# Check if tmux is installed
if ! command -v tmux &> /dev/null; then
    echo -e "${RED}Error: tmux is not installed${NC}"
    echo "Please install tmux first:"
    echo "  Ubuntu/Debian: sudo apt-get install tmux"
    echo "  macOS: brew install tmux"
    exit 1
fi

# Check if the binary exists
if [ ! -f "./target/release/luxtensor-node" ]; then
    echo -e "${RED}Error: luxtensor-node binary not found${NC}"
    echo "Please build the project first:"
    echo "  cargo build --release"
    exit 1
fi

# Create node directories if they don't exist
echo -e "${YELLOW}Creating node directories...${NC}"
mkdir -p node1 node2 node3

# Copy config files if they don't exist
for i in 1 2 3; do
    if [ ! -f "node${i}/config.toml" ]; then
        echo -e "${YELLOW}Copying config.node${i}.toml to node${i}/config.toml${NC}"
        cp "config.node${i}.toml" "node${i}/config.toml"
    fi
done

# Check if session already exists
if tmux has-session -t luxtensor 2>/dev/null; then
    echo -e "${YELLOW}Existing luxtensor session found. Killing it...${NC}"
    tmux kill-session -t luxtensor
fi

echo -e "${GREEN}Starting 3 nodes in tmux session 'luxtensor'...${NC}\n"

# Create new tmux session with 3 panes
tmux new-session -d -s luxtensor -n nodes

# Start Node 1 in the first pane
tmux send-keys -t luxtensor:nodes.0 'cd node1 && echo "Starting Node 1..." && ../target/release/luxtensor-node --config config.toml' C-m

# Split horizontally and start Node 2
tmux split-window -h -t luxtensor:nodes
tmux send-keys -t luxtensor:nodes.1 'cd node2 && echo "Starting Node 2..." && ../target/release/luxtensor-node --config config.toml' C-m

# Split the right pane vertically and start Node 3
tmux split-window -v -t luxtensor:nodes.1
tmux send-keys -t luxtensor:nodes.2 'cd node3 && echo "Starting Node 3..." && ../target/release/luxtensor-node --config config.toml' C-m

# Adjust layout to tiled
tmux select-layout -t luxtensor:nodes tiled

echo -e "${GREEN}✓ All nodes started successfully!${NC}\n"
echo "Tmux session 'luxtensor' created with 3 nodes"
echo ""
echo "Commands:"
echo "  - Attach to session: ${BLUE}tmux attach -t luxtensor${NC}"
echo "  - Detach from session: ${BLUE}Ctrl+B then D${NC}"
echo "  - Switch between panes: ${BLUE}Ctrl+B then arrow keys${NC}"
echo "  - Stop all nodes: ${BLUE}./stop-nodes.sh${NC}"
echo ""
echo "Node endpoints:"
echo "  - Node 1: RPC=http://localhost:8545"
echo "  - Node 2: RPC=http://localhost:8555"
echo "  - Node 3: RPC=http://localhost:8565"
echo ""

# Ask if user wants to attach
read -t 10 -p "Attach to tmux session now? [Y/n] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]] || [[ -z $REPLY ]]; then
    tmux attach -t luxtensor
fi
