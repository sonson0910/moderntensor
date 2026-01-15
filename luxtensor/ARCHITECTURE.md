# LuxTensor Architecture Overview

## Executive Summary

LuxTensor is a custom Layer-1 blockchain optimized for decentralized AI workloads, built in Rust with full EVM compatibility.

---

## Crate Structure

```
luxtensor/crates/
├── luxtensor-core      # Core types, blocks, transactions, state
├── luxtensor-consensus # PoS, emission, rewards, slashing, staking
├── luxtensor-rpc       # JSON-RPC server, WebSocket, Ethereum compatibility
├── luxtensor-network   # P2P networking, sync, peer management
├── luxtensor-storage   # RocksDB storage, metagraph persistence
├── luxtensor-crypto    # Cryptographic utilities (keccak, signatures)
├── luxtensor-contracts # EVM/Solidity integration
├── luxtensor-node      # Node runner, block production
├── luxtensor-cli       # Command-line interface
└── luxtensor-tests     # Integration tests
```

---

## Module Details

### luxtensor-core (Foundation)

| File | Purpose |
|------|---------|
| `types.rs` | Core types (Address, Hash, etc.) |
| `block.rs` | Block structure and validation |
| `transaction.rs` | Transaction types |
| `state.rs` | Account state management |
| `subnet.rs` | Subnet definitions |
| `account.rs` | Account model |

### luxtensor-consensus (Tokenomics v3.1)

| Module | Lines | Purpose |
|--------|-------|---------|
| `pos.rs` | ~300 | Proof of Stake consensus |
| `validator.rs` | ~250 | Validator set management |
| `emission.rs` | ~400 | Adaptive emission (per tokenomics) |
| `reward_distribution.rs` | ~400 | Miner/Validator/Delegator rewards |
| `burn_manager.rs` | ~250 | 4 burn mechanisms |
| `slashing.rs` | ~500 | Slashing (80% burned per tokenomics) |
| `node_tier.rs` | ~350 | Progressive staking tiers |
| `token_allocation.rs` | ~500 | Vesting, TGE |
| `commit_reveal.rs` | ~600 | Anti-cheat: commit-reveal weights |
| `weight_consensus.rs` | ~600 | Multi-validator weight consensus |

### luxtensor-rpc (API Layer)

| File | Size | Purpose |
|------|------|---------|
| `server.rs` | 77KB | Main RPC handlers (needs refactor) |
| `eth_rpc.rs` | 34KB | Ethereum-compatible methods |
| `websocket.rs` | 17KB | WebSocket subscriptions |
| `logs.rs` | 15KB | Event logs |
| `subnet_rpc.rs` | 8KB | Subnet management |
| `node_rpc.rs` | 9KB | Node status |

### luxtensor-network (P2P)

| File | Purpose |
|------|---------|
| `p2p.rs` | libp2p integration |
| `sync.rs` | Block synchronization |
| `peer.rs` | Peer management |
| `messages.rs` | Network messages |
| `rate_limiter.rs` | DDoS protection |

---

## Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        USER/SDK                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     luxtensor-rpc                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │  eth_*      │  │  staking_*  │  │  query_*    │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
└─────────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│ luxtensor-core  │  │luxtensor-consns │  │luxtensor-storage│
│                 │  │                 │  │                 │
│ • Blocks        │  │ • PoS           │  │ • RocksDB       │
│ • Transactions  │  │ • Emission      │  │ • Metagraph     │
│ • State         │  │ • Slashing      │  │ • Indexer       │
└─────────────────┘  └─────────────────┘  └─────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    luxtensor-network                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │    P2P      │  │    Sync     │  │   Peers     │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
└─────────────────────────────────────────────────────────────────┘
```

---

## Tokenomics Implementation

| Tokenomics Feature | Implementation |
|-------------------|----------------|
| Max Supply: 21M | `token_allocation.rs` |
| Adaptive Emission | `emission.rs` |
| 4 Burn Mechanisms | `burn_manager.rs` |
| Progressive Staking (4 tiers) | `node_tier.rs` |
| Lock Bonuses (up to 2x) | `reward_distribution.rs` |
| Slashing (80% burned) | `slashing.rs`, `commit_reveal.rs` |

---

## EVM Compatibility

- **Engine**: revm (Cancun spec)
- **Features**: Contract deployment, calls, precompiles
- **Standards**: EIP-1559, EIP-170
- **Tooling**: Hardhat, Foundry, Web3.py, ethers.js

---

## Known Improvement Areas

| Area | Issue | Priority | Recommendation |
|------|-------|----------|----------------|
| `server.rs` | 77KB, 1909 lines | Medium | Split into modules |
| Config files | 3 similar node configs | Low | Template-based config |
| Tests | Integration tests needed | High | Add more coverage |

---

## Quick Start

```bash
# Build
cd luxtensor && cargo build --release

# Run node
./target/release/luxtensor-node --config config.toml

# Run tests
cargo test --workspace
```

---

*Document Version: 1.0 - January 2026*
