# Luxtensor Indexer

Blockchain indexer service for the Luxtensor network. Indexes blocks, transactions, token transfers, and stake events into PostgreSQL for fast querying.

## Features

- 🔗 Real-time block indexing via WebSocket
- 💾 PostgreSQL storage with automatic migrations
- 🚀 HTTP API for querying indexed data
- 📊 Transaction history by address
- 💸 Token transfer tracking
- ⚡ Stake event history

## Quick Start

### Prerequisites

- Rust 1.75+
- PostgreSQL 14+
- Luxtensor Node running

### Setup

```bash
# Create database
createdb luxtensor_indexer

# Set environment variables
export DATABASE_URL=postgres://postgres:password@localhost/luxtensor_indexer
export NODE_WS_URL=ws://localhost:8546
export GRAPHQL_BIND=0.0.0.0:4000

# Build
cargo build --release

# Run
./target/release/luxtensor-indexer
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check & sync status |
| `/blocks` | GET | Latest indexed block |
| `/query` | POST | Query transactions, transfers, stakes |

### Example Queries

```bash
# Health check
curl http://localhost:4000/health

# Get latest block
curl http://localhost:4000/blocks

# Query transactions
curl -X POST http://localhost:4000/query \
  -H "Content-Type: application/json" \
  -d '{"type": "transactions", "address": "0x...", "limit": 50}'
```

## SDK Client

```python
from sdk import IndexerClient

client = IndexerClient("http://localhost:4000")

# Get sync status
status = client.get_sync_status()
print(f"Last block: {status.last_indexed_block}")

# Get transactions
txs = client.get_transactions("0x...")
```

## Project Structure

```
indexer/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs          # Main indexer struct
    ├── main.rs         # Entry point
    ├── config.rs       # Configuration
    ├── error.rs        # Error types
    ├── models.rs       # Data models
    ├── storage.rs      # PostgreSQL operations
    ├── listener.rs     # WebSocket block listener
    ├── decoder.rs      # Transaction decoder
    └── graphql.rs      # HTTP API server
```

## License

MIT
