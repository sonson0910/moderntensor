# ModernTensor SDK

[![PyPI version](https://img.shields.io/pypi/v/moderntensor.svg)](https://pypi.org/project/moderntensor/)
[![Python 3.10+](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/downloads/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Python SDK for interacting with the **Luxtensor** blockchain and building decentralized AI/ML subnets on the ModernTensor network.

---

## Quick Install

```bash
pip install moderntensor
```

Or install from source for development:

```bash
git clone https://github.com/sonson0910/moderntensor.git
cd moderntensor
pip install -e ".[dev]"
```

## Quick Start

### Connect to the blockchain

```python
from sdk import connect

# Connect to a Luxtensor node
client = connect(url="http://localhost:8545", network="testnet")

# Check connection
print(client.is_connected())  # True
```

### Query the latest block

```python
block_number = client.get_block_number()
print(f"Latest block: {block_number}")

block = client.get_block(block_number)
print(f"Block hash: {block.hash}")
print(f"Transactions: {len(block.transactions)}")
```

### Query an account

```python
account = client.get_account("0xYourAddress...")
print(f"Balance : {account.balance} MDT")
print(f"Nonce   : {account.nonce}")
print(f"Stake   : {account.stake}")
```

### Send a transaction

```python
from sdk.transactions import create_transfer_transaction, sign_transaction

tx = create_transfer_transaction(
    sender="0xSenderAddress",
    recipient="0xRecipientAddress",
    amount=1_000_000,
    nonce=account.nonce,
)
signed = sign_transaction(tx, private_key="0xYourPrivateKey")
result = client.submit_transaction(signed)
print(f"Tx hash: {result.tx_hash}")
```

### Async client (high-performance)

```python
import asyncio
from sdk import async_connect

async def main():
    client = await async_connect(url="http://localhost:8545")

    # Batch multiple RPC calls in a single round-trip
    results = await client.batch_call([
        ("eth_blockNumber", []),
        ("staking_getValidators", []),
    ])
    print(f"Block: {results[0]}, Validators: {len(results[1])}")

asyncio.run(main())
```

## Metagraph API (`lux_*` RPC Methods)

The SDK exposes the **MetagraphDB-backed `lux_*` namespace** — the canonical source of truth for subnet, neuron, and weight state. Data is written via a **dual-write strategy**: legacy handlers persist to both the in-memory cache (`DashMap`) and the new persistent `MetagraphDB` (RocksDB).

### Subnet queries

```python
# List all subnets
subnets = client.lux_list_subnets()           # → list[dict]

# Get a specific subnet
subnet = client.lux_get_subnet_info(subnet_id=1)  # → dict | None

# Get emission schedule
emissions = client.lux_get_emissions(subnet_id=1) # → dict
```

### Neuron queries

```python
# All neurons in a subnet
neurons = client.lux_get_neurons(subnet_id=1)     # → list[dict]

# Single neuron
neuron = client.lux_get_neuron(subnet_id=1, uid=0) # → dict | None

# Count
count = client.lux_get_neuron_count(subnet_id=1)  # → int
```

### Weight queries

```python
# Weights set by a specific neuron
weights = client.lux_get_weights(subnet_id=1, uid=0)   # → list[dict]

# All weights in a subnet
all_w = client.lux_get_all_weights(subnet_id=1)        # → list[dict]
```

### Async versions

```python
import asyncio
from sdk import async_connect

async def main():
    client = await async_connect("http://localhost:8545")
    neurons = await client.lux_get_neurons(subnet_id=1)
    print(neurons)

asyncio.run(main())
```

## Features

| Area | Highlights |
|------|-----------|
| **Blockchain Client** | Sync & async RPC client, batch calls, account/block/tx queries |
| **MetagraphDB (`lux_*`)** | Persistent subnet/neuron/weight state via RocksDB dual-write |
| **Consensus** | Slashing, circuit breaker, liveness monitoring, fork choice (GHOST), fast finality |
| **AI/ML Framework** | Subnet protocol, zkML integration (ezkl), advanced scoring, node tiers |
| **CLI (`mtcli`)** | Wallet management, transactions, staking, subnet operations |
| **Key Management** | BIP39/BIP44 coldkey/hotkey, PBKDF2 + Fernet encryption |
| **Networking** | Axon server (miners), Dendrite client (validators), Synapse protocol |
| **Tokenomics** | Reward calculation, emission schedules, staking mechanics |
| **Monitoring** | Prometheus metrics, OpenTelemetry tracing |

## RPC Namespaces

| Namespace | Purpose |
|-----------|---------|
| `eth_*` | Ethereum-compatible: block, account, tx |
| `staking_*` | Validator staking, registration, rewards |
| `subnet_*` | Legacy subnet operations (write to DashMap + MetagraphDB) |
| `neuron_*` | Legacy neuron operations (write to DashMap + MetagraphDB) |
| `weight_*` | Legacy weight operations (write to DashMap + MetagraphDB) |
| `lux_*` | **New** — reads from MetagraphDB (source of truth) |
| `metagraph_*` | Full metagraph state, consensus views |
| `system_*` | Node health, roles, stats |

## Project Structure

```
sdk/
├── __init__.py                # Public API exports
├── luxtensor_client.py        # Sync RPC client (lux_* + legacy)
├── async_luxtensor_client.py  # Async RPC client
├── transactions.py            # Transaction creation & signing
├── websocket_client.py        # Real-time event subscriptions
├── client/
│   ├── subnet_mixin.py        # lux_getSubnetInfo, lux_listSubnets…
│   ├── neuron_mixin.py        # lux_getNeurons, lux_getNeuron…
│   ├── weights_mixin.py       # lux_getWeights, lux_getAllWeights…
│   └── metagraph_mixin.py     # metagraph_getState, metagraph_getWeights…
├── cli/                       # mtcli command-line tool
├── ai_ml/                     # AI/ML framework & zkML
├── axon/                      # Miner/validator server
├── dendrite/                  # AI inference query client
├── synapse/                   # Request/response protocol
├── consensus/                 # PoS consensus logic
├── core/                      # Cache, data types, scoring
├── models/                    # Pydantic data models
├── keymanager/                # Wallet & key management
├── security/                  # RBAC, auditing, rate limiting
├── tokenomics/                # Token economics
└── monitoring/                # Metrics & tracing
```

## API Reference

### LuxtensorClient (sync)

| Method | Description |
|--------|-------------|
| `connect(url, network)` | Create a connected client |
| `get_chain_info()` | Blockchain metadata |
| `get_block_number()` | Current block height |
| `get_block(n)` | Block by number |
| `get_account(addr)` | Account info (balance, nonce, stake) |
| `get_balance(addr)` | Account balance |
| `submit_transaction(tx)` | Submit signed transaction |
| `get_transaction(hash)` | Transaction by hash |
| `get_validators()` | Active validator set |
| `get_subnet_info(id)` | Subnet metadata (legacy cache) |
| `lux_get_subnet_info(id)` | Subnet metadata (MetagraphDB) |
| `get_neurons(subnet_id)` | Neurons in a subnet (legacy) |
| `lux_get_neurons(subnet_id)` | Neurons in a subnet (MetagraphDB) |
| `lux_get_weights(subnet_id, uid)` | Weights from MetagraphDB |
| `lux_get_all_weights(subnet_id)` | All weights from MetagraphDB |
| `get_stake(addr)` | Staked amount |
| `is_connected()` | Connection status |

### AsyncLuxtensorClient

All sync methods are available as coroutines, plus:

| Method | Description |
|--------|-------------|
| `batch_call(calls)` | Execute multiple RPC calls concurrently |

## Integration Tests

Run the full integration test suite against a live node:

```bash
# Start node first
cargo run --bin luxtensor-node

# Run tests (another terminal)
python tests/integration_test_sdk.py
python tests/integration_test_sdk.py --rpc http://your-node:8545
```

Tests cover: node health, `lux_*` subnet/neuron/weight queries, dual-write validation, error handling.

## Documentation

- [Quick Start Guide](QUICKSTART.md)
- [API Reference](API_REFERENCE.md)
- [Architecture Overview](../docs/architecture/)
- [Whitepaper (Vietnamese)](../MODERNTENSOR_WHITEPAPER_VI.md)
- [Changelog](../CHANGELOG.md)

## Contributing

We welcome contributions! See [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

Areas where help is especially welcome:

1. **Testing** — expand the test suite
2. **Documentation** — tutorials and API docs
3. **Axon / Dendrite** — miner and validator networking
4. **AI/ML** — subnet templates and scoring algorithms
5. **DevOps** — CI, packaging, deployment

## License

This project is licensed under the [MIT License](../LICENSE).
