# ADR-001: Consensus Mechanism Selection

**Status**: Accepted
**Date**: 2026-01-31
**Authors**: Luxtensor Team

---

## Context

Luxtensor is a Layer 1 blockchain designed for decentralized AI/ML computations. The consensus mechanism must:

1. Provide fast finality for real-time AI inference results
2. Support validator rotation based on subnet performance
3. Enable slashing for malicious weight manipulation
4. Maintain security with a reasonable validator set size

## Decision

**We chose Proof-of-Stake (PoS) with GHOST fork-choice rule.**

### Alternatives Considered

| Mechanism | Pros | Cons | Decision |
|-----------|------|------|----------|
| **PoS + GHOST** | Fast finality, energy efficient, validator slashing | Requires stake lockup | ✅ **Selected** |
| PBFT | Instant finality | O(n²) message complexity, max ~100 validators | ❌ Rejected |
| PoW | Proven security | High energy, slow finality | ❌ Rejected |
| DPoS | High TPS | Centralization risk | ❌ Rejected |

### Key Design Choices

1. **Block Time**: 3 seconds
   - Balances throughput vs network propagation
   - Fast enough for AI inference latency requirements

2. **Validator Set**: Up to 128 validators
   - Large enough for decentralization
   - Small enough for efficient communication

3. **Finality**: 32 block confirmations (~96 seconds)
   - Probabilistic finality similar to Ethereum

4. **Slashing Conditions**:
   - Double signing: 50% stake slash
   - Offline (100 missed blocks): 10% slash
   - Long-range attacks: checkpoints enforced

## Consequences

### Positive

- Sub-minute transaction finality
- Energy efficient consensus
- Economically secure via slashing

### Negative

- Requires minimum stake (10,000 MDT)
- Validators need reliable uptime

### Risks

- Nothing-at-stake (mitigated by slashing)
- Long-range attacks (mitigated by checkpoints)

## Implementation

- `luxtensor-consensus/src/pos.rs` - Core PoS logic
- `luxtensor-consensus/src/fork_choice.rs` - GHOST implementation
- `luxtensor-consensus/src/slashing.rs` - Slashing manager
- `luxtensor-consensus/src/long_range_protection.rs` - Checkpoint validation
