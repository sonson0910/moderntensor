# ADR-005: AI Layer Integration

**Status**: Accepted
**Date**: 2026-01-31
**Authors**: Luxtensor Team

---

## Context

Luxtensor is purpose-built for decentralized AI/ML. The AI layer must:

1. Support multi-model inference across subnets
2. Validate AI outputs via commit-reveal and weight consensus
3. Protect against AI service failures
4. Score miners based on inference quality

## Decision

### Architecture Overview

```
┌──────────────────────────────────────────────────┐
│                    SDK Layer                      │
│  ┌──────────┐  ┌──────────────┐  ┌────────────┐  │
│  │  Axon    │  │   Dendrite   │  │   ZKML     │  │
│  │ (Server) │  │   (Client)   │  │  Scoring   │  │
│  └──────────┘  └──────────────┘  └────────────┘  │
└──────────────────────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────┐
│               Consensus Layer                     │
│  ┌──────────────┐  ┌───────────────────────────┐ │
│  │ Commit-Reveal│  │    Weight Consensus       │ │
│  │   Protocol   │  │ (Multi-validator voting)  │ │
│  └──────────────┘  └───────────────────────────┘ │
│  ┌──────────────┐  ┌───────────────────────────┐ │
│  │   Circuit    │  │    Liveness Monitor       │ │
│  │   Breaker    │  │ (AI health tracking)      │ │
│  └──────────────┘  └───────────────────────────┘ │
└──────────────────────────────────────────────────┘
```

### Subnet Model

| Component | Purpose |
|-----------|---------|
| **Subnet** | Specialized network for a task (e.g., text generation) |
| **Miner** | Runs AI model, serves inference requests |
| **Validator** | Scores miners, sets weights |
| **UID** | Unique identifier within subnet |

### Weight Setting Flow

1. **Commit Phase**: Validators commit hash of weights
2. **Reveal Phase**: Validators reveal actual weights + salt
3. **Consensus**: Multi-validator agreement on final weights
4. **Apply**: Weights determine miner rewards

### Circuit Breaker Design

Protects blockchain from AI layer failures:

| State | Condition | Behavior |
|-------|-----------|----------|
| **Closed** | Healthy | Normal operation |
| **HalfOpen** | Testing | Limited requests |
| **Open** | Failed | Fallback weights |

Triggers:

- 50%+ timeout rate
- AI endpoint unreachable
- Consensus stall

### Alternatives Considered

| Approach | Pros | Cons | Decision |
|----------|------|------|----------|
| **Commit-Reveal + Consensus** | Prevents weight manipulation | Adds latency | ✅ Selected |
| Single validator weights | Simple | Centralization risk | ❌ Rejected |
| On-chain ML | Verifiable | Gas costs prohibitive | ❌ Rejected |

## Consequences

### Positive

- Sybil-resistant weight setting
- Fault-tolerant AI integration
- Clear incentives for quality inference

### Negative

- Epoch delays for weight updates
- Complexity in validator coordination

### Risks

- AI model quality subjectivity (mitigated by multi-validator voting)
- Model collusion (mitigated by stake requirements)

## Implementation

- `luxtensor-consensus/src/commit_reveal.rs` - Commit-reveal protocol
- `luxtensor-consensus/src/weight_consensus.rs` - Multi-validator voting
- `luxtensor-consensus/src/circuit_breaker.rs` - AI layer protection
- `sdk/ai_ml/` - Python AI/ML utilities
- `sdk/axon/` - Miner server framework
- `sdk/dendrite/` - Validator query client
