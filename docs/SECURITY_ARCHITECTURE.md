# ModernTensor Security Architecture

**Comprehensive security model and anti-cheat mechanisms**

---

## Security Overview

ModernTensor implements a **4-layer security model** to protect against various attack vectors in decentralized AI networks.

```mermaid
graph TB
    subgraph "Security Layers"
        L1[Layer 1: Cryptographic Security]
        L2[Layer 2: Economic Security]
        L3[Layer 3: Consensus Security]
        L4[Layer 4: Behavioral Security]
    end

    L1 --> L2 --> L3 --> L4

    style L1 fill:#e74c3c
    style L2 fill:#f39c12
    style L3 fill:#3498db
    style L4 fill:#2ecc71
```

---

## Layer 1: Cryptographic Security

### Key Technologies

| Component | Algorithm | Purpose |
|-----------|-----------|---------|
| **Signatures** | ECDSA secp256k1 | Transaction authentication |
| **Hashing** | Keccak256 | Block hashing, commit hashes |
| **Key Derivation** | BIP39/BIP32 | HD wallet generation |
| **Address Format** | Ethereum-compatible | 20-byte addresses |

### Transaction Security Flow

```mermaid
sequenceDiagram
    participant W as Wallet
    participant T as Transaction
    participant V as Verifier

    W->>W: Generate Private Key
    W->>T: Sign(hash(tx), privateKey)
    T->>V: Submit Signed TX
    V->>V: Recover Public Key
    V->>V: Verify Signature
    V->>V: Check Address Match

    alt Valid
        V->>T: Accept
    else Invalid
        V->>T: Reject
    end
```

---

## Layer 2: Economic Security

### Stake-Based Protection

| Mechanism | Description | Penalty |
|-----------|-------------|---------|
| **Minimum Stake** | Required to participate | None (barrier to entry) |
| **Slashing** | Penalty for misbehavior | Up to 100% stake |
| **Jailing** | Temporary suspension | Lost rewards |
| **Bonding Period** | Lock-up for unstaking | 7-day delay |

### Slashing Conditions

```mermaid
graph LR
    subgraph "Violations"
        V1[Double Signing]
        V2[No Reveal After Commit]
        V3[Weight Manipulation]
        V4[Prolonged Inactivity]
    end

    subgraph "Penalties"
        P1[100% Slash]
        P2[10% Slash]
        P3[50% Slash]
        P4[5% Slash]
    end

    V1 --> P1
    V2 --> P2
    V3 --> P3
    V4 --> P4
```

### Economic Attack Resistance

| Attack | Cost Model | Mitigation |
|--------|------------|------------|
| **51% Attack** | Acquire 51% stake (~$X million) | Stake distribution monitoring |
| **Sybil Attack** | N × minimum_stake | High stake requirements |
| **Bribery** | Must outbid honest rewards | Slashing > potential gain |

---

## Layer 3: Consensus Security

### Commit-Reveal Scheme

Prevents weight manipulation by hiding weights until all validators commit.

```mermaid
stateDiagram-v2
    [*] --> Idle

    Idle --> CommitPhase: Epoch Start

    state CommitPhase {
        [*] --> Collecting
        Collecting: hash(weights + salt)
        Collecting --> Committed: Submit Commit
    }

    CommitPhase --> RevealPhase: Commit Window Ends

    state RevealPhase {
        [*] --> Revealing
        Revealing: Submit (weights, salt)
        Revealing --> Verified: Hash Match
    }

    RevealPhase --> Finalization: Reveal Window Ends

    state Finalization {
        [*] --> Aggregating
        Aggregating: Stake-weighted average
        Aggregating --> Applied: Update Weights
    }

    Finalization --> [*]: Epoch End
```

### Multi-Validator Consensus

```mermaid
graph TB
    subgraph "Validators (N=7)"
        V1[V1: 20% stake]
        V2[V2: 18% stake]
        V3[V3: 15% stake]
        V4[V4: 12% stake]
        V5[V5: 10% stake]
        V6[V6: 13% stake]
        V7[V7: 12% stake]
    end

    subgraph "Consensus"
        AGG[Stake-Weighted Aggregation]
        THR[Threshold: 67%]
    end

    V1 & V2 & V3 & V4 & V5 & V6 & V7 --> AGG
    AGG --> THR
    THR --> |Achieved| FINAL[Finalized Weights]
```

---

## Layer 4: Behavioral Security (Anti-Cheat)

### Threat Model

| Threat | Description | Detection |
|--------|-------------|-----------|
| **Weight Copying** | Validator copies other's weights | Commit-reveal timing |
| **Free-Riding** | Miner doesn't compute, copies results | Cross-validator verification |
| **Validator Collusion** | Validators agree to cheat | Stake distribution monitoring |
| **Result Manipulation** | Miner sends fake results | Multi-validator scoring |

### Anti-Cheat Detection Flow

```mermaid
flowchart TB
    subgraph "Detection"
        D1[Weight Similarity Check]
        D2[Timing Analysis]
        D3[Performance Anomaly]
        D4[Network Pattern Analysis]
    end

    subgraph "Scoring"
        D1 --> S1[Similarity Score]
        D2 --> S2[Timing Score]
        D3 --> S3[Anomaly Score]
        D4 --> S4[Pattern Score]
    end

    subgraph "Decision"
        S1 & S2 & S3 & S4 --> AGG[Aggregate Score]
        AGG --> THR{Threshold?}
        THR -->|Above| SLASH[Slash & Report]
        THR -->|Below| OK[Clear]
    end
```

### Weight Copying Prevention

```python
# Commit Phase
commit_hash = keccak256(encode(weights) + salt)
submit_commit(subnet_id, commit_hash)

# Reveal Phase (after commit window)
submit_reveal(subnet_id, weights, salt)

# Verification
assert keccak256(encode(weights) + salt) == stored_commit_hash
```

---

## Security Audit Status

| Component | Audit Status | Findings |
|-----------|--------------|----------|
| **Core Cryptography** | ✅ Internal Review | 0 Critical |
| **Consensus Logic** | ✅ Internal Review | 0 Critical |
| **Smart Contracts** | 🔄 Pending External | - |
| **RPC Security** | ✅ Internal Review | 2 Medium (fixed) |
| **P2P Network** | ✅ Internal Review | 1 Low |

---

## Incident Response

### Security Incident Levels

| Level | Definition | Response Time |
|-------|------------|---------------|
| **Critical** | Funds at risk | <1 hour |
| **High** | Consensus disruption | <4 hours |
| **Medium** | Service degradation | <24 hours |
| **Low** | Minor issues | <7 days |

### Response Flow

```mermaid
flowchart LR
    DETECT[Detect Issue] --> ASSESS[Assess Severity]
    ASSESS --> |Critical| HALT[Halt Network]
    ASSESS --> |High| PATCH[Emergency Patch]
    ASSESS --> |Medium| PLAN[Plan Fix]
    ASSESS --> |Low| BACKLOG[Add to Backlog]

    HALT --> FIX[Fix & Test]
    PATCH --> FIX
    PLAN --> FIX

    FIX --> DEPLOY[Deploy Fix]
    DEPLOY --> POSTMORTEM[Post-Mortem]
```

---

## Best Practices for Participants

### For Validators

- Use hardware security keys for signing
- Run nodes in isolated environments
- Monitor for unusual network activity
- Never share private keys or seeds

### For Miners

- Verify validator authenticity
- Use encrypted connections
- Regular security updates
- Backup keys securely

### For Developers

- Follow secure coding guidelines
- Audit all smart contracts
- Use rate limiting on APIs
- Implement input validation

---

*Last Updated: January 2026*
