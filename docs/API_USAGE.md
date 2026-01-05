# ModernTensor Phase 5: API Usage Examples

This document provides examples of how to use the ModernTensor JSON-RPC and GraphQL APIs.

## JSON-RPC API

### Starting the API Server

```python
from sdk.api.rpc import JSONRPC
from sdk.storage.blockchain_db import BlockchainDB
from sdk.blockchain.state import StateDB
from sdk.storage.indexer import MemoryIndexer
from sdk.blockchain.validation import ChainConfig
import uvicorn

# Initialize components
blockchain_db = BlockchainDB("./data/blockchain")
state_db = StateDB()
indexer = MemoryIndexer()
config = ChainConfig(chain_id=1)

# Create JSON-RPC server
rpc_server = JSONRPC(
    blockchain_db=blockchain_db,
    state_db=state_db,
    indexer=indexer,
    chain_config=config
)

# Start the server
uvicorn.run(rpc_server.app, host="0.0.0.0", port=8545)
```

### Making JSON-RPC Requests

#### Get Current Block Number
```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_blockNumber",
    "params": [],
    "id": 1
  }'
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": 1234,
  "id": 1
}
```

#### Get Block by Number
```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_getBlockByNumber",
    "params": ["latest", false],
    "id": 1
  }'
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "number": "0x4d2",
    "hash": "0x...",
    "parentHash": "0x...",
    "timestamp": "0x...",
    "transactions": ["0x...", "0x..."]
  },
  "id": 1
}
```

#### Get Account Balance
```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_getBalance",
    "params": ["0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb", "latest"],
    "id": 1
  }'
```

#### Send Raw Transaction
```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_sendRawTransaction",
    "params": ["0xf86c..."],
    "id": 1
  }'
```

#### Submit AI Task (ModernTensor-specific)
```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "mt_submitAITask",
    "params": [{
      "model_hash": "0x1234567890abcdef",
      "input_data": "image_data_base64",
      "requester": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
      "reward": 1000
    }],
    "id": 1
  }'
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": "abc123def456...",
  "id": 1
}
```

#### Get AI Task Result
```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "mt_getAIResult",
    "params": ["abc123def456..."],
    "id": 1
  }'
```

## GraphQL API (Optional)

### Starting the GraphQL API

```python
from sdk.api.graphql_api import GraphQLAPI
from sdk.storage.blockchain_db import BlockchainDB
from sdk.blockchain.state import StateDB
from sdk.storage.indexer import MemoryIndexer
from fastapi import FastAPI
import uvicorn

# Initialize components
blockchain_db = BlockchainDB("./data/blockchain")
state_db = StateDB()
indexer = MemoryIndexer()

# Create GraphQL API
graphql_api = GraphQLAPI(
    blockchain_db=blockchain_db,
    state_db=state_db,
    indexer=indexer
)

# Create FastAPI app and add GraphQL router
app = FastAPI()
if graphql_api.router:
    app.include_router(graphql_api.router, prefix="/graphql")

# Start the server
uvicorn.run(app, host="0.0.0.0", port=8080)
```

### Making GraphQL Queries

#### Query Block by Height
```graphql
query {
  block(height: 100) {
    hash
    height
    timestamp
    validator
    transactionCount
    transactions {
      hash
      from_address
      to_address
      value
    }
  }
}
```

#### Query Account Information
```graphql
query {
  account(address: "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb") {
    address
    balance
    nonce
    is_contract
    transactions(limit: 10) {
      hash
      value
      status
    }
  }
}
```

#### Query Chain Information
```graphql
query {
  chain_info {
    chain_id
    best_height
    best_hash
    total_transactions
    genesis_hash
  }
}
```

#### Query Multiple Blocks
```graphql
query {
  blocks(fromHeight: 0, toHeight: 10) {
    hash
    height
    timestamp
    transactionCount
  }
}
```

## Python Client Example

```python
import requests
import json

class ModernTensorClient:
    def __init__(self, rpc_url="http://localhost:8545"):
        self.rpc_url = rpc_url
        self.request_id = 0
    
    def _make_request(self, method, params=None):
        self.request_id += 1
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or [],
            "id": self.request_id
        }
        
        response = requests.post(
            self.rpc_url,
            headers={"Content-Type": "application/json"},
            data=json.dumps(payload)
        )
        
        result = response.json()
        
        if "error" in result:
            raise Exception(f"RPC Error: {result['error']}")
        
        return result["result"]
    
    def get_block_number(self):
        """Get current block number"""
        return int(self._make_request("eth_blockNumber"), 16)
    
    def get_block(self, block_number="latest", full_tx=False):
        """Get block by number"""
        return self._make_request(
            "eth_getBlockByNumber",
            [hex(block_number) if isinstance(block_number, int) else block_number, full_tx]
        )
    
    def get_balance(self, address, block="latest"):
        """Get account balance"""
        result = self._make_request("eth_getBalance", [address, block])
        return int(result, 16)
    
    def send_raw_transaction(self, tx_hex):
        """Send signed transaction"""
        return self._make_request("eth_sendRawTransaction", [tx_hex])
    
    def submit_ai_task(self, model_hash, input_data, requester, reward):
        """Submit AI task"""
        task_params = {
            "model_hash": model_hash,
            "input_data": input_data,
            "requester": requester,
            "reward": reward
        }
        return self._make_request("mt_submitAITask", [task_params])
    
    def get_ai_result(self, task_id):
        """Get AI task result"""
        return self._make_request("mt_getAIResult", [task_id])

# Usage example
if __name__ == "__main__":
    client = ModernTensorClient()
    
    # Get current block number
    block_num = client.get_block_number()
    print(f"Current block: {block_num}")
    
    # Get latest block
    block = client.get_block("latest")
    print(f"Latest block hash: {block['hash']}")
    
    # Get account balance
    address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
    balance = client.get_balance(address)
    print(f"Balance: {balance}")
    
    # Submit AI task
    task_id = client.submit_ai_task(
        model_hash="0x1234567890abcdef",
        input_data="test_input",
        requester=address,
        reward=1000
    )
    print(f"Task ID: {task_id}")
    
    # Get task result
    result = client.get_ai_result(task_id)
    print(f"Task status: {result['status']}")
```

## Integration with Web3.py

ModernTensor's JSON-RPC API is Ethereum-compatible, so you can use standard Ethereum tools:

```python
from web3 import Web3

# Connect to ModernTensor node
w3 = Web3(Web3.HTTPProvider('http://localhost:8545'))

# Check connection
print(f"Connected: {w3.is_connected()}")

# Get block number
print(f"Block number: {w3.eth.block_number}")

# Get block
block = w3.eth.get_block('latest')
print(f"Latest block: {block['hash'].hex()}")

# Get balance
balance = w3.eth.get_balance('0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb')
print(f"Balance: {w3.from_wei(balance, 'ether')} ETH")
```

## Notes

- The JSON-RPC API follows the Ethereum JSON-RPC specification with additional ModernTensor-specific methods prefixed with `mt_`
- GraphQL API requires the optional `strawberry-graphql[fastapi]` package
- All API methods are asynchronous and can handle concurrent requests
- Transaction pool management is automatic for pending transactions
- AI task storage is currently in-memory (will be persisted in future phases)
