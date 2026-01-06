# LuxTensor 🦀

**High-performance Layer 1 blockchain written in Rust**

LuxTensor is a Rust implementation of the ModernTensor blockchain, optimized for AI/ML workloads with native support for zero-knowledge machine learning and decentralized AI validation.

## Status

🚧 **Phase 1: Foundation** - Core primitives and cryptography implemented

## Features

- ⚡ **High Performance**: 10-100x faster than Python implementation
- 🔒 **Memory Safe**: Rust's ownership system prevents memory leaks
- 🚀 **True Parallelism**: No GIL, efficient concurrency with tokio
- 🤖 **AI-First**: Built for AI/ML validation workloads
- 🔐 **Secure**: Type-safe, compile-time error checking

## Architecture

LuxTensor is organized as a Cargo workspace with 8 specialized crates:

- **luxtensor-core** - Core blockchain primitives (Block, Transaction, State, Account)
- **luxtensor-crypto** - Cryptography (Keccak256, Blake3, secp256k1, Merkle trees)
- **luxtensor-consensus** - PoS consensus mechanism (Phase 2)
- **luxtensor-network** - P2P networking with libp2p (Phase 3)
- **luxtensor-storage** - RocksDB database with Merkle Patricia Trie (Phase 4)
- **luxtensor-rpc** - JSON-RPC API server (Phase 5)
- **luxtensor-node** - Full node binary
- **luxtensor-cli** - Command-line interface

## Quick Start

### Prerequisites

- Rust 1.75+ (`rustup update`)
- Cargo

### Build

```bash
# Clone repository
git clone https://github.com/sonson0910/luxtensor
cd luxtensor

# Build all crates
cargo build --release

# Run tests
cargo test --workspace
```

### Run Node

```bash
# Start node
./target/release/luxtensor-node
```

### CLI Usage

```bash
# Show version
./target/release/luxtensor version

# Generate new keypair
./target/release/luxtensor generate-key

# Check status
./target/release/luxtensor status
```

## Development

### Watch Mode

```bash
cargo watch -x "test --workspace"
```

### Run Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p luxtensor-core

# With output
cargo test --workspace -- --nocapture
```

### Benchmarks

```bash
cargo bench
```

### Documentation

```bash
cargo doc --open
```

## Performance Targets

| Metric | Python (ModernTensor) | Rust (LuxTensor) | Improvement |
|--------|----------------------|------------------|-------------|
| **TPS** | 50-100 | 1,000-5,000 | **10-50x** |
| **Block Time** | 3-5s | <1s | **3-5x** |
| **Memory/Node** | 500MB | <100MB | **5x** |
| **Block Hash** | 5.2ms | 0.05ms | **100x** |
| **Signature Verify** | 8.1ms | 0.12ms | **67x** |

## Roadmap

- ✅ **Phase 1**: Foundation (4 weeks) - Core primitives, crypto
- ⏳ **Phase 2**: Consensus (6 weeks) - PoS, validator selection, fork choice
- ⏳ **Phase 3**: Network (6 weeks) - P2P with libp2p, sync protocol
- ⏳ **Phase 4**: Storage (4 weeks) - RocksDB, Merkle Patricia Trie
- ⏳ **Phase 5**: RPC (4 weeks) - JSON-RPC API server
- ⏳ **Phase 6**: Node (4 weeks) - Full node implementation
- ⏳ **Phase 7**: Testing (6 weeks) - Comprehensive testing
- ⏳ **Phase 8**: Security Audit (4 weeks) - External audit
- ⏳ **Phase 9**: Deployment (4 weeks) - Testnet, mainnet

**Total Timeline**: 42 weeks (10.5 months)

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test --workspace`)
5. Run clippy (`cargo clippy --all-targets`)
6. Format code (`cargo fmt`)
7. Commit your changes (`git commit -m 'Add amazing feature'`)
8. Push to branch (`git push origin feature/amazing-feature`)
9. Open a Pull Request

## License

MIT License - see [LICENSE](LICENSE) file for details

## Links

- **ModernTensor (Python)**: https://github.com/sonson0910/moderntensor
- **Documentation**: See `/docs` directory
- **Roadmap**: See `RUST_MIGRATION_ROADMAP.md` in parent directory

## Acknowledgments

Built with 🦀 Rust, inspired by:
- Ethereum (EVM, state model)
- Polkadot (Substrate framework)
- Solana (High performance)
- NEAR Protocol (Developer experience)

---

**Status**: Phase 1 Complete - Foundation  
**Next**: Phase 2 - Consensus Implementation  
**Timeline**: 10.5 months to production mainnet
