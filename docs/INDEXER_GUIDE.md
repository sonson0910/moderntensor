# Hướng Dẫn Sử Dụng Luxtensor Indexer

## Mục Lục

1. [Giới Thiệu](#giới-thiệu)
2. [Cài Đặt](#cài-đặt)
3. [Chạy Indexer](#chạy-indexer)
4. [API Endpoints](#api-endpoints)
5. [SDK Client](#sdk-client)
6. [Database Schema](#database-schema)

---

## Giới Thiệu

Luxtensor Indexer là service index blockchain data vào PostgreSQL, cung cấp:

- **Lịch sử transactions** của bất kỳ address
- **Token transfers** tracking
- **Stake history** cho validators
- **Fast queries** thay vì scan toàn bộ blockchain

### Architecture

```
Luxtensor Node (WS:8546)
        ↓ Block events
Luxtensor Indexer
        ↓ Writes
PostgreSQL
        ↓ Reads
HTTP API / SDK Client
```

---

## Cài Đặt

### Yêu Cầu

- Rust 1.75+
- PostgreSQL 14+
- Luxtensor Node đang chạy

### Build từ Source

```bash
cd moderntensor/luxtensor
cargo build --release -p luxtensor-indexer
```

### Thiết Lập PostgreSQL

```bash
createdb luxtensor_indexer
```

---

## Chạy Indexer

### Environment Variables

```bash
export DATABASE_URL=postgres://postgres:password@localhost/luxtensor_indexer
export NODE_WS_URL=ws://localhost:8546
export GRAPHQL_BIND=0.0.0.0:4000
export RUST_LOG=info
```

### Chạy

```bash
# Linux/macOS
./target/release/luxtensor-indexer

# Windows
.\target\release\luxtensor-indexer.exe
```

### Output Mong Đợi

```
╔══════════════════════════════════════════════════════════╗
║           Luxtensor Indexer v0.1.0                       ║
╚══════════════════════════════════════════════════════════╝
INFO Starting indexer...
INFO Connected to PostgreSQL
INFO GraphQL server starting on 0.0.0.0:4000
INFO Connected to node WebSocket
INFO New block: 1234 (txs: 5)
```

---

## API Endpoints

### Health Check

```bash
curl http://localhost:4000/health
```

Response:

```json
{
  "status": "ok",
  "last_block": 12345,
  "syncing": true
}
```

### Get Latest Block

```bash
curl http://localhost:4000/blocks
```

### Query Data

```bash
curl -X POST http://localhost:4000/query \
  -H "Content-Type: application/json" \
  -d '{"type": "transactions", "address": "0x123...", "limit": 50}'
```

### Available Query Types

| Type | Parameters | Description |
|------|------------|-------------|
| `block` | `number` | Get block by number |
| `blocks` | `from`, `to` | Get block range |
| `transactions` | `address`, `limit`, `offset` | Transaction history |
| `transfers` | `address`, `limit`, `offset` | Token transfers |
| `stake_history` | `hotkey`, `limit` | Stake events |

---

## SDK Client

### Sync Client

```python
from sdk import IndexerClient

client = IndexerClient("http://localhost:4000")

# Check sync status
status = client.get_sync_status()
print(f"Last block: {status.last_indexed_block}")

# Get transaction history
txs = client.get_transactions("0x123...", limit=50)
for tx in txs:
    print(f"{tx.hash}: {tx.from_address} → {tx.to_address}")

# Get token transfers
transfers = client.get_transfers("0x123...")

# Get stake history
stakes = client.get_stake_history("0xhotkey...")
```

### Async Client

```python
from sdk import AsyncIndexerClient
import asyncio

async def main():
    async with AsyncIndexerClient("http://localhost:4000") as client:
        healthy = await client.health_check()
        txs = await client.get_transactions("0x123...")

asyncio.run(main())
```

### Available Methods

| Method | Return Type | Description |
|--------|-------------|-------------|
| `health_check()` | `bool` | Check indexer health |
| `get_sync_status()` | `SyncStatus` | Get sync status |
| `get_latest_block()` | `IndexedBlock` | Latest indexed block |
| `get_transactions(addr)` | `List[IndexedTransaction]` | Transaction history |
| `get_transfers(addr)` | `List[TokenTransfer]` | Token transfers |
| `get_stake_history(hotkey)` | `List[StakeEvent]` | Stake events |

---

## Database Schema

### blocks

```sql
CREATE TABLE blocks (
    number BIGINT PRIMARY KEY,
    hash VARCHAR(66) NOT NULL,
    parent_hash VARCHAR(66),
    timestamp BIGINT NOT NULL,
    tx_count INT NOT NULL
);
```

### transactions

```sql
CREATE TABLE transactions (
    hash VARCHAR(66) PRIMARY KEY,
    block_number BIGINT NOT NULL,
    from_address VARCHAR(42) NOT NULL,
    to_address VARCHAR(42),
    value VARCHAR(78) NOT NULL,
    gas_used BIGINT NOT NULL,
    status SMALLINT NOT NULL,
    tx_type VARCHAR(50) NOT NULL
);
```

### token_transfers

```sql
CREATE TABLE token_transfers (
    id BIGSERIAL PRIMARY KEY,
    tx_hash VARCHAR(66) NOT NULL,
    block_number BIGINT NOT NULL,
    from_address VARCHAR(42) NOT NULL,
    to_address VARCHAR(42) NOT NULL,
    amount VARCHAR(78) NOT NULL,
    timestamp BIGINT NOT NULL
);
```

### stakes

```sql
CREATE TABLE stakes (
    id BIGSERIAL PRIMARY KEY,
    block_number BIGINT NOT NULL,
    coldkey VARCHAR(42) NOT NULL,
    hotkey VARCHAR(42) NOT NULL,
    amount VARCHAR(78) NOT NULL,
    action VARCHAR(20) NOT NULL,
    timestamp BIGINT NOT NULL
);
```

### Indexes

```sql
CREATE INDEX idx_tx_from ON transactions(from_address);
CREATE INDEX idx_tx_to ON transactions(to_address);
CREATE INDEX idx_transfers_from ON token_transfers(from_address);
CREATE INDEX idx_stakes_hotkey ON stakes(hotkey);
```

---

## Troubleshooting

| Vấn đề | Nguyên nhân | Giải pháp |
|--------|-------------|-----------|
| Connection refused | PostgreSQL chưa chạy | `systemctl start postgresql` |
| WebSocket error | Node chưa chạy | Start luxtensor-node first |
| Slow queries | Missing indexes | Run migration scripts |

---

## Liên Hệ

- GitHub: [github.com/sonson0910/moderntensor](https://github.com/sonson0910/moderntensor)
