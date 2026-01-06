# Hướng Dẫn Sử Dụng LuxTensor

**Ngày:** 6 Tháng 1, 2026  
**Trạng thái:** ✅ Phase 1 Hoàn Thành  

---

## 📦 Đã Tạo Xong

Tôi đã tạo xong cấu trúc repository LuxTensor trong thư mục `luxtensor/` với:

### ✅ Hoàn Thành
1. **Cargo Workspace** - 8 crates đã setup
2. **Phase 1 Implementation** - Core + Crypto hoàn chỉnh
3. **17 tests** - Tất cả đều pass ✅
4. **CI/CD** - GitHub Actions workflow
5. **Documentation** - README và setup guides

### 📁 Cấu Trúc
```
luxtensor/
├── Cargo.toml                   # Workspace configuration
├── README.md                    # Documentation
├── .github/workflows/ci.yml     # CI/CD pipeline
├── crates/
│   ├── luxtensor-core/         ✅ Phase 1 Complete (8 tests)
│   ├── luxtensor-crypto/       ✅ Phase 1 Complete (9 tests)
│   ├── luxtensor-consensus/    ⏳ Phase 2 (stub)
│   ├── luxtensor-network/      ⏳ Phase 3 (stub)
│   ├── luxtensor-storage/      ⏳ Phase 4 (stub)
│   ├── luxtensor-rpc/          ⏳ Phase 5 (stub)
│   ├── luxtensor-node/         ✅ Node binary
│   └── luxtensor-cli/          ✅ CLI tool
├── tests/                       # Integration tests
├── benches/                     # Benchmarks
└── docs/                        # Documentation
```

---

## 🚀 Cách Sử Dụng

### Bước 1: Build Project

```bash
cd luxtensor

# Build tất cả crates
cargo build --release

# Hoặc build debug (nhanh hơn)
cargo build
```

### Bước 2: Run Tests

```bash
# Chạy tất cả tests
cargo test --workspace

# Chạy tests của crate cụ thể
cargo test -p luxtensor-core
cargo test -p luxtensor-crypto

# Chạy test cụ thể
cargo test test_genesis_block
```

### Bước 3: Sử Dụng CLI

```bash
# Show version
./target/release/luxtensor version

# Generate keypair mới
./target/release/luxtensor generate-key
# Output:
# Generated new keypair:
# Address: 0x1a2b3c4d5e...
# ⚠️  IMPORTANT: Save your private key securely!

# Check status
./target/release/luxtensor status
```

### Bước 4: Run Node

```bash
# Start node
./target/release/luxtensor-node

# Output:
# 🦀 LuxTensor Node v0.1.0
# High-performance Layer 1 blockchain
# 
# Status: Phase 1 - Foundation
# Components initialized:
#   ✓ Core primitives (Block, Transaction, State)
#   ✓ Cryptography (Keccak256, Blake3, secp256k1)
#   ⏳ Consensus (TODO: Phase 2)
#   ⏳ Network (TODO: Phase 3)
#   ⏳ Storage (TODO: Phase 4)
#   ⏳ RPC (TODO: Phase 5)
```

---

## 💻 Development

### Watch Mode (Auto-rebuild on changes)

```bash
# Install cargo-watch nếu chưa có
cargo install cargo-watch

# Auto-run tests khi code thay đổi
cargo watch -x "test --workspace"

# Auto-check khi code thay đổi
cargo watch -x check
```

### Formatting & Linting

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Run clippy (linter)
cargo clippy --all-targets

# Fix warnings tự động
cargo fix --allow-dirty
```

### Build Optimization

```bash
# Release build (tối ưu)
cargo build --release

# Profile build info
cargo build --release --timings

# Check dependencies
cargo tree

# Update dependencies
cargo update
```

---

## 🧪 Testing Chi Tiết

### Core Tests (8 tests)

```bash
$ cargo test -p luxtensor-core

running 8 tests
✓ test_account_creation
✓ test_account_with_balance
✓ test_genesis_block
✓ test_block_hash
✓ test_transaction_creation
✓ test_transaction_hash
✓ test_state_db_creation
✓ test_state_db_set_account

All tests passed ✅
```

### Crypto Tests (9 tests)

```bash
$ cargo test -p luxtensor-crypto

running 9 tests
✓ test_keccak256
✓ test_blake3
✓ test_sha256
✓ test_merkle_tree_empty
✓ test_merkle_tree_single_leaf
✓ test_merkle_tree_multiple_leaves
✓ test_keypair_generation
✓ test_keypair_from_secret
✓ test_sign

All tests passed ✅
```

---

## 📝 Code Examples

### Tạo Block

```rust
use luxtensor_core::{Block, BlockHeader};

// Tạo genesis block
let genesis = Block::genesis();
println!("Genesis hash: {:?}", genesis.hash());
println!("Height: {}", genesis.height());
```

### Tạo Transaction

```rust
use luxtensor_core::{Transaction, Address};

let from = Address::zero();
let to = Some(Address::zero());
let tx = Transaction::new(
    0,           // nonce
    from,
    to,
    1000,        // value
    1,           // gas_price
    21000,       // gas_limit
    vec![],      // data
);

let hash = tx.hash();
println!("Transaction hash: {:?}", hash);
```

### Generate Keypair

```rust
use luxtensor_crypto::KeyPair;

// Generate random keypair
let keypair = KeyPair::generate();
let address = keypair.address();
println!("Address: 0x{}", hex::encode(address));

// Sign message
let message = [0u8; 32];
let signature = keypair.sign(&message);
println!("Signature: {:?}", signature);
```

### Hash Functions

```rust
use luxtensor_crypto::{keccak256, blake3_hash, sha256};

let data = b"hello world";

let hash1 = keccak256(data);
let hash2 = blake3_hash(data);
let hash3 = sha256(data);

println!("Keccak256: {:?}", hash1);
println!("Blake3: {:?}", hash2);
println!("SHA256: {:?}", hash3);
```

---

## 🔄 Chuyển Sang Repo Riêng

Để tạo GitHub repository riêng cho LuxTensor:

### Bước 1: Tạo Repo Trên GitHub

```bash
# Trên GitHub, tạo repo mới:
# https://github.com/sonson0910/luxtensor
```

### Bước 2: Copy Code

```bash
# Copy toàn bộ thư mục luxtensor
cp -r moderntensor/luxtensor/ ~/luxtensor/

cd ~/luxtensor

# Initialize git
git init
git add .
git commit -m "Initial LuxTensor implementation - Phase 1 complete"

# Push to GitHub
git remote add origin https://github.com/sonson0910/luxtensor.git
git branch -M main
git push -u origin main
```

### Bước 3: Setup CI/CD

CI/CD đã được configure sẵn trong `.github/workflows/ci.yml`, sẽ tự động chạy khi push code.

---

## 📊 Hiện Tại Có Gì

### ✅ Implemented (Phase 1)

**Core Primitives:**
- Block & BlockHeader với Merkle roots
- Transaction với signature (v, r, s)
- Account với nonce, balance, storage
- StateDB với cache
- Genesis block

**Cryptography:**
- Keccak256 (Ethereum-compatible)
- Blake3, SHA256
- secp256k1 keypair generation
- Message signing
- Address derivation
- Merkle tree

**Tools:**
- CLI với keypair generation
- Node binary
- Test suite (17 tests)
- CI/CD pipeline

### ⏳ TODO (Phase 2-9)

**Phase 2: Consensus** (6 tuần)
- PoS validator selection
- Fork choice rule
- Reward distribution

**Phase 3: Network** (6 tuần)
- P2P với libp2p
- Block propagation
- Sync protocol

**Phase 4: Storage** (4 tuần)
- RocksDB integration
- Merkle Patricia Trie

**Phase 5: RPC** (4 tuần)
- JSON-RPC server
- Standard methods

**Phase 6-9:** Node, Testing, Security, Deployment

---

## 🎯 Next Steps

### Ngay Bây Giờ

1. ✅ **Review code** - Xem qua implementation
2. ✅ **Run tests** - Verify tất cả đều pass
3. ✅ **Try CLI** - Test generate-key
4. ⏭️ **Tạo repo riêng** - Push lên GitHub

### Tuần Tới

1. **Begin Phase 2** - Implement PoS consensus
2. **Hire Rust engineers** - Nếu cần team
3. **Setup project board** - Track progress

---

## 📚 Documentation

- **README.md** - Overview và quick start
- **RUST_MIGRATION_ROADMAP.md** - Lộ trình chi tiết 42 tuần
- **LUXTENSOR_SETUP.md** - Setup guide
- **PYTHON_RUST_MAPPING.md** - Component mapping

---

## 🆘 Troubleshooting

### Build Errors

```bash
# Clean build
cargo clean
cargo build

# Update Rust
rustup update

# Check Rust version
rustc --version
cargo --version
```

### Test Failures

```bash
# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name -- --nocapture
```

### Dependencies Issues

```bash
# Update dependencies
cargo update

# Check for outdated deps
cargo install cargo-outdated
cargo outdated
```

---

## ✅ Summary

**Đã có:**
- ✅ Cargo workspace hoàn chỉnh
- ✅ Phase 1 implementation (Core + Crypto)
- ✅ 17 tests passing
- ✅ CLI và Node binaries
- ✅ CI/CD pipeline
- ✅ Documentation

**Có thể làm ngay:**
- Build và run tests
- Generate keypairs
- Start node
- Review code structure

**Tiếp theo:**
- Tạo GitHub repo riêng cho LuxTensor
- Begin Phase 2 (Consensus)
- Tuyển Rust engineers nếu cần

---

**Mọi thứ đã sẵn sàng để bắt đầu Phase 2! 🦀🚀**
