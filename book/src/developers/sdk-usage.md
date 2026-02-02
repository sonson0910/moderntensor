# SDK Usage

ModernTensor provides a Python SDK for interacting with the blockchain.

## Installation

```bash
pip install moderntensor-sdk
```

## Initializing Client

```python
from moderntensor.sdk import LuxtensorClient

# Connect to local node
client = LuxtensorClient("http://localhost:8545")

# Check connection
print(f"Connected: {client.is_connected()}")
print(f"Chain ID: {client.eth.chain_id}")
```

## Sending Transactions

```python
from moderntensor.sdk import Keypair

# Load wallet
keypair = Keypair.from_private_key("0x...")

# Send MDT
tx_hash = client.transfer(
    to="0xRecipientAddress...",
    amount=10.5, # MDT
    keypair=keypair
)
print(f"Transaction sent: {tx_hash}")
```

## Interacting with AI Layer

```python
# Submit AI Request
request_id = client.ai.submit_request(
    model_hash="0x...",
    input_data=b"Generate an image of a futuristic city"
)

# Wait for result
result = client.ai.wait_for_result(request_id)
print(f"Result: {result}")
```
