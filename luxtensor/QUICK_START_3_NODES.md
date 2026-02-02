# Quick Reference: Running 3 LuxTensor Nodes

## Prerequisites

1. Build the project:

```bash
cargo build --release
```

1. Ensure you have `tmux` installed (optional, for automated startup):

```bash
# Ubuntu/Debian
sudo apt-get install tmux

# macOS
brew install tmux
```

## Method 1: Using Helper Scripts (Recommended)

### Start All Nodes

```bash
./start-nodes.sh
```

This will:

- Create `node1`, `node2`, and `node3` directories
- Copy configuration files
- Start all 3 nodes in a tmux session
- Each node has unique ports (P2P, RPC, WebSocket)

### Check Node Status

```bash
./check-nodes.sh
```

This displays:

- Running processes
- Tmux session status
- RPC connectivity for each node
- Current block height
- Connected peers

### Stop All Nodes

```bash
./stop-nodes.sh
```

### Tmux Controls (after attaching)

- **Attach**: `tmux attach -t luxtensor`
- **Detach**: `Ctrl+B` then `D`
- **Switch panes**: `Ctrl+B` then arrow keys
- **Close pane**: `Ctrl+D` or type `exit`

## Method 2: Manual Startup (3 Terminal Windows)

### Terminal 1 - Node 1

```bash
mkdir -p node1
cp config.node1.toml node1/config.toml
cd node1
../target/release/luxtensor-node --config config.toml
```

### Terminal 2 - Node 2

```bash
mkdir -p node2
cp config.node2.toml node2/config.toml
cd node2
../target/release/luxtensor-node --config config.toml
```

### Terminal 3 - Node 3

```bash
mkdir -p node3
cp config.node3.toml node3/config.toml
cd node3
../target/release/luxtensor-node --config config.toml
```

## Node Endpoints

| Node | P2P Port | RPC Endpoint |
|------|----------|--------------|
| Node 1 | 30303 | <http://localhost:8545> |
| Node 2 | 30304 | <http://localhost:8555> |
| Node 3 | 30305 | <http://localhost:8565> |

## Testing the Network

### Query Block Number

```bash
# Node 1
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"lux_blockNumber","params":[],"id":1}'

# Node 2
curl -X POST http://localhost:8555 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"lux_blockNumber","params":[],"id":1}'

# Node 3
curl -X POST http://localhost:8565 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"lux_blockNumber","params":[],"id":1}'
```

### Check Peer Connections

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}'
```

## Troubleshooting

### Port Already in Use

```bash
# Find process using the port
lsof -ti:8545

# Kill the process
lsof -ti:8545 | xargs kill
```

### Clean Start (Remove All Data)

```bash
# Stop all nodes first
./stop-nodes.sh

# Remove all blockchain data
rm -rf node1/data node2/data node3/data

# Start nodes again
./start-nodes.sh
```

### View Logs in Real-Time

When running manually, logs appear in each terminal window.

With tmux:

1. Attach to session: `tmux attach -t luxtensor`
2. Navigate between panes with `Ctrl+B` + arrow keys
3. Scroll logs: `Ctrl+B` then `[`, use arrow keys, press `q` to exit scroll mode

## Configuration Files

Each node has its own configuration file with unique ports:

- `config.node1.toml` → Node 1 config (P2P=30303, RPC=8545)
- `config.node2.toml` → Node 2 config (P2P=30304, RPC=8555)
- `config.node3.toml` → Node 3 config (P2P=30305, RPC=8565)

Key differences in each config:

```toml
[node]
name = "node-1"  # Unique name

[network]
listen_port = 30303  # Unique P2P port

[rpc]
listen_port = 8545  # Unique RPC port
```

## Next Steps

After running your local network:

1. Interact with nodes using CLI tools
2. Send transactions between nodes
3. Deploy and test smart contracts
4. Monitor network synchronization
5. Test validator operations

## Full Documentation

For comprehensive guides:

- [Multi-Node Setup Guide (English)](MULTI_NODE_SETUP_GUIDE.md)
- [Hướng Dẫn Chạy Nhiều Node (Tiếng Việt)](HUONG_DAN_CHAY_NHIEU_NODE.md)
- [Data Synchronization Guide](DATA_SYNC_TEST_GUIDE.md)
- [Main README](README.md)

---

**Happy testing! 🚀**
