# Hướng Dẫn Sử Dụng ModernTensor SDK

## Mục Lục

1. [Cài Đặt](#cài-đặt)
2. [Quick Start](#quick-start)
3. [LuxtensorClient](#luxtensorclient)
4. [WebSocket Subscriptions](#websocket-subscriptions)
5. [Caching](#caching)
6. [Tokenomics](#tokenomics)

---

## Cài Đặt

### Yêu Cầu

- Python 3.9+
- pip

### Cài Đặt từ Source

```bash
git clone https://github.com/sonson0910/moderntensor.git
cd moderntensor
pip install -e .

# Dependencies
pip install httpx websockets pydantic cryptography
```

---

## Quick Start

```python
from sdk import LuxtensorClient, connect

# Khởi tạo client
client = LuxtensorClient("http://localhost:8545")

# Lấy block number
block = client.get_block_number()
print(f"Current block: {block}")

# Lấy balance
balance = client.get_balance("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
print(f"Balance: {balance} LUX")
```

---

## LuxtensorClient

### Blockchain Queries

```python
from sdk import LuxtensorClient

client = LuxtensorClient("http://localhost:8545")

# Block operations
block_number = client.get_block_number()
block = client.get_block(block_number)
block_hash = client.get_block_hash(block_number)

# Account operations
balance = client.get_balance("0x...")
nonce = client.get_nonce("0x...")
```

### Subnet Operations

```python
# Lấy danh sách subnets
subnets = client.get_all_subnets()

# Kiểm tra subnet tồn tại
exists = client.subnet_exists(subnet_id=1)

# Lấy hyperparameters
params = client.get_subnet_hyperparameters(subnet_id=1)

# Lấy neurons trong subnet
neurons = client.get_neurons(subnet_id=1)
```

### Staking Operations

```python
# Lấy stake của coldkey-hotkey pair
stake = client.get_stake_for_coldkey_and_hotkey(
    coldkey="0x...",
    hotkey="0x..."
)

# Lấy tổng stake toàn mạng
total = client.get_total_stake()

# Lấy delegates
delegates = client.get_delegates()
```

---

## WebSocket Subscriptions

### Subscribe New Blocks

```python
import asyncio
from sdk import LuxtensorWebSocket, BlockEvent

async def on_block(event: BlockEvent):
    print(f"New block: {event.block_number}")
    print(f"  Hash: {event.block_hash}")
    print(f"  Txs: {event.tx_count}")

async def main():
    ws = LuxtensorWebSocket("ws://localhost:8546")
    ws.subscribe_blocks(on_block)
    await ws.connect()

asyncio.run(main())
```

### Available Subscriptions

| Subscription | Callback Type | Description |
|--------------|---------------|-------------|
| `subscribe_blocks` | `BlockEvent` | New blocks |
| `subscribe_pending_transactions` | `TransactionEvent` | Pending txs |
| `subscribe_account_changes` | `AccountChangeEvent` | Balance changes |
| `subscribe_stake_updates` | `Dict` | Stake changes |

---

## Caching

### Sử Dụng @cached Decorator

```python
from sdk import cached, LuxtensorClient

client = LuxtensorClient()

@cached(ttl=60)  # Cache 60 giây
async def get_cached_balance(address: str):
    return await client.get_balance(address)

# Gọi lần đầu: query RPC
balance = await get_cached_balance("0x...")

# Gọi lần sau trong 60s: trả từ cache
balance = await get_cached_balance("0x...")
```

### TTL Presets

| Loại data | TTL khuyến nghị |
|-----------|-----------------|
| Block | 3s |
| Transaction | 3600s (immutable) |
| Account balance | 10-30s |
| Subnet params | 60-300s |

---

## Tokenomics

### Token Allocation

```python
from sdk.tokenomics import TokenAllocationRPC

client = TokenAllocationRPC("http://localhost:8545")

# Execute TGE
result = client.execute_tge()

# Get allocation stats
stats = client.get_allocation_stats()
print(f"Total minted: {stats['total_minted']}")

# Claim vested tokens
claimed = client.claim_vested("0xbeneficiary...")
```

---

## Async Client

```python
from sdk import AsyncLuxtensorClient

async def main():
    async with AsyncLuxtensorClient() as client:
        block = await client.get_block_number()
        balance = await client.get_balance("0x...")

import asyncio
asyncio.run(main())
```

---

## Error Handling

```python
from sdk import LuxtensorClient

client = LuxtensorClient()

try:
    balance = client.get_balance("0xinvalid")
except Exception as e:
    print(f"Error: {e}")
```

---

## Best Practices

1. **Sử dụng caching** cho các queries thường xuyên
2. **Dùng async client** cho high-throughput applications
3. **Subscribe WebSocket** thay vì polling
4. **Handle errors** gracefully

```python
# Good practice
with LuxtensorClient() as client:
    # Use client
    pass
# Auto-closed
```

---

## Liên Hệ

- GitHub: [github.com/sonson0910/moderntensor](https://github.com/sonson0910/moderntensor)
