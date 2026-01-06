# LuxTensor Implementation Guide

## Module Conversion Guide: Python → Rust

This document provides guidance for converting each ModernTensor Python module to Rust.

---

## 1. Core Types (`luxtensor-types`)

### Python Source
```
sdk/blockchain/types (embedded in various files)
```

### Conversion Notes
- Python `bytes` → Rust `[u8; N]` for fixed-size, `Vec<u8>` for dynamic
- Python `int` → Rust `u64`, `u128` depending on size
- Python `dataclass` → Rust `struct` with `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Python exceptions → Rust `Result<T, Error>` with `thiserror`

### Examples

**Python:**
```python
@dataclass
class Block:
    height: int
    timestamp: int
    previous_hash: bytes
```

**Rust:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub height: u64,
    pub timestamp: u64,
    pub previous_hash: Hash,
}
```

---

## 2. Cryptography (`luxtensor-crypto`)

### Python Source
```
sdk/blockchain/crypto.py (~12,803 LOC)
```

### Key Conversions

| Python | Rust Library | Notes |
|--------|--------------|-------|
| `ecdsa` | `ed25519-dalek` | EdDSA is preferred |
| `hashlib.sha3_256` | `sha3::Keccak256` | Same algorithm |
| Custom Merkle | Implement in Rust | Simpler, more efficient |

### Implementation Priority
1. ✅ KeyPair generation and signing (Week 1)
2. ✅ Address derivation (Week 1)
3. ✅ Hash functions (Week 1)
4. ✅ Merkle tree basics (Week 2)
5. ⏸️ Merkle proofs (Week 2)

---

## 3. Core Blockchain (`luxtensor-core`)

### Python Source
```
sdk/blockchain/block.py        (~8,809 LOC)
sdk/blockchain/transaction.py  (~14,118 LOC)
sdk/blockchain/state.py        (~18,792 LOC)
sdk/blockchain/validation.py   (~14,030 LOC)
```

### Module Structure

```
luxtensor-core/
├── src/
│   ├── lib.rs           # Module exports
│   ├── block.rs         # Block structure
│   ├── transaction.rs   # Transaction structure
│   ├── state.rs         # State management
│   ├── validation.rs    # Block/tx validation
│   └── receipt.rs       # Transaction receipts
└── tests/
    └── integration_tests.rs
```

### Key Conversions

#### Block Structure
**Python:**
```python
@dataclass
class Block:
    version: int
    height: int
    timestamp: int
    previous_hash: bytes
    state_root: bytes
    txs_root: bytes
    transactions: List[Transaction]
    validator: bytes
    signature: bytes
```

**Rust:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub height: BlockHeight,
    pub timestamp: u64,
    pub previous_hash: Hash,
    pub state_root: Hash,
    pub txs_root: Hash,
    pub validator: Address,
    pub signature: Signature,
}
```

#### State Management
**Python:**
```python
class StateDB:
    def __init__(self, storage_path: str):
        self.db = Database(storage_path)
        self.cache = {}
    
    def get_account(self, address: bytes) -> Account:
        pass
```

**Rust:**
```rust
pub struct StateDB {
    db: Arc<DB>,
    cache: Arc<RwLock<HashMap<Address, Account>>>,
}

impl StateDB {
    pub fn new(path: &str) -> Result<Self> { ... }
    pub fn get_account(&self, address: &Address) -> Result<Account> { ... }
}
```

### Implementation Priority
1. Block structure (Week 3)
2. Transaction structure (Week 4)
3. State DB basics (Week 5)
4. Validation logic (Week 6)
5. Integration tests (Week 7-8)

---

## 4. Consensus (`luxtensor-consensus`)

### Python Source
```
sdk/consensus/pos.py           (~15,560 LOC)
sdk/consensus/fork_choice.py   (~11,907 LOC)
sdk/consensus/ai_validation.py (~10,576 LOC)
```

### Module Structure

```
luxtensor-consensus/
├── src/
│   ├── lib.rs           # Module exports
│   ├── pos.rs           # PoS implementation
│   ├── validator.rs     # Validator management
│   ├── fork_choice.rs   # Fork choice rule
│   ├── rewards.rs       # Reward distribution
│   └── ai_validation.rs # AI task validation
└── tests/
```

### Key Conversions

#### Validator Selection
**Python:**
```python
class ProofOfStake:
    def select_validator(self, slot: int) -> bytes:
        # VRF-based selection weighted by stake
        pass
```

**Rust:**
```rust
impl ProofOfStake {
    pub fn select_validator(&self, slot: u64) -> Result<Address> {
        let validators = self.validator_set.read().unwrap();
        let total_stake = validators.total_stake();
        let random_point = self.vrf_output(slot) % total_stake;
        
        // Select based on cumulative stake
        // ...
    }
}
```

### Implementation Priority
1. PoS basics (Week 1-2)
2. Validator set management (Week 2)
3. Reward distribution (Week 3)
4. Fork choice (Week 4)
5. AI validation integration (Week 5-6)

---

## 5. Network Layer (`luxtensor-network`)

### Python Source
```
sdk/network/p2p.py       (~21,935 LOC)
sdk/network/sync.py      (~17,768 LOC)
sdk/network/messages.py  (~10,668 LOC)
```

### Using libp2p

The Rust implementation will use `libp2p` instead of custom P2P:

**Python (custom):**
```python
class P2PNode:
    def __init__(self, listen_port: int):
        self.peers = {}
        # Custom socket implementation
```

**Rust (libp2p):**
```rust
use libp2p::{Swarm, NetworkBehaviour, floodsub, mdns};

#[derive(NetworkBehaviour)]
pub struct P2PBehaviour {
    pub floodsub: Floodsub,
    pub mdns: Mdns,
}

pub struct P2PNode {
    swarm: Swarm<P2PBehaviour>,
}
```

### Benefits of libp2p
- ✅ Battle-tested (IPFS, Polkadot)
- ✅ NAT traversal built-in
- ✅ Peer discovery automatic
- ✅ Multiple transport protocols
- ✅ Security (noise, TLS)

### Implementation Priority
1. Basic libp2p setup (Week 1)
2. Message protocol (Week 2)
3. Block propagation (Week 3)
4. Sync mechanism (Week 4)
5. Peer management (Week 5-6)

---

## 6. Storage Layer (`luxtensor-storage`)

### Python Source
```
sdk/storage/ (minimal implementation)
```

### Using RocksDB

**Python (custom):**
```python
class BlockchainDB:
    def __init__(self, data_dir: str):
        self.blocks_db = LevelDB(f"{data_dir}/blocks")
```

**Rust (RocksDB):**
```rust
use rocksdb::{DB, ColumnFamilyDescriptor};

pub struct BlockchainDB {
    db: Arc<DB>,
}

impl BlockchainDB {
    pub fn new(path: &str) -> Result<Self> {
        let cfs = vec![
            ColumnFamilyDescriptor::new("blocks", Options::default()),
            ColumnFamilyDescriptor::new("transactions", Options::default()),
        ];
        let db = DB::open_cf_descriptors(&opts, path, cfs)?;
        Ok(Self { db: Arc::new(db) })
    }
}
```

### Implementation Priority
1. RocksDB setup with column families (Week 1)
2. Block storage (Week 2)
3. Transaction indexing (Week 2)
4. State storage (Week 3)
5. Query optimization (Week 4)

---

## 7. API Layer (`luxtensor-api`)

### Python Source
```
sdk/api/ (~1,200 LOC)
```

### Using Modern Web Frameworks

**Python (FastAPI):**
```python
from fastapi import FastAPI
app = FastAPI()

@app.post("/")
async def handle_rpc(payload: dict):
    # Handle JSON-RPC
```

**Rust (axum):**
```rust
use axum::{routing::post, Router, Json};

async fn handle_rpc(Json(payload): Json<Value>) -> Json<Value> {
    // Handle JSON-RPC
}

let app = Router::new().route("/", post(handle_rpc));
```

### Implementation Priority
1. JSON-RPC server (Week 1-2)
2. Standard ETH methods (Week 2)
3. Custom LuxTensor methods (Week 2)
4. GraphQL API (Week 3)
5. WebSocket support (Week 4)

---

## 8. Testing Strategy

### Test Pyramid

```
         /\
        /  \        E2E Tests (5%)
       /----\
      /      \      Integration Tests (15%)
     /--------\
    /          \    Unit Tests (80%)
   /------------\
```

### Test Coverage by Module

| Module | Unit Tests | Integration Tests | Target Coverage |
|--------|-----------|-------------------|-----------------|
| types | ✅ Yes | - | > 90% |
| crypto | ✅ Yes | - | > 95% |
| core | ✅ Yes | ✅ Yes | > 85% |
| consensus | ✅ Yes | ✅ Yes | > 80% |
| network | ✅ Yes | ✅ Yes | > 75% |
| storage | ✅ Yes | - | > 85% |
| api | ✅ Yes | ✅ Yes | > 80% |

### Property-Based Testing

Use `proptest` for complex logic:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_block_hash_deterministic(
        height in 0u64..1000000,
        timestamp in 0u64..i64::MAX as u64
    ) {
        let block1 = create_block(height, timestamp);
        let block2 = create_block(height, timestamp);
        prop_assert_eq!(block1.hash(), block2.hash());
    }
}
```

---

## 9. Performance Optimization

### Benchmarking

Use `criterion` for benchmarks:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_block_hash(c: &mut Criterion) {
    let block = create_test_block();
    c.bench_function("block_hash", |b| {
        b.iter(|| black_box(block.hash()))
    });
}

criterion_group!(benches, benchmark_block_hash);
criterion_main!(benches);
```

### Optimization Checklist

- [ ] Profile with `cargo flamegraph`
- [ ] Use `parking_lot::RwLock` instead of `std::sync::RwLock`
- [ ] Use `dashmap` for concurrent HashMap
- [ ] Enable LTO in release builds
- [ ] Use `#[inline]` for hot functions
- [ ] Minimize allocations
- [ ] Use `smallvec` for small vectors
- [ ] Pool database connections

---

## 10. Migration Checklist

### Per Module

- [ ] Understand Python implementation
- [ ] Design Rust structure
- [ ] Implement core types
- [ ] Write unit tests
- [ ] Implement functionality
- [ ] Write integration tests
- [ ] Benchmark performance
- [ ] Document API
- [ ] Code review
- [ ] Merge to main

### Overall Progress

- [x] Phase 0: Setup (Week 1-3)
- [ ] Phase 1: Core (Week 4-11)
- [ ] Phase 2: Consensus (Week 12-17)
- [ ] Phase 3: Network (Week 18-23)
- [ ] Phase 4: Storage (Week 24-27)
- [ ] Phase 5: API (Week 28-31)
- [ ] Phase 6: Node/CLI (Week 32-35)
- [ ] Phase 7: Testing (Week 36-41)
- [ ] Phase 8: Docs (Week 42-44)
- [ ] Phase 9: Launch (Week 45-50)

---

## Resources

### Rust Learning
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Async Book](https://rust-lang.github.io/async-book/)

### Blockchain in Rust
- [Substrate](https://substrate.io/)
- [Solana](https://github.com/solana-labs/solana)
- [Near Protocol](https://github.com/near/nearcore)

### Libraries
- [tokio](https://tokio.rs/) - Async runtime
- [libp2p](https://github.com/libp2p/rust-libp2p) - P2P networking
- [rocksdb](https://docs.rs/rocksdb/) - Database
- [axum](https://github.com/tokio-rs/axum) - Web framework

---

**Last Updated:** January 6, 2026
