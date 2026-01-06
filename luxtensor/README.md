# LuxTensor

**A high-performance Layer 1 blockchain written in Rust for decentralized AI/ML workloads**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

## 🚀 Overview

LuxTensor is a Rust implementation of a Layer 1 blockchain optimized for AI/ML workloads. It's a complete rewrite of the ModernTensor blockchain in Rust, designed for:

- ⚡ **High Performance**: 10-100x faster than Python implementation
- 🔒 **Memory Safety**: Rust's ownership system prevents common bugs
- 🌐 **Scalability**: Efficient P2P networking and consensus
- 🔐 **Security**: Built-in memory safety and thread safety

## 📋 Features

- **Proof of Stake (PoS) Consensus**: Efficient and eco-friendly consensus mechanism
- **GHOST Fork Choice**: Optimal chain selection algorithm
- **Account-Based State**: Ethereum-style state management
- **P2P Networking**: libp2p-based peer-to-peer communication
- **RocksDB Storage**: Fast and reliable persistent storage
- **JSON-RPC API**: Standard blockchain API interface
- **Prometheus Metrics**: Built-in monitoring and observability

## 🏗️ Architecture

```
luxtensor/
├── core/           # Core blockchain primitives (blocks, transactions, state)
├── consensus/      # PoS consensus and validator management
├── network/        # P2P networking and chain synchronization
├── storage/        # Persistent storage layer
├── rpc/           # JSON-RPC and GraphQL APIs
├── node/          # Full node implementation
├── primitives/    # Common types and utilities
└── testnet/       # Testnet utilities and tools
```

## 🔧 Prerequisites

- Rust 1.75 or higher
- Cargo
- LLVM (for RocksDB compilation)

## 📦 Installation

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Clone the repository

```bash
git clone https://github.com/sonson0910/luxtensor.git
cd luxtensor
```

### 3. Build the project

```bash
# Build all components
cargo build --release

# Build specific component
cargo build -p luxtensor-node --release
```

### 4. Run tests

```bash
# Run all tests
cargo test

# Run tests for specific component
cargo test -p luxtensor-core
```

## 🚀 Quick Start

### Running a Full Node

```bash
# Start a node with default configuration
cargo run -p luxtensor-node --release -- start

# Start with custom configuration
cargo run -p luxtensor-node --release -- start --config custom-config.toml

# Join testnet
cargo run -p luxtensor-node --release -- start --network testnet
```

### Configuration

Create a `config.toml` file:

```toml
[node]
name = "my-node"
data_dir = "./data"

[network]
listen_addr = "/ip4/0.0.0.0/tcp/30333"
bootstrap_nodes = [
    "/ip4/127.0.0.1/tcp/30333/p2p/..."
]

[consensus]
validator = false

[rpc]
enabled = true
addr = "127.0.0.1:9933"
```

## 🧪 Development

### Project Structure

Each component is a separate crate:

- **luxtensor-core**: Core blockchain types and logic
- **luxtensor-consensus**: Consensus mechanisms
- **luxtensor-network**: P2P networking
- **luxtensor-storage**: Database layer
- **luxtensor-rpc**: RPC server and API
- **luxtensor-node**: Full node binary

### Adding Dependencies

Add to workspace dependencies in root `Cargo.toml`:

```toml
[workspace.dependencies]
new-crate = "1.0"
```

Then use in component:

```toml
[dependencies]
new-crate = { workspace = true }
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint code
cargo clippy -- -D warnings

# Check for security issues
cargo audit
```

### Benchmarking

```bash
# Run benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench block_processing
```

## 📊 Performance

Current performance metrics (target vs Python):

| Metric | Python | Rust Target |
|--------|--------|-------------|
| Block processing | 100ms | 10ms |
| Transaction throughput | 50 TPS | 500-1000 TPS |
| State access | 50ms | 5ms |
| Memory usage | ~500MB | ~100MB |

## 🔐 Security

LuxTensor benefits from Rust's memory safety guarantees:

- No null pointer dereferences
- No data races
- No buffer overflows
- Thread-safe by default

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📚 Documentation

- [Migration Roadmap](../RUST_MIGRATION_ROADMAP.md) - Detailed migration plan from Python
- [Architecture Guide](docs/architecture.md) - System architecture
- [API Reference](docs/api.md) - RPC API documentation
- [Developer Guide](docs/development.md) - Development guidelines

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🌟 Acknowledgments

- Inspired by [ModernTensor](https://github.com/sonson0910/moderntensor) Python implementation
- Built with [Substrate](https://substrate.io/) patterns
- Powered by [libp2p](https://libp2p.io/) networking stack

## 📞 Contact

- Website: https://luxtensor.io (coming soon)
- Twitter: @luxtensor (coming soon)
- Discord: [Join our community](https://discord.gg/luxtensor) (coming soon)

---

**Status**: 🚧 Under active development - Migration in progress

See [RUST_MIGRATION_ROADMAP.md](../RUST_MIGRATION_ROADMAP.md) for current progress and timeline.
