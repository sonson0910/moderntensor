# ModernTensor SDK - Quick Start Guide

> **Complete examples for common use cases**

## Prerequisites

```bash
# Clone repository
git clone https://github.com/moderntensor/moderntensor.git
cd moderntensor

# Install SDK
pip install -e .

# Start Luxtensor node (required)
cd luxtensor && cargo run --release
```

---

## Import Guide

```python
# ✅ After pip install (recommended)
from moderntensor.sdk import connect, LuxtensorClient

# ✅ Development mode (from repo root)
import sys
sys.path.insert(0, 'moderntensor')
from sdk import connect, LuxtensorClient
```

---

## Example 1: Connect and Query Blockchain

```python
"""
Basic blockchain queries - balances, blocks, validators.
Demonstrates: Unit conversion, error handling, connection checking.
"""
from moderntensor.sdk import (
    connect,
    to_mdt,           # Convert base units → MDT
    format_mdt,       # Format for display
    RpcError,         # Base error class
    BlockNotFoundError,
)

# Connect to local node
client = connect(url="http://localhost:8545", network="testnet")

# Verify connection
if not client.is_connected():
    print("❌ Cannot connect to Luxtensor node")
    exit(1)

print("✅ Connected to Luxtensor!")

# Get blockchain info
chain_info = client.get_chain_info()
print(f"Chain ID: {chain_info.chain_id}")
print(f"Current Block: {chain_info.block_number}")
print(f"Network: {chain_info.network}")

# Get account balance with proper unit conversion
address = "0x1234567890abcdef1234567890abcdef12345678"

try:
    balance_wei = client.get_balance(address)  # Returns base units (wei)
    balance_mdt = to_mdt(balance_wei)          # Convert to MDT

    print(f"Balance (wei): {balance_wei}")
    print(f"Balance (MDT): {balance_mdt}")
    print(f"Formatted: {format_mdt(balance_wei)}")  # "1.2345 MDT"

except RpcError as e:
    print(f"❌ RPC Error: {e.message} (code: {e.code})")

# Get active validators
validators = client.get_validators()
print(f"Active validators: {len(validators)}")
for v in validators[:5]:
    stake_mdt = to_mdt(v.stake)  # Convert stake to MDT
    print(f"  - {v.address}: {stake_mdt} MDT stake")
```

> **⚠️ Unit Conversion Important!**
> `get_balance()` returns **base units** (wei, 10^18).
> Always use `to_mdt()` before displaying to users.

---

## Example 2: Create Wallet and Send Transaction

```python
"""
Complete flow: Generate wallet → Check balance → Send transfer.
Demonstrates: Unit conversion with from_mdt(), error handling.
"""
from moderntensor.sdk import (
    connect,
    create_transfer_transaction,
    encode_transaction_for_rpc,
    to_mdt,               # Convert wei → MDT
    from_mdt,             # Convert MDT → wei
    InsufficientFundsError,
    NonceTooLowError,
    RpcError,
)
from moderntensor.sdk.keymanager import KeyManager

# Step 1: Generate new wallet
km = KeyManager()
coldkey = km.generate_coldkey(password="secure-password-123")
hotkey = km.generate_hotkey(coldkey_name=coldkey.name)

print(f"✅ Coldkey created: {coldkey.address}")
print(f"✅ Hotkey created: {hotkey.address}")

# Step 2: Connect to blockchain
client = connect("http://localhost:8545")

# Step 3: Check balance with proper unit conversion
balance_wei = client.get_balance(coldkey.address)
balance_mdt = to_mdt(balance_wei)
print(f"Balance: {balance_mdt} MDT")

# Convert minimum required to wei for comparison
min_required_wei = from_mdt(1000)  # 1000 MDT in wei
if balance_wei < min_required_wei:
    print("⚠️ Insufficient balance. Get testnet tokens from faucet:")
    print(f"   https://faucet.luxtensor.io/?address={coldkey.address}")
    exit(1)

# Step 4: Create and sign transfer transaction
nonce = client.get_nonce(coldkey.address)

# IMPORTANT: amount is in wei (base units), use from_mdt() to convert!
amount_mdt = 100  # We want to send 100 MDT
amount_wei = from_mdt(amount_mdt)  # Convert to wei

tx = create_transfer_transaction(
    from_address=coldkey.address,
    to_address="0xRecipientAddressHere",
    amount=amount_wei,  # Must be in wei!
    nonce=nonce,
    private_key=coldkey.private_key,  # Signs the transaction
    gas_price=50,
    gas_limit=21000,
)

# Step 5: Submit transaction with error handling
try:
    tx_hash = client.submit_transaction(encode_transaction_for_rpc(tx))
    print(f"✅ Transaction submitted: {tx_hash}")

    # Step 6: Wait for confirmation
    receipt = client.wait_for_transaction(tx_hash, timeout=60)
    if receipt.success:
        print(f"✅ Transaction confirmed in block {receipt.block_number}")
    else:
        print(f"❌ Transaction failed: {receipt.error}")

except InsufficientFundsError as e:
    print(f"❌ Not enough funds: need {to_mdt(e.data['need'])} MDT")
except NonceTooLowError as e:
    print(f"❌ Nonce too low. Expected: {e.data['expected']}, got: {e.data['got']}")
    # Retry with correct nonce
except RpcError as e:
    print(f"❌ RPC Error ({e.code}): {e.message}")
```

> **⚠️ Signing Pattern:**
> All state-changing operations require `private_key` parameter.
> The `create_transfer_transaction()` function signs internally.

---

## Example 3: Register as Miner on Subnet

```python
"""
Complete miner registration flow on a subnet.
"""
from moderntensor.sdk import connect
from moderntensor.sdk.keymanager import KeyManager

# Step 1: Setup wallet (use existing or create new)
km = KeyManager()

# Load existing keys
coldkey = km.load_coldkey("my-coldkey", password="my-password")
hotkey = km.load_hotkey("my-hotkey")

# Or generate new
# coldkey = km.generate_coldkey(password="secure-password")
# hotkey = km.generate_hotkey(coldkey_name=coldkey.name)

print(f"Coldkey: {coldkey.address}")
print(f"Hotkey: {hotkey.address}")

# Step 2: Connect
client = connect("http://localhost:8545")

# Step 3: Check if already registered
SUBNET_ID = 1  # Target subnet

is_registered = client.is_hotkey_registered(
    subnet_id=SUBNET_ID,
    hotkey=hotkey.address
)

if is_registered:
    uid = client.get_uid_for_hotkey(SUBNET_ID, hotkey.address)
    print(f"✅ Already registered with UID: {uid}")
else:
    # Step 4: Check registration requirements
    subnet_info = client.get_subnet_info(SUBNET_ID)
    min_stake = subnet_info.min_stake
    print(f"Minimum stake required: {min_stake} MDT")

    # Step 5: Register
    result = client.register_neuron(
        subnet_id=SUBNET_ID,
        hotkey=hotkey.address,
        coldkey=coldkey.address,
        stake=min_stake,
        private_key=coldkey.private_key,
    )

    if result.success:
        print(f"✅ Registered! UID: {result.uid}")
        print(f"   TX Hash: {result.tx_hash}")
    else:
        print(f"❌ Registration failed: {result.error}")

# Step 5: Verify registration
neuron = client.get_neuron_info(SUBNET_ID, hotkey.address)
print(f"""
Neuron Status:
  UID: {neuron.uid}
  Stake: {neuron.stake}
  Trust: {neuron.trust}
  Incentive: {neuron.incentive}
""")
```

---

## Example 4: Set Weights as Validator

```python
"""
Validator sets weights for miners in subnet.
Requires: Registered as validator with sufficient stake.
"""
from moderntensor.sdk import connect
from moderntensor.sdk.commit_reveal import CommitRevealClient

# Connect
client = connect("http://localhost:8545")
cr_client = CommitRevealClient("http://localhost:8545")

# Your validator info
SUBNET_ID = 1
VALIDATOR_HOTKEY = "0xYourValidatorHotkey"
VALIDATOR_COLDKEY = "0xYourValidatorColdkey"
PRIVATE_KEY = "your-private-key"

# Step 1: Get current neurons to score
neurons = client.get_neurons(SUBNET_ID)
print(f"Found {len(neurons)} neurons to score")

# Step 2: Calculate weights (your scoring logic here)
uids = []
weights = []

for neuron in neurons:
    uids.append(neuron.uid)

    # Example: Score based on incentive + trust
    score = (neuron.incentive * 0.6) + (neuron.trust * 0.4)
    weights.append(int(score * 65535))  # Normalize to uint16

print(f"Calculated weights for {len(uids)} neurons")

# Step 3: Commit-Reveal flow (if subnet requires it)
subnet_info = client.get_subnet_info(SUBNET_ID)

if subnet_info.commit_reveal_enabled:
    # Phase 1: Commit
    salt = cr_client.generate_salt()
    commit_hash = cr_client.compute_hash(uids, weights, salt)

    commit_result = cr_client.commit_weights(
        subnet_id=SUBNET_ID,
        commit_hash=commit_hash,
        hotkey=VALIDATOR_HOTKEY,
        private_key=PRIVATE_KEY,
    )
    print(f"✅ Committed: {commit_result.tx_hash}")

    # Wait for reveal period
    print("⏳ Waiting for reveal period...")
    cr_client.wait_for_reveal_period(SUBNET_ID)

    # Phase 2: Reveal
    reveal_result = cr_client.reveal_weights(
        subnet_id=SUBNET_ID,
        uids=uids,
        weights=weights,
        salt=salt,
        hotkey=VALIDATOR_HOTKEY,
        private_key=PRIVATE_KEY,
    )
    print(f"✅ Revealed: {reveal_result.tx_hash}")

else:
    # Direct weight setting
    result = client.set_weights(
        subnet_id=SUBNET_ID,
        uids=uids,
        weights=weights,
        hotkey=VALIDATOR_HOTKEY,
        private_key=PRIVATE_KEY,
    )
    print(f"✅ Weights set: {result.tx_hash}")

# Verify
current_weights = client.get_weights(SUBNET_ID, uid=0)
print(f"Current weights: {current_weights[:5]}...")
```

---

## Example 5: Setup Axon Server (Miner)

```python
"""
Setup an Axon server to serve AI model inference requests.
"""
from moderntensor.sdk.axon import Axon, AxonConfig
from moderntensor.sdk.synapse import Synapse
import asyncio

# Step 1: Configure Axon server
config = AxonConfig(
    host="0.0.0.0",
    port=8091,
    external_ip="your-public-ip",  # Or None for auto-detect
    max_concurrent=100,
    rate_limit=50,  # requests per minute
    enable_auth=True,
)

# Step 2: Create Axon instance
axon = Axon(config=config)

# Step 3: Define your forward function (AI inference)
async def forward_handler(synapse: Synapse) -> Synapse:
    """
    Handle inference requests from validators.

    Args:
        synapse: Request containing input data

    Returns:
        Synapse with output data
    """
    # Get input from synapse
    input_data = synapse.input_tensor

    # Your AI model inference here
    # output = your_model.predict(input_data)
    output = input_data * 2  # Placeholder

    # Set output
    synapse.output_tensor = output
    synapse.status = "success"

    return synapse

# Step 4: Attach handlers
axon.attach(
    endpoint="/forward",
    handler=forward_handler,
    methods=["POST"],
)

# Optional: Add backward handler for training
async def backward_handler(synapse: Synapse) -> Synapse:
    """Handle gradient updates for training."""
    gradients = synapse.gradients
    # Update model weights
    synapse.status = "success"
    return synapse

axon.attach("/backward", backward_handler)

# Step 5: Register API key for validators
api_key = axon.register_api_key("validator-uid-123")
print(f"API Key for validators: {api_key}")

# Step 6: Start server
print(f"🚀 Starting Axon server on {config.host}:{config.port}")
axon.run()  # Blocking
```

---

## Example 6: Async Client (High Performance)

```python
"""
Async client for high-performance batch operations.
"""
from moderntensor.sdk import async_connect
import asyncio

async def main():
    # Connect with async client
    client = await async_connect("http://localhost:8545")

    # Batch multiple queries in single round-trip
    calls = [
        ("eth_blockNumber", []),
        ("staking_getValidators", []),
        ("subnet_getAll", []),
        ("system_health", []),
    ]

    results = await client.batch_call(calls)

    block_number = int(results[0], 16)
    validators = results[1]
    subnets = results[2]
    health = results[3]

    print(f"Block: {block_number}")
    print(f"Validators: {len(validators)}")
    print(f"Subnets: {len(subnets)}")
    print(f"Health: {health}")

    # Concurrent queries for multiple accounts
    addresses = [
        "0xAddress1...",
        "0xAddress2...",
        "0xAddress3...",
    ]

    # All queries execute concurrently
    balances = await asyncio.gather(*[
        client.get_balance(addr) for addr in addresses
    ])

    for addr, balance in zip(addresses, balances):
        print(f"{addr[:10]}...: {balance} MDT")

asyncio.run(main())
```

---

## Example 7: Check Block Finality (New)

```python
"""
Using the consensus module to check block finality status.
"""
from moderntensor.sdk import connect, FastFinality, ValidatorInfo

client = connect("http://localhost:8545")

# 1. Get active validators and stake
validators_list = client.get_validators()
validator_map = {
    v.address: ValidatorInfo(address=v.address, stake=v.stake)
    for v in validators_list
}

# 2. Initialize Fast Finality tracker
ff = FastFinality(finality_threshold_percent=67, validators=validator_map)

# 3. Simulate receiving signatures (in production, these come from P2P/RPC)
block_hash = "0xabc..."
# ... add signatures ...

# 4. Check status
if ff.is_finalized(block_hash):
    print(f"✅ Block {block_hash} is finalized!")
else:
    progress = ff.get_finality_progress(block_hash)
    print(f"⏳ Finality progress: {progress}%")
```

---

## Sync vs Async Client Comparison

| Feature | `LuxtensorClient` (Sync) | `AsyncLuxtensorClient` |
| :--- | :--- | :--- |
| **Use Case** | Simple scripts, CLI tools | High-perf apps, servers |
| **Batch Calls** | ❌ Not available | ✅ `batch_call()` |
| **Concurrent Requests** | ❌ Sequential | ✅ `asyncio.gather()` |
| **Thread-safe** | ✅ Yes | N/A (single-threaded) |
| **Performance** | Good | Excellent |
| **Complexity** | Low | Medium |

**When to use Sync:**

- CLI tools
- Simple scripts
- Low request volume
- Learning/prototyping

**When to use Async:**

- Axon/Dendrite servers
- Batch processing
- Real-time applications
- High throughput needed

---

## Troubleshooting

### Connection Issues

```python
# Check if node is running
import requests
response = requests.post(
    "http://localhost:8545",
    json={"jsonrpc": "2.0", "method": "system_health", "params": [], "id": 1}
)
print(response.json())
```

### Import Errors

```python
# If "ModuleNotFoundError: No module named 'moderntensor'"
# Option 1: Install in development mode
pip install -e .

# Option 2: Add to path
import sys
sys.path.insert(0, '/path/to/moderntensor')
```

### Transaction Failures

```python
# Check nonce
nonce = client.get_nonce(address)
print(f"Current nonce: {nonce}")

# Check balance for gas
balance = client.get_balance(address)
gas_cost = gas_price * gas_limit
print(f"Need: {gas_cost}, Have: {balance}")
```

---

## Next Steps

1. **Explore API Reference:** [API_REFERENCE.md](API_REFERENCE.md)
2. **Run Examples:** `python examples/luxtensor_client_example.py`
3. **Join Discord:** [discord.gg/moderntensor](https://discord.gg/moderntensor)
4. **Read Whitepaper:** [MODERNTENSOR_WHITEPAPER_VI.md](../MODERNTENSOR_WHITEPAPER_VI.md)

---

Last updated: January 28, 2026
