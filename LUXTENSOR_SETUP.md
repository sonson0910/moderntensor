# LuxTensor - Initial Setup Guide
# Hướng Dẫn Khởi Tạo Repository Rust

**Dự án:** LuxTensor (ModernTensor Rust Implementation)  
**Ngày:** 6 Tháng 1, 2026  
**Mục đích:** Setup initial Rust repository structure

---

## 📋 Prerequisites

### Required Tools
```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update
rustc --version  # Should be 1.75+

# Additional tools
cargo install cargo-watch     # Auto-rebuild on file changes
cargo install cargo-audit     # Security audit
cargo install cargo-tree      # Dependency tree
cargo install cargo-outdated  # Check outdated dependencies
cargo install cargo-expand    # Expand macros
```

### Editor Setup
```bash
# VS Code extensions
code --install-extension rust-lang.rust-analyzer
code --install-extension vadimcn.vscode-lldb
code --install-extension serayuzgur.crates

# IntelliJ Rust plugin
# Or use RustRover IDE
```

---

## 🚀 Quick Start

### Step 1: Create Repository
```bash
# Create new repository on GitHub
# https://github.com/sonson0910/luxtensor

# Clone it
git clone https://github.com/sonson0910/luxtensor
cd luxtensor

# Initialize Cargo workspace
cargo init --lib
```

### Step 2: Create Workspace Structure
```bash
# Create crate directories
mkdir -p crates
cd crates

# Create all crates
cargo new --lib luxtensor-core
cargo new --lib luxtensor-crypto
cargo new --lib luxtensor-consensus
cargo new --lib luxtensor-network
cargo new --lib luxtensor-storage
cargo new --lib luxtensor-rpc
cargo new --bin luxtensor-node
cargo new --bin luxtensor-cli

cd ..

# Create test and bench directories
mkdir -p tests benches docs examples
```

### Step 3: Setup Workspace Cargo.toml
```bash
cat > Cargo.toml << 'EOF'
[workspace]
members = [
    "crates/luxtensor-core",
    "crates/luxtensor-crypto",
    "crates/luxtensor-consensus",
    "crates/luxtensor-network",
    "crates/luxtensor-storage",
    "crates/luxtensor-rpc",
    "crates/luxtensor-node",
    "crates/luxtensor-cli",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/sonson0910/luxtensor"
authors = ["LuxTensor Team"]

[workspace.dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }
tokio-util = "0.7"
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"

# Cryptography
sha2 = "0.10"
sha3 = "0.10"
secp256k1 = { version = "0.28", features = ["recovery", "global-context"] }
k256 = { version = "0.13", features = ["ecdsa", "sha256"] }
blake3 = "1.5"
ed25519-dalek = "2.1"

# Networking
libp2p = { version = "0.53", features = ["tcp", "noise", "mplex", "gossipsub", "mdns"] }
quinn = "0.10"

# Storage
rocksdb = "0.21"

# RPC
jsonrpc-core = "18.0"
jsonrpc-http-server = "18.0"

# CLI
clap = { version = "4.4", features = ["derive"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Utils
hex = "0.4"
bytes = "1.5"
parking_lot = "0.12"

# Testing
criterion = "0.5"
proptest = "1.4"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true

[profile.bench]
inherits = "release"
EOF
```

### Step 4: Setup CI/CD
```bash
mkdir -p .github/workflows

cat > .github/workflows/ci.yml << 'EOF'
name: CI

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main, develop ]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Run tests
        run: cargo test --all-features --workspace

  fmt:
    name: Rustfmt
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - name: Enforce formatting
        run: cargo fmt --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - name: Linting
        run: cargo clippy --all-targets --all-features -- -D warnings

  audit:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: rustsec/audit-check@v1
EOF

cat > .github/workflows/release.yml << 'EOF'
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    name: Build Release
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - name: Build
        run: cargo build --release --bin luxtensor-node
      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: luxtensor-${{ matrix.os }}
          path: target/release/luxtensor-node*
EOF
```

### Step 5: Setup Rust Configuration
```bash
cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "default"
EOF

cat > .rustfmt.toml << 'EOF'
edition = "2021"
max_width = 100
tab_spaces = 4
use_small_heuristics = "Max"
EOF

cat > .gitignore << 'EOF'
# Rust
/target
Cargo.lock
**/*.rs.bk
*.pdb

# IDE
.vscode/
.idea/
*.swp
*.swo
*~

# OS
.DS_Store
Thumbs.db

# Data
/data
*.db
EOF
```

### Step 6: Create README
```bash
cat > README.md << 'EOF'
# LuxTensor 🦀

**High-performance Layer 1 blockchain written in Rust**

LuxTensor is a Rust implementation of the ModernTensor blockchain, optimized for:
- ⚡ **Performance**: 10-100x faster than Python
- 🔒 **Security**: Memory-safe, type-safe
- 🚀 **Scalability**: Efficient concurrency
- 🤖 **AI Integration**: Native zkML support

## Status

🚧 **Under Development** - Phase 1 (Foundation)

## Features

- ✅ Proof of Stake consensus
- ✅ Account-based state model
- ✅ P2P networking (libp2p)
- ✅ JSON-RPC API
- ✅ AI validation integration

## Quick Start

### Prerequisites
- Rust 1.75+ (`rustup update`)

### Build
```bash
cargo build --release
```

### Run Node
```bash
./target/release/luxtensor-node --config config.toml
```

### Run Tests
```bash
cargo test --workspace
```

## Architecture

- **luxtensor-core**: Core blockchain primitives (Block, Transaction, State)
- **luxtensor-crypto**: Cryptography (hash, signature, merkle)
- **luxtensor-consensus**: PoS consensus mechanism
- **luxtensor-network**: P2P networking
- **luxtensor-storage**: Database and state storage
- **luxtensor-rpc**: JSON-RPC API server
- **luxtensor-node**: Full node implementation
- **luxtensor-cli**: Command-line interface

## Development

### Watch mode
```bash
cargo watch -x "test --workspace"
```

### Benchmarks
```bash
cargo bench
```

### Documentation
```bash
cargo doc --open
```

## License

MIT License - see [LICENSE](LICENSE)

## Links

- **ModernTensor (Python)**: https://github.com/sonson0910/moderntensor
- **Documentation**: https://docs.luxtensor.io
- **Discord**: https://discord.gg/luxtensor

---

Built with 🦀 Rust
EOF
```

### Step 7: Initialize Git
```bash
git add .
git commit -m "Initial LuxTensor repository structure"
git push -u origin main
```

---

## 🏗️ Crate Configuration Examples

### luxtensor-core/Cargo.toml
```toml
[package]
name = "luxtensor-core"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
bincode = { workspace = true }
thiserror = { workspace = true }
bytes = { workspace = true }

luxtensor-crypto = { path = "../luxtensor-crypto" }

[dev-dependencies]
proptest = { workspace = true }
```

### luxtensor-node/Cargo.toml
```toml
[package]
name = "luxtensor-node"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[[bin]]
name = "luxtensor-node"
path = "src/main.rs"

[dependencies]
tokio = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
clap = { workspace = true }

luxtensor-core = { path = "../luxtensor-core" }
luxtensor-consensus = { path = "../luxtensor-consensus" }
luxtensor-network = { path = "../luxtensor-network" }
luxtensor-storage = { path = "../luxtensor-storage" }
luxtensor-rpc = { path = "../luxtensor-rpc" }
```

---

## 📝 Development Workflow

### Daily Development
```bash
# Start watch mode
cargo watch -x check -x "test --workspace"

# Format code
cargo fmt

# Lint code
cargo clippy --all-targets

# Run specific tests
cargo test transaction_validation

# Run with logs
RUST_LOG=debug cargo run --bin luxtensor-node
```

### Before Commit
```bash
# Full check
cargo fmt --check
cargo clippy -- -D warnings
cargo test --workspace
cargo audit
```

### Release Build
```bash
# Build optimized binary
cargo build --release --bin luxtensor-node

# Run benchmarks
cargo bench

# Generate docs
cargo doc --no-deps
```

---

## 🧪 Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_hash() {
        let block = Block::new(...);
        assert_eq!(block.hash().len(), 32);
    }
}
```

### Integration Tests
```rust
// tests/blockchain_integration.rs
#[tokio::test]
async fn test_full_flow() {
    // Test complete transaction flow
}
```

### Property Tests
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_transaction_serialization(tx: Transaction) {
        let bytes = bincode::serialize(&tx).unwrap();
        let decoded: Transaction = bincode::deserialize(&bytes).unwrap();
        assert_eq!(tx, decoded);
    }
}
```

---

## 📊 Performance Monitoring

### Benchmarks
```rust
// benches/block_validation.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_block_validation(c: &mut Criterion) {
    c.bench_function("block validation", |b| {
        let block = create_test_block();
        b.iter(|| {
            black_box(block.verify()).unwrap();
        });
    });
}

criterion_group!(benches, benchmark_block_validation);
criterion_main!(benches);
```

Run benchmarks:
```bash
cargo bench
```

---

## 🔧 Troubleshooting

### Common Issues

**Problem:** `error: linker 'cc' not found`
```bash
# Ubuntu/Debian
sudo apt install build-essential

# macOS (install Xcode Command Line Tools)
xcode-select --install
```

**Problem:** RocksDB compilation fails
```bash
# Install dependencies
sudo apt install libclang-dev libsnappy-dev
```

**Problem:** Slow compilation
```bash
# Use mold linker (Linux)
cargo install -f mold
echo 'target.x86_64-unknown-linux-gnu.linker = "clang"' >> ~/.cargo/config.toml
echo 'target.x86_64-unknown-linux-gnu.rustflags = ["-C", "link-arg=-fuse-ld=mold"]' >> ~/.cargo/config.toml
```

---

## 📚 Resources

### Rust Blockchain Examples
- [Substrate](https://github.com/paritytech/substrate)
- [Solana](https://github.com/solana-labs/solana)
- [NEAR](https://github.com/near/nearcore)
- [Lighthouse (Ethereum)](https://github.com/sigp/lighthouse)

### Learning Rust
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Rustlings](https://github.com/rust-lang/rustlings)

### Crypto in Rust
- [RustCrypto](https://github.com/RustCrypto)
- [rust-secp256k1](https://github.com/rust-bitcoin/rust-secp256k1)

---

## ✅ Next Steps

After completing this setup:

1. ✅ Repository created and initialized
2. ✅ CI/CD pipeline configured
3. ✅ Development environment ready
4. ⏭️ Start Phase 1: Implement core primitives
5. ⏭️ Write first tests
6. ⏭️ Create documentation

**Ready to start coding! 🦀🚀**
