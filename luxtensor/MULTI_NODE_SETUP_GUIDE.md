# Running Multiple LuxTensor Nodes - Local Network Setup Guide

This guide explains how to run multiple LuxTensor nodes on your local machine to create a local test network. This is useful for development, testing, and understanding how nodes communicate with each other.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Detailed Setup Instructions](#detailed-setup-instructions)
- [Configuration Explanation](#configuration-explanation)
- [Managing Your Local Network](#managing-your-local-network)
- [Troubleshooting](#troubleshooting)

## Prerequisites

Before starting, ensure you have:

- **Rust 1.75 or later** installed ([rustup.rs](https://rustup.rs/))
- **Git** installed
- **At least 2GB RAM** available
- **3 terminal windows/tabs** (or use tmux/screen)

## Quick Start

### Step 1: Build the Project

```bash
# Clone and build (if not already done)
cd /path/to/luxtensor
cargo build --release
```

### Step 2: Create Node Directories

```bash
# Create directories for 3 nodes
mkdir -p node1 node2 node3
```

### Step 3: Copy Configuration Files

```bash
# Copy the example configuration files
cp config.node1.toml node1/config.toml
cp config.node2.toml node2/config.toml
cp config.node3.toml node3/config.toml
```

### Step 4: Start the Nodes

Open 3 separate terminal windows and run:

**Terminal 1 - Node 1:**
```bash
cd node1
../target/release/luxtensor-node --config config.toml
```

**Terminal 2 - Node 2:**
```bash
cd node2
../target/release/luxtensor-node --config config.toml
```

**Terminal 3 - Node 3:**
```bash
cd node3
../target/release/luxtensor-node --config config.toml
```

## Detailed Setup Instructions

### Understanding the Node Configuration

Each node needs its own:
1. **Data directory** - Where blockchain data is stored
2. **Network port** - P2P communication port (must be unique)
3. **RPC port** - JSON-RPC API port (must be unique)

### Configuration Files

The repository includes example configurations for running 3 nodes:

- `config.node1.toml` - Node 1 configuration (Ports: P2P=30303, RPC=8545)
- `config.node2.toml` - Node 2 configuration (Ports: P2P=30304, RPC=8555)
- `config.node3.toml` - Node 3 configuration (Ports: P2P=30305, RPC=8565)

### Key Configuration Differences

Each node configuration differs in:

```toml
[node]
name = "node-1"  # Unique name for each node
data_dir = "./data"  # Local data directory

[network]
listen_port = 30303  # Unique P2P port (30303, 30304, 30305)
enable_mdns = true   # Enable local network discovery

[rpc]
listen_port = 8545   # Unique RPC port (8545, 8555, 8565)
```

### Node Discovery

The nodes will discover each other automatically using mDNS (Multicast DNS) since they're on the same local network. This is enabled by:

```toml
[network]
enable_mdns = true
```

For manual peer connections, you can specify bootstrap nodes after getting the node IDs.

## Configuration Explanation

### Node Section
- **name**: Human-readable identifier for the node
- **chain_id**: Must be the same for all nodes (1 for local dev)
- **data_dir**: Where the node stores blockchain data
- **is_validator**: Set to `true` if this node should participate in consensus

### Network Section
- **listen_addr**: "0.0.0.0" allows connections from all interfaces
- **listen_port**: P2P port for node communication (must be unique per node)
- **bootstrap_nodes**: List of seed nodes to connect to initially
- **max_peers**: Maximum number of peer connections (default: 50)
- **enable_mdns**: Automatic peer discovery on local network

### Storage Section
- **db_path**: RocksDB database location
- **enable_compression**: Compress stored data (recommended: true)
- **cache_size**: Memory cache size in MB (default: 256)

### RPC Section
- **enabled**: Enable JSON-RPC HTTP server
- **listen_addr**: "127.0.0.1" for local only, "0.0.0.0" for all interfaces
- **listen_port**: HTTP API port (must be unique per node)
- **cors_origins**: CORS policy (["*"] for development)

## Managing Your Local Network

### Checking Node Status

Query each node's status using the RPC API:

```bash
# Node 1 status
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"lux_blockNumber","params":[],"id":1}'

# Node 2 status
curl -X POST http://localhost:8555 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"lux_blockNumber","params":[],"id":1}'

# Node 3 status
curl -X POST http://localhost:8565 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"lux_blockNumber","params":[],"id":1}'
```

### Viewing Peer Connections

Check connected peers:

```bash
# Check Node 1 peers
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}'
```

### Monitoring Logs

Each node outputs logs to stdout. Watch for:
- ✅ "Node started" - Node initialization successful
- ✅ "Peer connected" - Successful peer connection
- ✅ "Block received" - Receiving blocks from peers
- ✅ "Block produced" - Node produced a new block (validators)

### Stopping Nodes

Press `Ctrl+C` in each terminal to gracefully shutdown nodes.

### Cleaning Up

To start fresh with clean state:

```bash
# Stop all nodes first, then:
rm -rf node1/data node2/data node3/data
```

## Using Helper Scripts

### Starting All Nodes (Using tmux)

Create a script `start-nodes.sh`:

```bash
#!/bin/bash

# Start 3 nodes in separate tmux windows
tmux new-session -d -s luxtensor 'cd node1 && ../target/release/luxtensor-node --config config.toml'
tmux split-window -h 'cd node2 && ../target/release/luxtensor-node --config config.toml'
tmux split-window -v 'cd node3 && ../target/release/luxtensor-node --config config.toml'
tmux select-layout tiled
tmux attach-session -t luxtensor
```

Make it executable:
```bash
chmod +x start-nodes.sh
./start-nodes.sh
```

### Stopping All Nodes

Create a script `stop-nodes.sh`:

```bash
#!/bin/bash

# Stop all luxtensor-node processes
pkill -SIGTERM luxtensor-node
echo "All nodes stopped"
```

## Advanced Configuration

### Running Validator Nodes

To run nodes as validators:

1. Generate validator keys for each node:
```bash
./target/release/luxtensor validator keygen --output node1/validator.key
./target/release/luxtensor validator keygen --output node2/validator.key
./target/release/luxtensor validator keygen --output node3/validator.key
```

2. Update each node's configuration:
```toml
[node]
is_validator = true
validator_key_path = "./validator.key"
```

3. Stake tokens (requires tokens in the account):
```bash
./target/release/luxtensor stake --amount 10000000000000000000 --rpc http://localhost:8545
```

### Custom Genesis Configuration

For a custom local network with specific initial state:

1. Create a genesis configuration file
2. All nodes must use the same genesis file
3. Specify genesis file in node configuration

## Troubleshooting

### Port Already in Use

**Error**: "Address already in use"

**Solution**: Another process is using the port. Either:
- Stop the other process: `lsof -ti:8545 | xargs kill`
- Change the port in the configuration file

### Nodes Not Discovering Each Other

**Problem**: Nodes remain isolated

**Solutions**:
1. Ensure `enable_mdns = true` in all node configurations
2. Check firewall settings - allow UDP multicast (5353)
3. Manually add peer connections using bootstrap_nodes
4. Ensure all nodes are on the same network interface

### Database Lock Error

**Error**: "Database is locked" or "Cannot acquire lock"

**Solution**: 
- Only one node can use a data directory at a time
- Ensure you're using different data directories for each node
- Check if another node process is still running: `ps aux | grep luxtensor-node`

### High CPU Usage

**Problem**: Nodes consuming excessive CPU

**Solutions**:
- This is normal during initial sync
- Reduce `max_peers` in configuration
- Increase `block_time` in consensus configuration

### Memory Issues

**Problem**: Out of memory errors

**Solutions**:
- Reduce `cache_size` in storage configuration (e.g., from 256 to 128 MB)
- Enable pruning in configuration
- Close other applications to free memory

### Checking Logs in Detail

For detailed logging:

```bash
# Set log level to debug in config.toml
[logging]
level = "debug"

# Or use environment variable
RUST_LOG=debug ./target/release/luxtensor-node --config config.toml
```

## Network Topology Examples

### Linear Topology
```
Node1 <-> Node2 <-> Node3
```
Set bootstrap_nodes to connect sequentially.

### Star Topology
```
    Node1
    /  \
Node2  Node3
```
Node2 and Node3 connect to Node1 as bootstrap.

### Full Mesh
```
Node1 <-> Node2
  \      /
   Node3
```
All nodes discover each other via mDNS.

## Performance Tips

1. **SSD Storage**: Use SSD for data directories for better I/O performance
2. **Memory**: Allocate sufficient RAM (512MB-1GB per node)
3. **CPU**: Multi-core processors benefit from parallel block processing
4. **Network**: Use wired connection for stability

## Next Steps

After successfully running your local network:

1. **Interact with nodes** using the CLI tools
2. **Send transactions** between nodes
3. **Deploy smart contracts** on your local network
4. **Test consensus** by stopping/starting validators
5. **Monitor performance** using metrics endpoints

## Additional Resources

- [Main README](README.md) - Project overview and features
- [Data Sync Guide](DATA_SYNC_TEST_GUIDE.md) - Understanding node synchronization
- [API Documentation](docs/api.md) - RPC API reference
- [Examples](examples/) - Code examples for interacting with nodes

## Support

For issues or questions:
- Open an issue on GitHub: https://github.com/sonson0910/luxtensor/issues
- Check existing documentation in the `/docs` directory
- Review test cases in `/crates/luxtensor-tests`

---

**Happy node running! 🚀**
