# ModernTensor SDK

Python SDK for interacting with Luxtensor blockchain and building AI/ML subnets.

## ⚠️ Important Migration Notice

**ModernTensor has migrated from Cardano to Luxtensor blockchain.**

- ✅ **Luxtensor** is now the official Layer 1 blockchain (account-based, Ethereum-style)
- ❌ **Cardano/PyCardano** dependencies have been removed (UTXO-based, incompatible)

If you're upgrading from an older version that used Cardano:

- See [CARDANO_DEPRECATION.md](../CARDANO_DEPRECATION.md) for migration guide
- Old UTXO/Datum/Redeemer code will not work with Luxtensor
- Use `LuxtensorClient` for all blockchain interactions

## Architecture

ModernTensor consists of two layers:

1. **Luxtensor (Blockchain Layer)** - Rust-based custom Layer 1 blockchain
   - Location: `/luxtensor/` directory
   - Handles: Consensus, P2P, Storage, RPC APIs
   - Status: Phase 1 complete, ongoing development

2. **ModernTensor SDK (Python Layer)** - This package
   - Location: `/sdk/` directory
   - Handles: Python client, AI/ML framework, developer tools
   - Status: Under active development

## Quick Start

### Installation

```bash
pip install -e .
```

### Connect to Luxtensor

```python
# ✅ After pip install (recommended)
from moderntensor.sdk import connect, LuxtensorClient

# ✅ Development mode (from repo root)
# import sys; sys.path.insert(0, 'moderntensor')
# from sdk import connect

# Connect to Luxtensor blockchain
client = connect(url="http://localhost:8545", network="testnet")

# Check connection
if client.is_connected():
    print("Connected!")

# Get current block
block_number = client.get_block_number()
print(f"Current block: {block_number}")

# Get account info
account = client.get_account("0x...")
print(f"Balance: {account.balance}")
```

### Async Usage (High Performance)

```python
from moderntensor.sdk import async_connect
import asyncio

async def main():
    # Async client for high performance
    client = await async_connect(url="http://localhost:8545")

    # Batch multiple queries in single round-trip
    calls = [
        ("eth_blockNumber", []),
        ("staking_getValidators", []),
    ]
    results = await client.batch_call(calls)
    print(f"Block: {results[0]}, Validators: {len(results[1])}")

asyncio.run(main())
```

### Sync vs Async Client

| Feature | `LuxtensorClient` (Sync) | `AsyncLuxtensorClient` |
|---------|-------------------------|------------------------|
| **Use Case** | Simple scripts, CLI | High-perf apps, servers |
| **Batch Calls** | ❌ Not available | ✅ `batch_call()` |
| **Concurrent Requests** | ❌ Sequential | ✅ `asyncio.gather()` |
| **Thread-safe** | ✅ Yes | N/A (async) |

> **Tip:** Use sync for learning/prototyping, async for production servers.

## Features

### ✅ Implemented

- **Luxtensor Client** - Python client to interact with Luxtensor blockchain
  - Sync and async operations
  - Account queries (balance, nonce, stake)
  - Block and transaction queries
  - Validator information
  - Subnet and neuron queries
  - Batch operations (async)

- **CLI Tools** (`mtcli`)
  - Wallet management (coldkey/hotkey)
  - Transaction operations
  - Staking operations

- **AI/ML Framework**
  - Subnet framework
  - zkML integration (ezkl)

- **Key Management**
  - Coldkey/hotkey generation
  - Key derivation

### 🚧 In Progress

- **Axon Server** - Server for miners/validators to serve AI models
- **Dendrite Client** - Client to query miners for AI inference
- **Synapse Protocol** - Request/response data structures for AI/ML
- **Enhanced Metagraph** - Network topology and miner rankings
- **Testing Framework** - Comprehensive test suite

### 📋 Planned

- Advanced AI/ML scoring mechanisms
- Subnet templates and tools
- Developer documentation
- Performance optimizations

## Project Structure

```
sdk/
├── __init__.py              # SDK exports (backward compatible)
├── luxtensor_client.py      # Main client (74KB, 140+ methods)
├── async_luxtensor_client.py # Async client
├── luxtensor_pallets.py     # Pallet call encoding (keccak256)
├── transactions.py          # Transaction signing
│
├── client/                  # NEW - Modular client components
│   ├── base.py             # BaseClient, data classes
│   ├── blockchain_mixin.py # Block/chain methods
│   ├── account_mixin.py    # Account methods
│   ├── transaction_mixin.py# TX methods
│   ├── staking_mixin.py    # Staking methods
│   ├── subnet0_mixin.py    # Root Subnet methods
│   └── neuron_mixin.py     # Neuron/AI methods
│
├── core/                    # Core utilities
│   ├── cache.py            # LRU cache with TTL
│   └── datatypes.py        # Core data types
│
├── models/                  # Pydantic data models
│   ├── subnet.py           # SubnetInfo, RootConfig
│   ├── root_subnet.py      # RootSubnet manager
│   ├── neuron.py           # NeuronInfo
│   └── ...                 # Other models
│
├── ai_ml/                   # AI/ML framework
│   ├── core/               # SubnetProtocol
│   ├── zkml/               # Zero-knowledge ML proofs
│   ├── agent/              # Miner/Validator AI agents
│   └── ...
│
├── axon/                    # Server for miners/validators
├── dendrite/                # Client for AI queries
├── synapse/                 # Request/response protocol
├── security/                # RBAC, auditing
├── keymanager/              # Wallet/key management
├── cli/                     # CLI tools (mtcli)
├── tokenomics/              # Token economics
└── monitoring/              # Metrics and monitoring
```

## Development

### Running Luxtensor Node

The SDK requires a running Luxtensor node:

```bash
cd luxtensor
cargo run --release
```

### Running Examples

```bash
# Luxtensor client example
python examples/luxtensor_client_example.py

# More examples coming soon...
```

### Testing

```bash
pytest tests/
```

## Documentation

- **[QUICKSTART.md](QUICKSTART.md)** - Complete examples for common use cases ⭐
- [SDK Redesign Roadmap](../SDK_REDESIGN_ROADMAP.md) - Implementation plan
- [Architecture Clarification](../SDK_ARCHITECTURE_CLARIFICATION.md) - SDK vs Blockchain
- [API Reference](API_REFERENCE.md) - All RPC methods
- [Luxtensor README](../luxtensor/README.md) - Blockchain layer docs

## API Reference

### LuxtensorClient

Synchronous client for blockchain interaction.

**Methods:**

- `get_chain_info()` - Get blockchain information
- `get_block_number()` - Get current block height
- `get_block(block_number)` - Get block by number
- `get_account(address)` - Get account info
- `get_balance(address)` - Get account balance
- `get_nonce(address)` - Get account nonce
- `submit_transaction(signed_tx)` - Submit transaction
- `get_transaction(tx_hash)` - Get transaction
- `get_validators()` - Get active validators
- `get_subnet_info(subnet_id)` - Get subnet info
- `get_neurons(subnet_id)` - Get neurons in subnet
- `get_weights(subnet_id, neuron_uid)` - Get weight matrix
- `get_stake(address)` - Get staked amount
- `is_connected()` - Check connection

### AsyncLuxtensorClient

Asynchronous client for high-performance operations.

**Additional Methods:**

- `batch_call(calls)` - Execute multiple RPC calls concurrently

All sync methods have async equivalents.

## Contributing

We welcome contributions! Areas where help is needed:

1. **Axon Server** - Building the miner/validator server
2. **Dendrite Client** - Query client for AI inference
3. **Synapse Protocol** - AI/ML protocol definitions
4. **Metagraph** - Network topology management
5. **Testing** - Writing comprehensive tests
6. **Documentation** - API docs and tutorials

## License

MIT License

## Links

- **Luxtensor (Blockchain):** `/luxtensor/`
- **Roadmap:** [SDK_REDESIGN_ROADMAP.md](../SDK_REDESIGN_ROADMAP.md)
- **ModernTensor Whitepaper:** [MODERNTENSOR_WHITEPAPER_VI.md](../MODERNTENSOR_WHITEPAPER_VI.md)
