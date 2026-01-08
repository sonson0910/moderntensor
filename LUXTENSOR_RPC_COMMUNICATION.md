# Luxtensor RPC Communication Guide

**Date:** 2026-01-08  
**Status:** ✅ Working

## Overview

The Python SDK now properly communicates with the Luxtensor blockchain via JSON-RPC over HTTP.

## Architecture

```
┌─────────────────────────────────────────────┐
│   Python SDK (Offchain)                     │
│   /sdk/luxtensor_client.py                  │
│   - LuxtensorClient                         │
│   - Makes HTTP POST requests                │
│   - JSON-RPC 2.0 protocol                   │
└──────────────┬──────────────────────────────┘
               │ HTTP POST :9944
               │ Content-Type: application/json
               ↓
┌─────────────────────────────────────────────┐
│   Luxtensor RPC Server (Rust)               │
│   /luxtensor/crates/luxtensor-rpc/          │
│   - jsonrpc_http_server                     │
│   - Handles eth_* methods                   │
│   - Handles lux_* methods                   │
└──────────────┬──────────────────────────────┘
               │ Read/Write
               ↓
┌─────────────────────────────────────────────┐
│   Luxtensor Blockchain Core (Rust)          │
│   - StateDB (account states)                │
│   - BlockchainDB (blocks, transactions)     │
│   - Consensus module                        │
└─────────────────────────────────────────────┘
```

## Implemented RPC Methods

### Blockchain Queries (eth_*)

| Method | Python Client | Luxtensor Server | Description |
|--------|--------------|------------------|-------------|
| `eth_blockNumber` | `get_block_number()` | ✅ Implemented | Get current block height |
| `eth_getBlockByNumber` | `get_block(num)` | ✅ Implemented | Get block by number |
| `eth_getBlockByHash` | - | ✅ Implemented | Get block by hash |
| `eth_getTransactionByHash` | `get_transaction(hash)` | ✅ Implemented | Get transaction details |

### Account Queries (eth_*)

| Method | Python Client | Luxtensor Server | Description |
|--------|--------------|------------------|-------------|
| `eth_getBalance` | `get_balance(addr)` | ✅ Implemented | Get account balance |
| `eth_getTransactionCount` | `get_nonce(addr)` | ✅ Implemented | Get account nonce |

### Transaction Submission (eth_*)

| Method | Python Client | Luxtensor Server | Description |
|--------|--------------|------------------|-------------|
| `eth_sendRawTransaction` | `submit_transaction(tx)` | ✅ Implemented | Submit signed transaction |

### AI/ML Methods (lux_*)

| Method | Python Client | Luxtensor Server | Description |
|--------|--------------|------------------|-------------|
| `lux_submitAITask` | `submit_ai_task(data)` | ✅ Implemented | Submit AI computation task |
| `lux_getAIResult` | `get_ai_result(task_id)` | ✅ Implemented | Get AI task result |
| `lux_getValidatorStatus` | `get_validator_status(addr)` | ✅ Implemented | Get validator information |

## Example Usage

### Start Luxtensor Node

```bash
cd /home/runner/work/moderntensor/moderntensor/luxtensor
cargo build --release
./target/release/luxtensor-node --rpc-port 9944
```

### Python Client

```python
from sdk.luxtensor_client import LuxtensorClient

# Connect to Luxtensor
client = LuxtensorClient("http://localhost:9944")

# Check connection
if client.is_connected():
    print("✓ Connected to Luxtensor")

# Get block number
block_num = client.get_block_number()
print(f"Current block: {block_num}")

# Get account balance
address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
balance = client.get_balance(address)
print(f"Balance: {balance} wei")

# Get account nonce
nonce = client.get_nonce(address)
print(f"Nonce: {nonce}")

# Get block data
block = client.get_block(block_num)
print(f"Block hash: {block.get('hash')}")
print(f"Transactions: {len(block.get('transactions', []))}")
```

### Submit AI Task

```python
task_data = {
    "model_hash": "0x1234...",
    "input_data": "0xabcd...",
    "requester": "0x742d35...",
    "reward": "0x2386f26fc10000"  # 10 ETH in hex
}

task_id = client.submit_ai_task(task_data)
print(f"Task submitted: {task_id}")

# Get result (may be None if not ready)
result = client.get_ai_result(task_id)
if result:
    print(f"Result: {result}")
```

### Submit Transaction

```python
# Sign transaction (using your private key)
signed_tx = "0x..." # RLP-encoded signed transaction

# Submit to blockchain
result = client.submit_transaction(signed_tx)
print(f"TX Hash: {result.tx_hash}")
print(f"Status: {result.status}")
```

## Data Formats

### Addresses
- Format: `0x` + 40 hex characters
- Example: `0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb`

### Block Numbers
- Python: integer (e.g., `12345`)
- RPC: hex string (e.g., `"0x3039"`)
- Special: `"latest"` for most recent block

### Balances & Values
- Python: integer in wei (smallest unit)
- RPC: hex string (e.g., `"0x2386f26fc10000"` = 10 ETH)

### Hashes
- Format: `0x` + 64 hex characters (32 bytes)
- Example: `0x1234567890abcdef...` (transaction/block hash)

## Not Yet Implemented

These methods will raise `NotImplementedError` until the Luxtensor RPC server implements them:

### Subnet Management
- `get_subnet_info(subnet_id)` - Get subnet metadata
- `get_all_subnets()` - List all subnets

### Neuron Management
- `get_neuron(subnet_id, uid)` - Get neuron details
- `get_neurons(subnet_id)` - List neurons in subnet
- `get_active_neurons(subnet_id)` - Get active neuron UIDs

### Weight Operations
- `get_weights(subnet_id, uid)` - Get neuron weights
- `set_weights(subnet_id, uids, weights)` - Set weights

### Staking
- `get_stake(address)` - Get staked amount
- `get_total_stake()` - Get total network stake
- `stake(amount)` - Stake tokens
- `unstake(amount)` - Unstake tokens

These features will be added as the Luxtensor consensus and subnet modules are developed.

## Error Handling

```python
try:
    balance = client.get_balance(address)
except Exception as e:
    if "Connection refused" in str(e):
        print("Luxtensor node is not running")
    elif "RPC error" in str(e):
        print(f"RPC call failed: {e}")
    else:
        print(f"Unexpected error: {e}")
```

## Testing

```python
# Test connection
assert client.is_connected(), "Cannot connect to Luxtensor"

# Test basic queries
block_num = client.get_block_number()
assert block_num >= 0, "Invalid block number"

block = client.get_block(block_num)
assert block is not None, "Block not found"
assert "hash" in block, "Block missing hash"

print("✓ All tests passed")
```

## Troubleshooting

### "Connection refused"
- Luxtensor node is not running
- Wrong port or URL
- Firewall blocking connection

**Solution:**
```bash
# Start Luxtensor node
cd luxtensor
./target/release/luxtensor-node --rpc-port 9944
```

### "RPC error: method not found"
- Method not implemented in Luxtensor RPC server yet
- Typo in method name

**Solution:** Check available methods in `/luxtensor/crates/luxtensor-rpc/src/server.rs`

### "Invalid params"
- Wrong parameter format (should be hex string with 0x prefix)
- Missing required parameters

**Solution:** Check method signature and parameter format

## References

- **Luxtensor RPC Server:** `/luxtensor/crates/luxtensor-rpc/src/server.rs`
- **Python Client:** `/sdk/luxtensor_client.py`
- **RPC Types:** `/luxtensor/crates/luxtensor-rpc/src/types.rs`
- **JSON-RPC 2.0 Spec:** https://www.jsonrpc.org/specification

---

**Status:** ✅ Working and tested  
**Last Updated:** 2026-01-08  
**Commit:** 60eebfe
