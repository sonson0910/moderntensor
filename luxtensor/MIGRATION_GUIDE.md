# LuxTensor Migration Guide

## 📖 Hướng Dẫn Chuyển Đổi từ Python sang Rust

### 1. Setup Môi Trường

#### Cài Đặt Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

#### Verify Installation
```bash
rustc --version
cargo --version
```

#### Cài Đặt Development Tools
```bash
# Code formatter
rustup component add rustfmt

# Linter
rustup component add clippy

# Additional tools
cargo install cargo-edit      # Manage dependencies
cargo install cargo-audit     # Security auditing
cargo install cargo-watch     # Auto-rebuild on changes
```

### 2. Build Project

```bash
cd luxtensor

# Build all modules
cargo build

# Build in release mode (optimized)
cargo build --release

# Build specific module
cargo build -p luxtensor-core
```

### 3. Run Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_transaction_creation

# Run tests for specific module
cargo test -p luxtensor-core
```

### 4. Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Check for security issues
cargo audit
```

### 5. Development Workflow

#### Watch Mode (Auto-rebuild)
```bash
cargo watch -x build -x test
```

#### Generate Documentation
```bash
cargo doc --open
```

#### Benchmarking
```bash
cargo bench
```

### 6. Migration Strategy

#### Phase 1: Core Primitives (Weeks 1-8)
1. **Crypto Module** (`core/src/crypto.rs`)
   - Migrate from `sdk/blockchain/crypto.py`
   - Use `secp256k1` crate for ECDSA
   - Implement Merkle tree

2. **Transaction Module** (`core/src/transaction.rs`)
   - Migrate from `sdk/blockchain/transaction.py`
   - Implement signing/verification
   - Add serialization

3. **Block Module** (`core/src/block.rs`)
   - Migrate from `sdk/blockchain/block.py`
   - Add header and body structures
   - Implement hashing

4. **State Module** (`core/src/state.rs`)
   - Migrate from `sdk/blockchain/state.py`
   - Implement account model
   - Add StateDB with cache

#### Phase 2: Consensus (Weeks 9-16)
- Migrate `sdk/consensus/pos.py` to `consensus/src/pos.rs`
- Migrate `sdk/consensus/fork_choice.py` to `consensus/src/fork_choice.rs`
- Implement validator management

#### Phase 3: Network (Weeks 17-20)
- Migrate `sdk/network/p2p.py` using `libp2p`
- Implement chain synchronization
- Add gossip protocol

#### Phase 4: Storage (Weeks 21-24)
- Implement RocksDB integration
- Add state storage
- Implement indexing

#### Phase 5: RPC/API (Weeks 25-28)
- Implement JSON-RPC server
- Add standard RPC methods
- Optional: GraphQL API

#### Phase 6: Full Node (Weeks 29-32)
- Integrate all components
- Add configuration management
- Implement CLI

### 7. Python to Rust Translation Patterns

#### Error Handling
```python
# Python
def process_block(block):
    if not block.valid():
        raise ValueError("Invalid block")
    return block.hash()
```

```rust
// Rust
fn process_block(block: &Block) -> Result<Hash, CoreError> {
    if !block.valid() {
        return Err(CoreError::InvalidBlock("Invalid block".to_string()));
    }
    Ok(block.hash())
}
```

#### Classes to Structs
```python
# Python
class Transaction:
    def __init__(self, from_addr, to_addr, value):
        self.from_addr = from_addr
        self.to_addr = to_addr
        self.value = value
    
    def hash(self):
        return sha256(serialize(self))
```

```rust
// Rust
#[derive(Serialize, Deserialize)]
struct Transaction {
    from_addr: Address,
    to_addr: Address,
    value: Balance,
}

impl Transaction {
    fn hash(&self) -> Hash {
        let serialized = bincode::serialize(self).unwrap();
        sha256(&serialized)
    }
}
```

#### Async Operations
```python
# Python
async def sync_chain(peer):
    blocks = await peer.get_blocks()
    for block in blocks:
        await process_block(block)
```

```rust
// Rust
async fn sync_chain(peer: &Peer) -> Result<()> {
    let blocks = peer.get_blocks().await?;
    for block in blocks {
        process_block(&block).await?;
    }
    Ok(())
}
```

### 8. Common Rust Patterns for Blockchain

#### Using Result for Error Handling
```rust
type Result<T> = std::result::Result<T, CoreError>;

fn validate_transaction(tx: &Transaction) -> Result<()> {
    if tx.value == 0 {
        return Err(CoreError::InvalidTransaction("Zero value".into()));
    }
    Ok(())
}
```

#### Builder Pattern
```rust
let transaction = Transaction::builder()
    .from([1u8; 20])
    .to([2u8; 20])
    .value(1000)
    .gas_limit(21000)
    .build()?;
```

#### Trait Implementation
```rust
trait Hashable {
    fn hash(&self) -> Hash;
}

impl Hashable for Transaction {
    fn hash(&self) -> Hash {
        // Implementation
    }
}
```

### 9. Testing Patterns

#### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_hash() {
        let tx = Transaction::new(/* ... */);
        let hash = tx.hash();
        assert_eq!(hash.len(), 32);
    }
}
```

#### Property-Based Tests
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_transaction_roundtrip(
        nonce in 0u64..1000,
        value in 0u128..1_000_000
    ) {
        let tx = Transaction::new(nonce, value, /* ... */);
        let serialized = bincode::serialize(&tx).unwrap();
        let deserialized: Transaction = bincode::deserialize(&serialized).unwrap();
        assert_eq!(tx, deserialized);
    }
}
```

### 10. Performance Tips

#### Use References to Avoid Copying
```rust
// Bad: Copies the block
fn process(block: Block) { }

// Good: Borrows the block
fn process(block: &Block) { }
```

#### Use `Arc` for Shared Data
```rust
use std::sync::Arc;

let block = Arc::new(Block::new(/* ... */));
let block_clone = Arc::clone(&block);
// Both point to same data, no deep copy
```

#### Async for I/O Operations
```rust
#[tokio::main]
async fn main() {
    let blocks = fetch_blocks().await;
    tokio::spawn(async move {
        process_blocks(blocks).await
    });
}
```

### 11. Common Pitfalls

#### Ownership Issues
```rust
// Problem: block moved here
let hash1 = block.hash(); // block moved
let hash2 = block.hash(); // Error: use of moved value

// Solution: Use references
let hash1 = block.hash(); // Implement hash(&self) not hash(self)
let hash2 = block.hash(); // Works!
```

#### Async Function Colors
```rust
// Can't call async from sync
fn sync_function() {
    // Error: Can't await here
    let result = async_function().await;
}

// Solution: Use tokio::block_on or make function async
async fn async_function_wrapper() {
    let result = async_function().await;
}
```

### 12. Resources

#### Learning Rust
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Rustlings](https://github.com/rust-lang/rustlings) - Interactive exercises

#### Blockchain in Rust
- [Substrate Developer Hub](https://docs.substrate.io/)
- [Ethereum Rust Client](https://github.com/paradigmxyz/reth)
- [Rust Blockchain Tutorial](https://blog.logrocket.com/how-to-build-a-blockchain-in-rust/)

#### Async Programming
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Async Book](https://rust-lang.github.io/async-book/)

#### Cryptography
- [RustCrypto](https://github.com/RustCrypto)
- [Dalek Cryptography](https://github.com/dalek-cryptography)

### 13. Next Steps

1. **Week 1**: Complete crypto module migration
2. **Week 2**: Finish transaction and block modules
3. **Week 3**: Implement state management
4. **Week 4**: Add validation layer
5. **Week 5-8**: Comprehensive testing and optimization

### 14. Getting Help

- **Rust Community**: https://users.rust-lang.org/
- **Discord**: Rust Programming Language Discord
- **Reddit**: r/rust
- **Stack Overflow**: Tag `rust`

---

**Happy Coding! 🦀**
