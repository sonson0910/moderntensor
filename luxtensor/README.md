# LuxTensor 🚀

**High-Performance Layer 1 Blockchain in Rust**

LuxTensor is a Rust implementation of the ModernTensor Layer 1 blockchain, designed for AI/ML workloads with native support for zero-knowledge machine learning and decentralized AI validation.

## 🎯 Project Status

**Current Phase:** Planning & Setup (Phase 0)  
**Target:** Production-ready mainnet in 9 months  
**Progress:** 0% (Converting from Python ModernTensor)

## ✨ Features

- **High Performance:** 10-100x faster than Python implementation
- **Proof of Stake:** Efficient consensus mechanism
- **P2P Network:** Built on libp2p for robust peer-to-peer communication
- **AI Validation:** Native support for AI/ML task validation
- **Type Safety:** Rust's strong type system ensures correctness
- **Async Runtime:** Built on Tokio for high concurrency

## 🏗️ Architecture

LuxTensor is organized as a Cargo workspace with multiple crates:

```
luxtensor/
├── crates/
│   ├── luxtensor-types/      # Core types and errors
│   ├── luxtensor-crypto/     # Cryptographic primitives
│   ├── luxtensor-core/       # Blockchain core (blocks, transactions, state)
│   ├── luxtensor-consensus/  # PoS consensus mechanism
│   ├── luxtensor-network/    # P2P networking
│   ├── luxtensor-storage/    # Database layer (RocksDB)
│   ├── luxtensor-api/        # JSON-RPC and GraphQL APIs
│   ├── luxtensor-node/       # Node binary
│   └── luxtensor-cli/        # CLI tools
└── tests/                    # Integration tests
```

## 🚀 Quick Start

### Prerequisites

- Rust 1.75 or later
- Cargo (comes with Rust)

### Installation

```bash
# Clone the repository
git clone https://github.com/sonson0910/luxtensor.git
cd luxtensor

# Build all crates
cargo build --release

# Run tests
cargo test --all

# Run benchmarks
cargo bench
```

### Running a Node

```bash
# Initialize genesis
cargo run --bin luxtensor-node -- init --genesis genesis.json

# Start node
cargo run --bin luxtensor-node -- start \
  --port 30303 \
  --rpc-port 8545 \
  --bootstrap /ip4/127.0.0.1/tcp/30304
```

### Using the CLI

```bash
# Create wallet
cargo run --bin luxtensor-cli -- wallet create my-wallet

# Check balance
cargo run --bin luxtensor-cli -- wallet balance 0x1234...

# Send transaction
cargo run --bin luxtensor-cli -- wallet send \
  --from my-wallet \
  --to 0x5678... \
  --amount 100
```

## 📚 Documentation

- [Conversion Roadmap](../RUST_CONVERSION_ROADMAP.md) - Detailed plan for converting from Python
- [Architecture Guide](docs/architecture.md) - System design and components
- [API Reference](docs/api.md) - RPC and GraphQL API documentation
- [Development Guide](docs/development.md) - Contributing guidelines

## 🛠️ Development

### Building

```bash
# Build all crates
cargo build

# Build specific crate
cargo build -p luxtensor-core

# Build with optimizations
cargo build --release
```

### Testing

```bash
# Run all tests
cargo test --all

# Run tests for specific crate
cargo test -p luxtensor-core

# Run with output
cargo test -- --nocapture
```

### Benchmarking

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench block_processing
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint code
cargo clippy --all-targets --all-features

# Check for security vulnerabilities
cargo audit
```

## 📊 Performance Targets

| Metric | Target | Python Baseline |
|--------|--------|-----------------|
| Transaction Throughput | > 1,000 TPS | ~10 TPS |
| Block Processing | < 1 second | ~10 seconds |
| Sync Speed | > 500 blocks/s | ~50 blocks/s |
| Memory Usage | < 2GB | ~4GB |

## 🗺️ Roadmap

See [RUST_CONVERSION_ROADMAP.md](../RUST_CONVERSION_ROADMAP.md) for the complete conversion plan.

### Phase 0: Setup (Month 1) - Current
- [x] Repository structure
- [x] Workspace configuration
- [ ] CI/CD pipeline
- [ ] Technical design

### Phase 1: Core Blockchain (Months 1-2)
- [ ] Types and crypto
- [ ] Block and transaction structures
- [ ] State management
- [ ] Validation logic

### Phase 2: Consensus (Months 2-3)
- [ ] PoS implementation
- [ ] Validator selection
- [ ] Fork choice
- [ ] Reward distribution

### Phase 3: Network (Months 3-4)
- [ ] P2P protocol (libp2p)
- [ ] Peer discovery
- [ ] Block propagation
- [ ] Sync mechanism

### Phases 4-9
- Storage, API, Testing, Documentation, Testnet, Mainnet

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Getting Started

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Write tests
5. Run `cargo fmt` and `cargo clippy`
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🔗 Links

- **Website:** https://luxtensor.io
- **Documentation:** https://docs.luxtensor.io
- **Discord:** https://discord.gg/luxtensor
- **Twitter:** https://twitter.com/luxtensor

## 🙏 Acknowledgments

- ModernTensor team for the original Python implementation
- Polkadot/Substrate for Rust blockchain inspiration
- libp2p community for networking stack
- Rust blockchain community

---

**Built with ❤️ in Rust**
