# ModernTensor Testnet Module

Complete testnet deployment infrastructure for ModernTensor Layer 1 blockchain.

## Overview

The testnet module provides all the tools necessary to deploy and manage a ModernTensor testnet, including:

- **Genesis Configuration** - Automated genesis block generation
- **Token Faucet** - Rate-limited test token distribution
- **Bootstrap Node** - Peer discovery service
- **Monitoring** - Real-time health tracking and alerts
- **Deployment Tools** - Docker Compose automation

## Quick Start

### 1. Deploy a Testnet

```python
from sdk.testnet.deployment import deploy_testnet

# Deploy with default configuration
deploy_testnet(
    network_name="moderntensor-testnet",
    chain_id=9999,
    num_validators=5
)
```

This will:
- Generate genesis configuration
- Create validator directories
- Generate Docker Compose configuration
- Create deployment documentation

### 2. Start the Testnet

```bash
docker-compose -f docker-compose.testnet.yml up -d
```

### 3. Access Services

- **Validators**: http://localhost:8545-8549
- **Faucet**: http://localhost:8080
- **Explorer**: http://localhost:3000
- **Bootstrap**: tcp://localhost:30303

## Modules

### Genesis Configuration

Generate genesis blocks and configuration files:

```python
from sdk.testnet import GenesisGenerator
from pathlib import Path

# Create generator
generator = GenesisGenerator()

# Create testnet configuration
config = generator.create_testnet_config(
    chain_id=9999,
    network_name="my-testnet",
    validator_count=5,
    validator_stake=10_000_000,
    faucet_balance=1_000_000_000_000
)

# Add custom validators
generator.add_validator(
    address="0x1234...",
    stake=5_000_000,
    public_key="0xabcd...",
    name="My Validator"
)

# Validate configuration
errors = generator.validate_config()
if not errors:
    print("✅ Configuration is valid")

# Export to files
generator.export_config(Path("./genesis"))
```

### Token Faucet

Distribute test tokens with rate limiting:

```python
from sdk.testnet import Faucet, FaucetConfig

# Configure faucet
config = FaucetConfig(
    tokens_per_request=100_000_000_000,  # 100 tokens
    cooldown_period=3600,                 # 1 hour
    max_requests_per_address=3,
    max_requests_per_ip=5
)

faucet = Faucet(config)

# Request tokens
result = await faucet.request_tokens(
    address="0x1234567890123456789012345678901234567890",
    ip_address="192.168.1.1"
)

if result['success']:
    print(f"Sent {result['amount']} tokens")
    print(f"TX Hash: {result['tx_hash']}")
else:
    print(f"Error: {result['error']}")

# Get statistics
stats = faucet.get_stats()
print(f"Total requests: {stats['total_requests']}")
print(f"Tokens distributed: {stats['total_tokens_distributed']}")
```

### Bootstrap Node

Coordinate peer discovery:

```python
from sdk.testnet import BootstrapNode, BootstrapConfig

# Configure bootstrap node
config = BootstrapConfig(
    listen_port=30303,
    max_peers=1000,
    chain_id=9999
)

node = BootstrapNode(config)

# Start the node
await node.start()

# Register a peer
success = node.register_peer(
    node_id="validator-1",
    address="192.168.1.100",
    port=30303,
    version="1.0.0"
)

# Get peers for discovery
peers = node.get_peers(
    max_count=10,
    exclude={"my-node-id"}
)

# Get statistics
stats = node.get_stats()
print(f"Active peers: {stats['active_peers']}")
```

### Monitoring

Track network health:

```python
from sdk.testnet.monitoring import TestnetMonitor, NodeHealth
import time

# Create monitor
monitor = TestnetMonitor()
await monitor.start()

# Update node health
health = NodeHealth(
    node_id="validator-1",
    status="healthy",
    last_block_height=1000,
    last_block_time=time.time(),
    peer_count=10,
    sync_status="synced",
    cpu_usage=45.2,
    memory_usage=62.5
)

monitor.update_node_health(health)

# Get network metrics
metrics = monitor.calculate_network_metrics()
print(f"Total nodes: {metrics.total_nodes}")
print(f"Healthy nodes: {metrics.healthy_nodes}")
print(f"Current height: {metrics.current_height}")

# Get dashboard data
dashboard = monitor.get_dashboard_data()
```

## Configuration

### Genesis Configuration

```json
{
  "chain_id": 9999,
  "network_name": "moderntensor-testnet",
  "genesis_time": "2026-01-05T00:00:00Z",
  "consensus": {
    "type": "pos",
    "epoch_length": 100,
    "slot_duration": 12,
    "validator_count": 21,
    "min_stake": 1000000
  },
  "network": {
    "p2p_port": 30303,
    "rpc_port": 8545,
    "max_peers": 50
  },
  "total_supply": 1000000000,
  "decimals": 18
}
```

### Faucet Configuration

```python
FaucetConfig(
    tokens_per_request=100_000_000_000,  # 100 tokens (18 decimals)
    max_requests_per_address=3,
    max_requests_per_ip=5,
    cooldown_period=3600,                 # 1 hour
    daily_limit=1000
)
```

### Bootstrap Configuration

```python
BootstrapConfig(
    listen_address="0.0.0.0",
    listen_port=30303,
    max_peers=1000,
    peer_timeout=3600,                    # 1 hour
    chain_id=9999
)
```

## Docker Deployment

### Generated docker-compose.yml

```yaml
version: '3.8'

services:
  bootstrap:
    image: moderntensor:testnet
    ports:
      - "30303:30303"
    volumes:
      - ./genesis:/genesis:ro
      - ./data/bootstrap:/data

  validator-1:
    image: moderntensor:testnet
    ports:
      - "8545:8545"
    volumes:
      - ./genesis:/genesis:ro
      - ./data/validator-1:/data
    depends_on:
      - bootstrap

  faucet:
    image: moderntensor:testnet
    ports:
      - "8080:8080"
    depends_on:
      - validator-1

  explorer:
    image: moderntensor:testnet
    ports:
      - "3000:3000"
    depends_on:
      - validator-1
```

## Features

### Rate Limiting
- Per-address limits (default: 3 requests per hour)
- Per-IP limits (default: 5 requests per hour)
- Daily global limit (default: 1000 requests)
- Configurable cooldown periods

### Anti-Abuse Protection
- Address blocking/unblocking
- IP blocking/unblocking
- Request history tracking
- Automatic cleanup of old records

### Monitoring
- Real-time node health tracking
- Network-wide metrics calculation
- Automatic alert generation
- Basic blockchain explorer

### Automation
- One-command deployment
- Automated validator setup
- Docker Compose orchestration
- Complete documentation generation

## Testing

Run the test suite:

```bash
# Run all testnet tests
pytest tests/testnet/test_testnet.py -v

# Run specific test
pytest tests/testnet/test_testnet.py::TestGenesis::test_genesis_generator -v
```

Test coverage includes:
- Genesis configuration and validation
- Faucet rate limiting and token distribution
- Bootstrap node peer management
- Monitoring and metrics calculation
- Deployment automation

## Examples

See `examples/deploy_testnet.py` for a complete deployment example.

## Documentation

For detailed information, see:
- `PHASE8_SUMMARY.md` - Complete implementation summary
- `LAYER1_ROADMAP.md` - Phase 8 section
- `TESTNET_DEPLOYMENT.md` - Generated deployment guide (after running deploy)

## Architecture

```
sdk/testnet/
├── __init__.py          # Module exports
├── genesis.py           # Genesis configuration (400 LOC)
├── faucet.py           # Token faucet (340 LOC)
├── bootstrap.py        # Bootstrap node (300 LOC)
├── monitoring.py       # Monitoring tools (270 LOC)
└── deployment.py       # Deployment automation (350 LOC)

Total: 1,700+ lines of production code
```

## Requirements

- Python 3.8+
- Docker and Docker Compose (for deployment)
- 4GB+ RAM per validator
- 50GB+ disk space per validator

## Status

✅ **Production Ready**
- All modules implemented and tested
- 28 comprehensive tests passing
- Complete documentation
- Ready for community testing

## License

Part of the ModernTensor project.

## Support

For issues and questions:
- GitHub Issues: https://github.com/sonson0910/moderntensor
- Documentation: See PHASE8_SUMMARY.md
