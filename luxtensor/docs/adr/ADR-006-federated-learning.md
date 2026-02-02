# ADR-006: Federated Learning Layer

## Status

Accepted

## Context

ModernTensor has successfully implemented native AI inference (Phase 4) with opcodes 0x10-0x13. To achieve full differentiation from competitors (Bittensor, Render, Fetch.AI), we need **decentralized training** capabilities.

Current AI blockchain landscape:

- **Bittensor**: Incentive layer only, no native training
- **Render**: GPU rental, no on-chain verification
- **ModernTensor**: First with native AI opcodes → now adding training

## Decision

Implement **Federated Learning Layer** using the following architecture:

### 1. Algorithm Selection: FedAvg (Federated Averaging)

**Rationale:**

- Well-understood, battle-tested algorithm
- Lower bandwidth requirements vs other methods
- Gradient compression compatible
- Simpler on-chain aggregation

**Trade-off:**

- Less robust to non-IID data than FedProx
- Acceptable for MVP, can upgrade later

### 2. Architecture Components

```
┌─────────────────────────────────────────────────┐
│            FEDERATED LEARNING LAYER             │
├──────────────┬──────────────┬──────────────────┤
│ TRAIN_REQUEST│  Gradient    │  Proof of        │
│ Opcode 0x14  │ Aggregator   │  Training (PoT)  │
└──────────────┴──────────────┴──────────────────┘
        │              │              │
        ▼              ▼              ▼
   Create Job    On-chain FedAvg   ZK Verify
```

### 3. TRAIN_REQUEST Opcode (0x14)

**Input Parameters:**

- `model_id` (bytes32): IPFS CID of base model
- `dataset_ref` (bytes32): IPFS reference to dataset descriptor
- `total_rounds` (uint256): Number of training rounds
- `min_participants` (uint64): Minimum trainers per round
- `hyperparams` (bytes): Encoded training config (lr, epochs, batch_size)

**Output:**

- `job_id` (bytes32): Unique training job identifier

### 4. Proof of Training (PoT)

Random checkpoint sampling to prevent gradient fabrication:

- Sample intermediate model states during training
- Require ZK proofs of correct forward/backward pass
- Slash participants submitting invalid proofs

### 5. Training Economics

- **Emission allocation**: 5% of block rewards to training pool
- **Per-round rewards**: Distributed proportionally to contribution
- **Slashing**: 10% of stake for invalid proofs

## Consequences

### Positive

- First-mover advantage in decentralized training
- Complete AI stack (Inference + Verification + Training)
- New revenue stream for miners

### Negative

- Increased complexity in consensus
- Higher bandwidth requirements
- Research risk (PoT is novel)

### Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| PoT escape attack | Conservative slashing + insurance fund |
| Gradient poisoning | Byzantine-robust aggregation in v2 |
| Bandwidth overload | Gradient compression + sparse updates |

## References

- [FedAvg Paper](https://arxiv.org/abs/1602.05629)
- [Gradient Compression Survey](https://arxiv.org/abs/2010.12252)
- [Byzantine-Robust FL](https://arxiv.org/abs/1703.02757)
