# ADR-002: Block Time Selection

**Status**: Accepted
**Date**: 2026-01-31
**Authors**: Luxtensor Team

---

## Context

Block time affects transaction throughput, finality, and network resource usage.

## Decision

**Block time: 3 seconds**

### Alternatives Considered

| Block Time | Pros | Cons | Decision |
|------------|------|------|----------|
| 1 second | High TPS | Network propagation issues | ❌ |
| **3 seconds** | Balance of speed/stability | Moderate finality | ✅ |
| 12 seconds | Proven (Ethereum) | Slow for AI inference | ❌ |

### Rationale

1. **AI Latency**: 3s allows fast inference result confirmation
2. **Network Propagation**: Sufficient for global P2P
3. **Validators**: Reasonable rotation period
4. **Finality**: ~96s with 32 confirmations

## Implementation

- `luxtensor-node/src/config.rs`: `BLOCK_TIME_SECONDS = 3`
