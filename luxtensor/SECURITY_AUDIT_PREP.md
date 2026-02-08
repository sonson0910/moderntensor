# LuxTensor Security Audit Preparation

> **Status**: Pre-audit checklist — last updated for v0.4.0  
> **Target**: External security audit engagement  
> **Coverage**: All 16 crates (~63 000 LOC Rust, ~12 000 LOC Python SDK)

---

## 1. Architecture Summary

| Layer | Crates | Purpose |
|-------|--------|---------|
| **Core** | `luxtensor-core` | Types, blocks, transactions, state, accounts, multisig, bridge interface |
| **Crypto** | `luxtensor-crypto` | Keccak-256, secp256k1 signatures, EC-VRF (RFC 9381), address derivation |
| **Consensus** | `luxtensor-consensus` | PoS engine, fork choice, BFT finality, slashing, emission, governance |
| **Network** | `luxtensor-network` | libp2p swarm, gossip, state sync, light client, eclipse protection |
| **Contracts** | `luxtensor-contracts` | EVM via `revm`, precompiles, account abstraction (ERC-4337) |
| **Storage** | `luxtensor-storage` | RocksDB-backed trie, WAL, state pruning |
| **RPC** | `luxtensor-rpc` | JSON-RPC 2.0 (eth_*, lux_*), WebSocket, rate limiting |
| **Node** | `luxtensor-node` | Full node orchestration, mempool, service lifecycle |
| **CLI** | `luxtensor-cli` | Node management, key generation, configuration |
| **Oracle** | `luxtensor-oracle` | Off-chain AI inference bridge |
| **ZKVM** | `luxtensor-zkvm` | RISC Zero integration, proof-of-training verifier |
| **Indexer** | `indexer` (Rust + Subxt) | On-chain event indexing |
| **Python SDK** | `sdk/` | Client library, signing, CLI tools |

### Chain IDs (Unified)
- Mainnet: **8899**
- Testnet: **9999**  
- Devnet:  **8898**

---

## 2. Threat Model

### 2.1 Consensus Attacks
| Threat | Mitigation | Audit Focus |
|--------|------------|-------------|
| Long-range attack | Checkpoint-based protection (`long_range_protection.rs`) | Verify checkpoint finality logic |
| Nothing-at-stake | Slashing for equivocation (`slashing.rs`) | Slashing evidence + penalty math |
| Validator withholding | Liveness monitor + rotation (`liveness.rs`, `rotation.rs`) | Liveness timeout edge cases |
| Stake grinding | RANDAO + EC-VRF leader election (`randao.rs`, `vrf.rs`) | VRF soundness & bias resistance |
| Governance attack | Quorum + timelock + supermajority (`governance.rs`) | Timelock bypass, vote weight manipulation |

### 2.2 Network Attacks
| Threat | Mitigation | Audit Focus |
|--------|------------|-------------|
| Eclipse attack | Diverse peer selection (`eclipse_protection.rs`) | ASN/subnet diversity enforcement |
| Sybil flooding | Rate limiter + peer scoring (`rate_limiter.rs`) | Rate limit bypass under load |
| Gossip amplification | Topic-based pub/sub with dedup | Message deduplication correctness |

### 2.3 Smart Contract / EVM Layer
| Threat | Mitigation | Audit Focus |
|--------|------------|-------------|
| Reentrancy via precompile | revm call depth + gas metering | Precompile gas costs |
| AA bundler manipulation | Paymaster budget checks | Budget exhaustion edge cases |
| State corruption | RocksDB WAL + checkpoint | Crash-recovery state integrity |

### 2.4 Cryptographic Risks
| Threat | Mitigation | Audit Focus |
|--------|------------|-------------|
| VRF bias | EC-VRF (secp256k1) per RFC 9381 | Verify try-and-increment hash-to-curve |
| Signature malleability | Canonical-S enforcement | Check all `verify()` paths |
| ZK proof forgery | Dev-mode proofs rejected in production | Feature-gate enforcement |

### 2.5 Cross-Chain Bridge
| Threat | Mitigation | Audit Focus |
|--------|------------|-------------|
| Replay attack | Nonce + message hash dedup | Nonce gap / reordering |
| Relay collusion | M-of-N attestation threshold | Threshold < relayer count |
| Double-spend | Lock-and-mint atomicity | Race condition in concurrent proofs |

---

## 3. Critical Code Paths (Priority Order)

1. **`luxtensor-crypto/src/vrf.rs`** — EC-VRF prove/verify (validator selection randomness)
2. **`luxtensor-crypto/src/signature.rs`** — secp256k1 signing/verification
3. **`luxtensor-consensus/src/pos.rs`** — Proof-of-Stake engine (block proposal, validation)
4. **`luxtensor-consensus/src/slashing.rs`** — Slashing evidence processing & penalty calculation
5. **`luxtensor-consensus/src/fast_finality.rs`** — BFT pre-commit/commit protocol
6. **`luxtensor-consensus/src/governance.rs`** — On-chain governance (proposals, voting, timelock)
7. **`luxtensor-core/src/bridge.rs`** — Cross-chain bridge message verification
8. **`luxtensor-contracts/src/revm_integration.rs`** — EVM execution & precompile routing
9. **`luxtensor-contracts/src/account_abstraction.rs`** — ERC-4337 UserOperation validation
10. **`luxtensor-storage/src/trie.rs`** — Merkle Patricia Trie (state root integrity)
11. **`luxtensor-zkvm/src/verifier.rs`** — ZK proof verification (replay protection, size limits)
12. **`luxtensor-rpc/src/server.rs`** — RPC authentication & rate limiting

---

## 4. Dependency Audit

### High-Risk Dependencies
| Crate | Version | Risk | Notes |
|-------|---------|------|-------|
| `k256` | 0.13 | LOW | RustCrypto, well-audited |
| `revm` | 17.x | MEDIUM | EVM implementation — follow CVEs |
| `libp2p` | 0.54 | MEDIUM | Complex networking stack |
| `rocksdb` | 0.22 | LOW | Mature C++ FFI |
| `risc0-zkvm` | feature-gated | HIGH | Only dev-mode without feature |

### Supply Chain
- All workspace deps pinned in root `Cargo.toml`
- No `build.rs` scripts with network access
- `cargo-audit` should be run before engagement

---

## 5. Test Coverage Summary

| Crate | Unit Tests | Integration | Property |
|-------|-----------|-------------|----------|
| `luxtensor-core` | ✅ 40+ | ✅ | proptest |
| `luxtensor-crypto` | ✅ 20+ | ✅ | — |
| `luxtensor-consensus` | ✅ 80+ | ✅ | proptest |
| `luxtensor-network` | ✅ 30+ | ✅ | — |
| `luxtensor-contracts` | ✅ 25+ | ✅ | — |
| `luxtensor-storage` | ✅ 20+ | ✅ | — |
| `luxtensor-rpc` | ✅ 15+ | ✅ | — |
| `luxtensor-zkvm` | ✅ 20+ | ✅ | — |
| `luxtensor-node` | ✅ 10+ | ⚠️ partial | — |
| Python SDK | ✅ 50+ | ✅ | — |

### Known Gaps
- Fuzzing: not yet enabled (recommend `cargo-fuzz` for VRF, signature, trie)
- Formal verification: none (low priority for initial audit)

---

## 6. Build & Reproduce

```bash
# Prerequisites
rustup default stable  # Rust 1.82+
cargo install cargo-audit cargo-deny

# Build all crates
cd luxtensor
cargo build --workspace 2>&1

# Run all tests
cargo test --workspace 2>&1

# Security checks
cargo audit
cargo deny check advisories

# Run with Docker
docker build -f docker/Dockerfile.rust -t luxtensor-node .
```

---

## 7. Known Issues & Accepted Risks

| # | Issue | Status | Risk Level |
|---|-------|--------|------------|
| 1 | ZK prover dev-mode only (no `risc0` feature) | Accepted | MEDIUM — proofs rejected without feature |
| 2 | Bridge is interface-only, no live relayers | Accepted | LOW — no funds at risk |
| 3 | Light client Merkle proofs not integrated into RPC yet | Accepted | LOW |
| 4 | Governance not yet wired into block execution | Accepted | LOW — module ready, integration pending |
| 5 | No formal BFT safety proof | Known | MEDIUM — tested but not mathematically proven |

---

## 8. Audit Scope Recommendation

### Phase 1 — Critical (2-3 weeks)
- Cryptographic primitives (VRF, signatures, hashing)
- Consensus engine (PoS, fork choice, finality, slashing)
- State management (trie, storage, state roots)

### Phase 2 — High (1-2 weeks)
- EVM integration & precompiles
- Account Abstraction (ERC-4337)
- Governance module
- Bridge interface

### Phase 3 — Medium (1 week)
- Network layer (P2P, gossip, rate limiting, eclipse protection)
- RPC security (auth, rate limiting, input validation)
- ZK verifier hardening

---

## 9. Contact & Access

- **Repository**: Private — access granted upon NDA
- **Documentation**: `docs/architecture/`, `WHITEPAPER.md`
- **Test environment**: Docker compose in `docker/`
- **Point of contact**: LuxTensor Core Team

---

*This document should be shared with auditors before engagement. Update the "Known Issues" section as fixes land.*
