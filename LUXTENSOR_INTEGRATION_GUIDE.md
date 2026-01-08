# Luxtensor Integration Guide

**How to use Luxtensor blockchain from Python SDK**

## Overview

Luxtensor is ModernTensor's custom Layer 1 blockchain written in Rust. The Python SDK interacts with Luxtensor via JSON-RPC, similar to how web3.py interacts with Ethereum.

## Architecture

```
┌─────────────────────────────────────┐
│   Python SDK (/sdk)                 │
│   - LuxtensorClient (JSON-RPC)      │
│   - AI/ML Framework                 │
│   - CLI Tools                       │
│   - Axon/Dendrite                   │
└──────────────┬──────────────────────┘
               │ JSON-RPC
               ↓
┌─────────────────────────────────────┐
│   Luxtensor Blockchain (/luxtensor) │
│   - Rust Implementation             │
│   - RPC Server (Port 9944)          │
│   - Consensus, Storage, P2P         │
└─────────────────────────────────────┘
```

## Luxtensor Directory Structure

```
/luxtensor/
├── Cargo.toml                    # Workspace configuration
├── crates/
│   ├── luxtensor-core/          # ✅ Block, Transaction, State, Account
│   ├── luxtensor-crypto/        # ✅ Keccak256, secp256k1, Merkle
│   ├── luxtensor-consensus/     # ⏳ PoS consensus (Phase 2)
│   ├── luxtensor-network/       # ⏳ P2P networking (Phase 3)
│   ├── luxtensor-storage/       # ✅ RocksDB, Merkle Patricia Trie
│   ├── luxtensor-rpc/          # ✅ JSON-RPC API server
│   ├── luxtensor-contracts/     # Smart contracts
│   ├── luxtensor-node/          # Full node binary
│   ├── luxtensor-cli/           # Rust CLI
│   └── luxtensor-tests/         # Integration tests
└── examples/                     # Example usage
```

**Legend:**
- ✅ = Implemented and working
- ⏳ = In progress
- 🔜 = Planned

## Using Luxtensor from Python

### 1. Start Luxtensor Node

```bash
# Build Luxtensor
cd luxtensor
cargo build --release

# Run node
./target/release/luxtensor-node

# Node will start on http://localhost:9944
```

### 2. Connect from Python

```python
from sdk.luxtensor_client import LuxtensorClient

# Connect to local node
client = LuxtensorClient("http://localhost:9944")

# Check connection
if client.is_connected():
    print("Connected to Luxtensor!")

# Get current block
block_number = client.get_block_number()
print(f"Current block: {block_number}")
```

### 3. Query Account Balance

```python
# Get account balance
address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
balance = client.get_balance(address)
print(f"Balance: {balance}")

# Get account nonce
nonce = client.get_nonce(address)
print(f"Nonce: {nonce}")
```

### 4. Submit Transaction

```python
from sdk.transactions import create_transfer_transaction

# Create transaction
tx = create_transfer_transaction(
    from_address="0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
    to_address="0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd",
    amount=1000000000,  # 1 LUX (assuming 9 decimals)
    nonce=nonce,
    private_key=private_key  # Your private key
)

# Submit to blockchain
tx_hash = client.submit_transaction(tx)
print(f"Transaction hash: {tx_hash}")

# Wait for confirmation
receipt = client.wait_for_transaction(tx_hash, timeout=30)
print(f"Transaction confirmed in block {receipt['blockNumber']}")
```

### 5. Query Blockchain State

```python
# Get block by number
block = client.get_block(block_number)
print(f"Block hash: {block['hash']}")
print(f"Transactions: {len(block['transactions'])}")

# Get transaction by hash
tx = client.get_transaction(tx_hash)
print(f"From: {tx['from']}")
print(f"To: {tx['to']}")
print(f"Value: {tx['value']}")

# Get transaction receipt
receipt = client.get_transaction_receipt(tx_hash)
print(f"Status: {receipt['status']}")
print(f"Gas used: {receipt['gasUsed']}")
```

### 6. Async Usage

```python
from sdk.async_luxtensor_client import AsyncLuxtensorClient
import asyncio

async def main():
    # Connect asynchronously
    client = AsyncLuxtensorClient("http://localhost:9944")
    
    # Parallel queries
    block, balance = await asyncio.gather(
        client.get_block_number(),
        client.get_balance(address)
    )
    
    print(f"Block: {block}, Balance: {balance}")

asyncio.run(main())
```

## Luxtensor RPC Methods

The Luxtensor RPC server implements these methods (similar to Ethereum JSON-RPC):

### Block Methods
- `luxtensor_blockNumber` - Get current block number
- `luxtensor_getBlockByNumber` - Get block by number
- `luxtensor_getBlockByHash` - Get block by hash

### Transaction Methods
- `luxtensor_sendTransaction` - Submit signed transaction
- `luxtensor_getTransactionByHash` - Get transaction details
- `luxtensor_getTransactionReceipt` - Get transaction receipt

### Account Methods
- `luxtensor_getBalance` - Get account balance
- `luxtensor_getTransactionCount` - Get account nonce
- `luxtensor_getCode` - Get contract code

### State Methods
- `luxtensor_call` - Execute call without creating transaction
- `luxtensor_estimateGas` - Estimate gas for transaction

### Network Methods
- `net_version` - Get network ID
- `net_peerCount` - Get peer count
- `net_listening` - Check if node is listening

### AI/ML Specific (Custom Methods)
- `luxtensor_submitAITask` - Submit AI validation task
- `luxtensor_getAITaskResult` - Get AI task result
- `luxtensor_getNeuronInfo` - Get neuron information
- `luxtensor_getSubnetInfo` - Get subnet information

## Smart Contracts

Luxtensor uses Rust smart contracts (not Plutus or Solidity).

### Deploying Contracts

```python
# Compile Rust contract to WASM
# (Done in Rust workspace)

# Deploy from Python
contract_bytecode = load_wasm_file("contract.wasm")
deployment_tx = create_contract_deployment(
    bytecode=contract_bytecode,
    constructor_args=[arg1, arg2],
    from_address=deployer_address,
    nonce=nonce
)

contract_address = client.deploy_contract(deployment_tx)
print(f"Contract deployed at: {contract_address}")
```

### Calling Contracts

```python
# Call contract method
result = client.call_contract(
    contract_address=contract_address,
    method="get_value",
    args=[],
    from_address=caller_address
)

# Send transaction to contract
tx = create_contract_call(
    contract_address=contract_address,
    method="set_value",
    args=[42],
    from_address=sender_address,
    nonce=nonce
)

tx_hash = client.submit_transaction(tx)
```

## Network Configuration

### Testnet

```python
client = LuxtensorClient(
    url="http://testnet.luxtensor.io:9944",
    network="testnet"
)
```

### Mainnet

```python
client = LuxtensorClient(
    url="http://mainnet.luxtensor.io:9944",
    network="mainnet"
)
```

### Local Development

```python
client = LuxtensorClient(
    url="http://localhost:9944",
    network="devnet"
)
```

## Differences from Cardano

| Feature | Cardano (Old) | Luxtensor (New) |
|---------|--------------|----------------|
| **Transaction Model** | UTXO-based | Account-based |
| **API** | BlockFrost REST API | JSON-RPC |
| **Smart Contracts** | Plutus (Haskell) | Rust/WASM |
| **Serialization** | CBOR | JSON |
| **Addresses** | Bech32 | 0x... (Ethereum-style) |
| **State** | UTxO set | Account balances |
| **Python Client** | pycardano | LuxtensorClient |

## Best Practices

1. **Always check connection** before making requests:
   ```python
   if not client.is_connected():
       raise ConnectionError("Not connected to Luxtensor")
   ```

2. **Use async client** for high-throughput applications:
   ```python
   async with AsyncLuxtensorClient(url) as client:
       results = await asyncio.gather(*tasks)
   ```

3. **Handle errors gracefully**:
   ```python
   try:
       tx_hash = client.submit_transaction(tx)
   except InsufficientBalanceError:
       print("Not enough balance")
   except InvalidNonceError:
       print("Invalid nonce")
   ```

4. **Cache frequently accessed data**:
   ```python
   # Cache chain ID and network config
   chain_id = client.get_chain_id()  # Cache this
   
   # Reuse for multiple transactions
   for tx in transactions:
       tx.chain_id = chain_id
   ```

5. **Use connection pooling** for production:
   ```python
   client = LuxtensorClient(
       url="http://localhost:9944",
       pool_connections=10,
       pool_maxsize=20
   )
   ```

## Troubleshooting

### Node not running

```bash
# Check if Luxtensor node is running
curl http://localhost:9944

# Start node if not running
cd luxtensor
./target/release/luxtensor-node
```

### Connection errors

```python
# Verify URL and port
client = LuxtensorClient("http://localhost:9944")
if not client.is_connected():
    print("Cannot connect to Luxtensor node")
```

### Transaction failures

```python
# Check account has sufficient balance
balance = client.get_balance(address)
print(f"Balance: {balance}")

# Check nonce is correct
nonce = client.get_nonce(address)
print(f"Current nonce: {nonce}")
```

## Resources

- **Luxtensor Documentation**: `/luxtensor/README.md`
- **SDK Documentation**: `/sdk/README.md`
- **Examples**: `/luxtensor/examples/`
- **RPC Specification**: `/luxtensor/crates/luxtensor-rpc/README.md`

## Migration from Cardano

If you're migrating code from Cardano/pycardano:

1. Read `CARDANO_DEPRECATION.md` for full migration guide
2. Replace `BlockFrostChainContext` with `LuxtensorClient`
3. Replace UTXO queries with account balance queries
4. Replace Plutus contracts with Rust contracts
5. Replace CBOR serialization with JSON

See examples in `/examples/migration/` directory.

---

**Status:** Luxtensor Phase 1 complete, actively developing Phase 2-9  
**Python SDK:** Actively maintained, full RPC integration  
**Production Ready:** Testnet available, mainnet planned
