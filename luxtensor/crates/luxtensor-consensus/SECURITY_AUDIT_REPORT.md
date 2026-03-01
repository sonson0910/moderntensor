# Security Audit Report — `luxtensor-consensus`

**Crate**: `luxtensor-consensus` (~12,632 lines of Rust)
**Auditor**: GitHub Copilot (Claude Opus 4.6)
**Date**: 2025-07-18
**Scope**: All 27 source files in `src/`
**Method**: Manual static analysis of every source file

---

## Executive Summary

The `luxtensor-consensus` crate is the core consensus engine for a Layer 1 PoS blockchain with an AI/ML subnet economy. The codebase is overall **well-structured** with evidence of prior security review iterations (annotated fixes: MC-1, MC-5, MC-7, H-2, H-4, H-5, H-6, M-1, M-NEW-2, L-NEW-1, FIX-6). Critical areas like the Yuma consensus pipeline and VRF key management demonstrate strong deterministic integer arithmetic.

However, **several consensus-critical f64 operations remain** in reward distribution and scoring paths that can cause state divergence across nodes on different platforms. Additionally, there are access-control gaps in the fast finality module and a governance design weakness that could allow low-turnout capture.

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH     | 5 |
| MEDIUM   | 8 |
| LOW      | 6 |
| INFO     | 8 |

---

## CRITICAL Findings

### C-1: Consensus-Critical f64 Arithmetic in Reward Distribution

**File**: `src/reward_distribution.rs` (lines 335–370, 430–460, 540–570, 600–640)
**Category**: Consensus Safety / Reward Distribution
**Impact**: State divergence between nodes → chain split

The `distribute_by_score`, `distribute_by_score_with_gpu`, `distribute_by_epoch_stats`, and `distribute_to_infrastructure` methods all compute per-participant reward shares using `f64` division:

```rust
// reward_distribution.rs ~line 350
let scaled_scores: Vec<([u8; 20], u128)> = miners
    .iter()
    .map(|m| {
        let raw = (m.score / total_score) * PRECISION as f64;
        let share = if raw.is_finite() { raw.clamp(0.0, PRECISION as f64) as u128 } else { 0 };
        (m.address, share)
    })
    .collect();
```

**Problem**: IEEE 754 `f64` operations do **not** guarantee identical results across platforms (x86 vs ARM, different compiler optimization levels, different instruction sets like SSE vs AVX). Since reward distribution is executed by every node to validate blocks, any divergence in the `u128` cast means nodes disagree on account balances, eventually causing a consensus fork.

The Yuma consensus module (`yuma_consensus.rs`) correctly avoids f64 entirely — this pattern should be replicated here.

**NaN risk**: While `is_finite()` guards are present, `total_score` being 0.0 for a non-empty miner list (all scores = 0.0) is guarded but `f64::sum()` on large inputs can lose precision differently across platforms even for the same inputs due to reordering (HashMap iteration order is non-deterministic).

**Recommendation**:
1. Convert `MinerInfo.score` from `f64` to `u32` (BPS) or `u64` (high-precision BPS).
2. Replace all `f64` proportional calculations with integer BPS arithmetic matching the Yuma consensus pattern.
3. If f64 must be retained for compatibility, canonicalize all miner lists by deterministic sort before summation, and use fixed-point intermediaries (multiply score by 10^12, convert to u128, then divide).

---

### C-2: Consensus-Critical f64 in Scoring Module Epoch Stats

**File**: `src/scoring.rs` (line ~550–570)
**Category**: Consensus Safety / Weight Consensus

The `get_all_miner_epoch_stats` method uses f64 division to produce scores consumed by reward distribution:

```rust
// scoring.rs — approximate
m.score as f64 / self.config.max_score as f64
```

This generates the `MinerEpochStats.base_score` value that feeds into `distribute_by_epoch_stats()` (C-1). Since `max_score` may vary per configuration and the f64 division is non-deterministic across platforms, this creates a cascading consensus divergence risk.

**Recommendation**: Use integer BPS scores throughout. Example: `score_bps = m.score * 10_000 / self.config.max_score`.

---

## HIGH Findings

### H-1: Fast Finality Missing Active-Validator Check

**File**: `src/fast_finality.rs` (lines ~100–150, `add_signature`)
**Category**: Consensus Safety / PoS Vulnerabilities

The `add_signature` method calls `validator_set.get_validator(&signer)` to retrieve the validator and its stake, but does **not** check the `active` flag:

```rust
// fast_finality.rs add_signature
let validator = self.validator_set.read()
    .get_validator(&signer)
    .ok_or_else(|| ConsensusError::ValidatorNotFound(...))?;
let stake = validator.stake;
// No check: if !validator.active { return Err(...) }
```

**Impact**: A deactivated, jailed, or exiting validator whose record still exists in `ValidatorSet` can cast finality signatures with full stake weight. Since slashing deactivates validators but doesn't remove them (MC-7 fix), a slashed validator retains the ability to participate in BFT finality until their entry is physically removed.

**Recommendation**: Add `if !validator.active { return Err(ConsensusError::ValidatorNotActive) }` immediately after retrieving the validator.

---

### H-2: Governance Approval Threshold Allows Low-Turnout Capture

**File**: `src/governance.rs` (lines 365–405, `finalize_voting`)
**Category**: Governance

The approval threshold is computed against `total_votes_cast` (for + against), not against `total_eligible_power`:

```rust
// governance.rs finalize_voting ~line 380
let approval_pct = if total_votes == 0 { 0u64 } else {
    for_power.checked_mul(10_000).ok_or(...)? / total_votes
};
if total_votes_cast_power >= quorum_power && approval_pct >= self.config.approval_threshold_bps {
    ProposalStatus::Approved
}
```

**Scenario**: With `quorum_bps = 3300` (33%) and `approval_threshold_bps = 6667` (66.67%):
- A proposal can pass with just 33% quorum turnout and 66.67% of those who voted approving
- Effective approval = 33% × 66.67% = **~22% of total eligible power**

This is a known governance design pattern (similar to real-world legislatures), but for an on-chain protocol governing economic parameters and slashing rules, it allows a coordinated minority to push through harmful parameter changes.

**Recommendation**: Consider computing approval against `total_eligible_power` rather than `total_votes`, or increase `quorum_bps` to at least 5000 (50%).

---

### H-3: View-Change Message `stake` Field is Caller-Supplied

**File**: `src/fast_finality.rs` (lines ~350–400, `add_view_change_vote`)
**Category**: PoS Vulnerabilities

The `ViewChangeMessage` struct contains a `stake: u128` field that is passed by the caller. Unlike the finality `add_signature` method which looks up stake from `ValidatorSet`, the view-change path trusts the caller-supplied stake value:

```rust
pub struct ViewChangeMessage {
    pub height: u64,
    pub new_view: u32,
    pub validator: Address,
    pub stake: u128,  // Caller-supplied — not verified against ValidatorSet
    pub timestamp: u64,
}
```

**Impact**: A malicious node could broadcast view-change messages with inflated stake, potentially triggering unwanted view changes and disrupting consensus.

**Recommendation**: Ignore `msg.stake` and look up the validator's actual stake from `self.validator_set.read().get_validator(&msg.validator)` inside `add_view_change_vote`.

---

### H-4: `check_and_update_state` TOCTOU in Circuit Breaker

**File**: `src/circuit_breaker.rs` (lines 265–275)
**Category**: Circuit Breaker

```rust
fn check_and_update_state(&self) {
    let state = *self.state.read();             // Read lock
    if state == CircuitState::Open {            // Check
        if let Some(opened_at) = *self.opened_at.read() {
            if opened_at.elapsed() >= self.config.open_duration {
                self.transition_to(CircuitState::HalfOpen);  // Write lock (separate)
            }
        }
    }
}
```

Between the `read()` check and `transition_to()` write, another thread could modify state. While `transition_to` has an idempotency guard (`old_state == new_state`), the `record_success` and `record_failure` methods also drop their write lock before calling `transition_to`, creating a window where state could be modified:

```rust
// record_success ~line 182
let state = self.state.write();
// ...
drop(state); // drop before transition_to which also takes write lock
self.transition_to(CircuitState::Closed);
```

**Impact**: In a high-concurrency scenario, a circuit breaker could briefly enter an inconsistent state (e.g., failure/success counts don't match the state). While this is local to each node and doesn't affect consensus, it could cause the AI layer circuit breaker to malfunction, allowing failing operations through or blocking healthy ones.

**Recommendation**: Merge `check_and_update_state` into a single write lock acquisition, and avoid dropping the lock before `transition_to` — instead update state in place.

---

### H-5: Weight Consensus Committee Selection Lacks Epoch Binding

**File**: `src/weight_consensus.rs` (lines 598–620, `select_committee`)
**Category**: Weight Consensus

```rust
pub fn select_committee(
    validators: &[Address],
    committee_size: usize,
    block_hash: &Hash,
    subnet_uid: u64,
) -> Vec<Address> {
    let mut hasher = Keccak256::new();
    hasher.update(block_hash);
    hasher.update(subnet_uid.to_le_bytes());
    // No epoch or height binding
```

The committee seed is `H(block_hash || subnet_uid)`. If the same `block_hash` appears at different heights (e.g., after a reorg or checkpoint replay), the same committee is selected. More practically, since `block_hash` changes every block this is unlikely, but adding `epoch` or `block_height` to the seed provides defense in depth.

**Recommendation**: Include the current epoch or block height in the hash: `hasher.update(current_epoch.to_le_bytes())`.

---

## MEDIUM Findings

### M-1: Dead Code in Halving Schedule

**File**: `src/halving.rs` (lines ~70–80)
**Category**: Economic Model

```rust
let effective_halvings = halvings.min(self.max_halvings);
// ^ clamps halvings to max_halvings

if halvings > self.max_halvings {
    // ^ This condition can NEVER be true after the .min() above
    return 0;
}
```

The `halvings` variable has been clamped to `max_halvings` by `.min()`, so the `if` branch is dead code. The intended behavior (return 0 after max halvings) still works because the subsequent `>> effective_halvings` shifts the reward below `minimum_reward`, triggering the minimum check. But the dead code is misleading.

**Recommendation**: Move the max-halvings check before the `.min()` call, or remove the dead branch.

---

### M-2: GovernanceModule Proposal Counter Not Persisted

**File**: `src/governance.rs` (line ~75)
**Category**: Governance

`proposal_id_counter: RwLock<u64>` starts at 0 and increments monotonically. After a node restart, it resets to 0, potentially reusing proposal IDs. While proposals are keyed by ID in a HashMap and old finalized proposals are garbage-collected, a reused ID could collide with an in-memory proposal that hasn't been GC'd.

**Recommendation**: Persist the counter to storage, or derive proposal IDs from a hash including the proposer and block height (similar to `WeightProposal.id`).

---

### M-3: Unbounded VTrust Score History

**File**: `src/weight_consensus.rs` (lines 435–500, `VTrustScorer`)
**Category**: Weight Consensus

The `VTrustScorer` stores `(aligned_count, total_count)` per validator address in a `HashMap<Address, (u64, u64)>`. While the counts themselves are u64, the **number of tracked validators** is unbounded. Over time, validators who exit the network still accumulate entries.

**Recommendation**: Add periodic pruning of validators not in the current `ValidatorSet`, or bound the map size.

---

### M-4: Infrastructure Rewards Silently Lost When No Nodes Registered

**File**: `src/reward_distribution.rs` (lines 590–600, `distribute_to_infrastructure`)
**Category**: Reward Distribution

```rust
fn distribute_to_infrastructure(&self, pool: u128, nodes: &[InfrastructureNodeInfo]) -> HashMap<[u8; 20], u128> {
    if nodes.is_empty() {
        // No infrastructure nodes registered — undistributed infra pool
        return rewards;
    }
```

The 2% infrastructure pool is allocated but returned empty when no infra nodes exist. The `reward_executor.rs` catches this and sends undistributed infra rewards to the DAO treasury:

```rust
// reward_executor.rs ~line 370
let infra_undistributed = infra_pool_total.saturating_sub(infra_distributed);
if infra_undistributed > 0 {
    *dao_bal = dao_bal.saturating_add(infra_undistributed);
}
```

However, `infra_pool_total` is recomputed as `total_distributed * 200 / 10_000`, which may not exactly match the original BPS calculation due to integer division rounding. This can cause a small (dust-level) discrepancy.

**Recommendation**: Pass `infra_pool` as a return value from `distribute()` instead of recomputing it.

---

### M-5: `NodeRegistry.register()` Double Lock Ordering

**File**: `src/node_tier.rs` (lines 310–330)
**Category**: Concurrency

```rust
pub fn register(&self, ...) -> Result<NodeTier, &'static str> {
    let mut nodes = self.nodes.write();         // Lock 1
    nodes.insert(address, node);
    self.nodes_by_tier.write()                   // Lock 2 (while holding Lock 1)
        .entry(tier).or_insert_with(Vec::new).push(address);
    drop(nodes);
```

While this ordering is consistent within `register()`, `get_by_tier()` acquires locks in the same order (`nodes → nodes_by_tier`), but `update_stake()` acquires `nodes.write()` then `nodes_by_tier.write()` — consistent. The explicit ordering is good, but `unregister()` acquires `nodes.write()` then `nodes_by_tier.write()` inside the same scope. If any code path ever takes these locks in reverse order, it will deadlock.

**Recommendation**: Document the lock order as a module-level invariant with a comment, and consider using a single `RwLock<(HashMap, HashMap)>` to eliminate the risk.

---

### M-6: Long-Range Protection Checkpoint Validation is O(n)

**File**: `src/long_range_protection.rs` (lines 150–170, `validate_against_checkpoints`)
**Category**: Performance / DoS

```rust
pub fn validate_against_checkpoints(&self, block_hash: Hash, height: u64) -> bool {
    let checkpoints = self.checkpoints.read();
    for cp in checkpoints.iter() {
        if cp.height == height {
            return cp.block_hash == block_hash;
        }
    }
    true
}
```

This performs a linear scan over all checkpoints for every block validation. With `checkpoint_interval=100` and a chain growing to millions of blocks, the checkpoint list grows unboundedly (thousands of entries). While `prune_old_checkpoints()` exists, it's never called automatically.

**Recommendation**: Use a `HashMap<u64, Checkpoint>` keyed by height for O(1) lookup, and auto-prune checkpoints in `update_finalized()`.

---

### M-7: `suggested_priority_fee` Uses f64

**File**: `src/eip1559.rs` (lines 195–210)
**Category**: Consensus Safety

```rust
pub fn suggested_priority_fee(&self) -> u128 {
    let usage_ratio = self.last_gas_used as f64 / self.config.target_gas_used as f64;
    // ...
}
```

While this is an RPC-facing helper and not consensus-critical (block validation uses `calculate_next_base_fee` which is integer-only), the method signatures suggest it could be confused with consensus code.

**Recommendation**: Add a `// NON-CONSENSUS: RPC helper only` comment, or convert to integer comparison: `if self.last_gas_used > self.config.target_gas_used * 3 / 2`.

---

### M-8: Rotation Module `slash_validator` Inconsistent Lock Usage

**File**: `src/rotation.rs` (lines 300–340)
**Category**: Concurrency / Slashing

`ValidatorRotation.slash_validator()` calls `self.current_validators.slash_stake()` where `current_validators` is a `ValidatorSet` (not behind `RwLock`). Meanwhile, `SlashingManager` holds `validator_set: Arc<RwLock<ValidatorSet>>` with documented lock ordering. If both `SlashingManager` and `ValidatorRotation` can slash the same validator concurrently, there's no shared lock coordination.

**Recommendation**: Ensure only one code path (either `SlashingManager` or `ValidatorRotation`) performs slashing, or gate `ValidatorRotation.slash_validator()` behind the same `Arc<RwLock<ValidatorSet>>` used by `SlashingManager`.

---

## LOW Findings

### L-1: `scoring.rs` Module Comment Contradicts Implementation

**File**: `src/scoring.rs` (header)
**Category**: Documentation

The module header states *"DETERMINISTIC: No f64 in consensus-critical paths"*, but `get_all_miner_epoch_stats` and `merge_yuma_output` use f64 division. While the consensus-critical Yuma pipeline itself avoids f64, the scoring output feeds into reward distribution which is consensus-critical.

**Recommendation**: Update the module header to clarify which methods are deterministic and which use f64.

---

### L-2: `fork_choice.rs` Combined Score Theoretical Overflow

**File**: `src/fork_choice.rs` (line ~130)
**Category**: Fork Choice

```rust
let combined = depth as u128 * DEPTH_WEIGHT + attestation_stake / STAKE_DIVISOR;
```

For `DEPTH_WEIGHT = 1_000_000_000_000` and `depth` as `u128`, the multiplication overflows when `depth > u128::MAX / 10^12 ≈ 3.4 × 10^26 blocks`. This is practically unreachable, but a `checked_mul` would be more defensive.

**Recommendation**: Use `depth.checked_mul(DEPTH_WEIGHT).unwrap_or(u128::MAX)` for defense-in-depth.

---

### L-3: `validator.rs` `saturating_sub` Masks Potential Logic Errors

**File**: `src/validator.rs` (line ~280, `update_stake`)
**Category**: PoS Vulnerabilities

```rust
self.total_stake = self.total_stake.saturating_sub(old_stake).saturating_add(new_stake);
```

If `old_stake > total_stake` due to a prior bug, `saturating_sub` silently produces 0 instead of panicking, masking the accounting error. In a consensus system, silent data corruption is worse than a crash.

**Recommendation**: Use `checked_sub` and return an error if the invariant is violated.

---

### L-4: `commit_reveal.rs` Non-Revealer Detection Has No Grace Period

**File**: `src/commit_reveal.rs` (lines ~400–450, `finalize_epoch_with_slashing`)
**Category**: Slashing

Validators who commit weights but fail to reveal within the reveal window are immediately slashed. There is no grace period for network partitions or temporary outages. A validator that commits, then experiences a brief connectivity loss during the reveal window, loses 5% of their stake.

**Recommendation**: Consider a multi-epoch grace period (e.g., allow reveal in the next epoch at a penalty rather than full slash).

---

### L-5: `economic_model.rs` Uses `f64` for Supply Projection

**File**: `src/economic_model.rs` (throughout)
**Category**: Economic Model

The `project_supply`, `analyze_equilibrium`, and `sweep_*` functions use extensive f64 arithmetic. These are analysis/reporting functions, not consensus-critical, but the `inflation_rate_pct` and `circulating_supply` values fed to `validate_parameters` influence cross-module parameter checks.

**Recommendation**: Mark these functions clearly as `// NON-CONSENSUS: analysis only` and ensure they are never called from block validation paths.

---

### L-6: `burn_manager.rs` Half-Prune Loses Ordering Information

**File**: `src/burn_manager.rs` (line ~180)
**Category**: Economic Model

When burn events exceed `MAX_BURN_EVENTS`, the oldest half is drained. The remaining events maintain their relative order, but the total-burned counters are not affected. This is fine for accounting, but means historical queries for specific burn events may return incomplete results.

**Recommendation**: Document this behavior in the public API, or use a separate counter for total burns independent of the event log.

---

## INFO Findings

### I-1: Testnet Pseudo-VRF Correctly Gated

**File**: `src/pos.rs` (lines ~200–300)
**Category**: VRF/RANDAO

The testnet keccak256-based pseudo-VRF is correctly compiled only when `#[cfg(not(feature = "production-vrf"))]`. Production builds use ECVRF-EDWARDS25519-SHA512-TAI via `vrf_key.rs`. The `vrf_key.rs` implementation is well-structured with proper proof generation, verification, and serialization.

---

### I-2: Prior Audit Fixes Well-Documented

Multiple annotated security fixes throughout the codebase demonstrate an ongoing security review culture:
- **MC-1**: Atomic seed reads in PoS (`pos.rs`)
- **MC-5**: Consistent snapshot reads in allocation stats (`token_allocation.rs`)
- **MC-7**: Zero-stake ghost validator deactivation (`validator.rs`)
- **H-2**: Voting power sourced from ValidatorSet, not caller (`governance.rs`)
- **H-4**: RANDAO mix_lock serialization (`randao.rs`)
- **H-5**: GPU self-declaration removed, fork_choice TOCTOU fix
- **H-6**: Governance vote TOCTOU, RANDAO last-look-bias
- **M-1**: Governance arithmetic overflow protection
- **M-NEW-2**: Collusion detection bounded pair scanning
- **L-NEW-1**: VTrust persistence support

---

### I-3: Yuma Consensus Deterministic Design

**File**: `src/yuma_consensus.rs`

The Yuma consensus module is the gold standard within this codebase for deterministic arithmetic. All operations use u128 BPS with explicit rounding remainder distribution. The median-clip algorithm uses `u128.cmp()` (total ordering, no NaN). Row normalization assigns the rounding remainder to the largest-weight slot deterministically.

---

### I-4: BFT Fast Finality Equivocation Detection

**File**: `src/fast_finality.rs` (line ~120)

The `height_votes` map (`HashMap<u64, HashMap<Address, Hash>>`) correctly tracks which block hash each validator voted for at each height, detecting equivocation (voting for different blocks at the same height). This is properly pruned alongside signatures.

---

### I-5: RANDAO Commit-Reveal Anti-Bias

**File**: `src/randao.rs` and `src/pos.rs`

The `prev_epoch_randao` lookback mechanism (FIX-6) prevents last-look bias by using the RANDAO mix from the **previous** epoch for validator selection, forcing validators to commit before knowing the current epoch's entropy. The `mix_lock` Mutex (H-4 fix) serializes all reveal operations to prevent TOCTOU races.

---

### I-6: Lock Ordering Documentation in Slashing

**File**: `src/slashing.rs` (comments)

The documented lock order (`validator_set → jailed → slash_history`) is consistently followed in both `slash()` and `process_unjail()`. This is a mature concurrency pattern.

---

### I-7: Proper Bounds on Growth-Prone Data Structures

Multiple modules implement bounded data structures to prevent OOM:
- `MAX_EVIDENCE_ENTRIES = 10,000` with median-height pruning (slashing)
- `MAX_SLASH_HISTORY = 50,000` with drain-oldest (slashing)
- `MAX_BURN_EVENTS = 100,000` with half-drain (burn_manager)
- `MAX_TRACKED_BLOCKS = 10,000` with auto-prune (fork_choice)
- `MAX_REWARD_HISTORY_PER_ADDRESS = 1000` (reward_executor)
- `MAX_ACTIVE_PROPOSALS = 100` (governance)
- `MAX_APPLIED_HISTORY = 1000` (weight_consensus)
- Ring buffer per voter in VotingPatternTracker
- `MAX_PAIRS_TO_SCAN = 5000` in collusion detection

---

### I-8: EIP-1559 Fee Market Integer Arithmetic

**File**: `src/eip1559.rs`

The core `calculate_next_base_fee` method uses only integer arithmetic (`u128`, `u64`) with proper `checked_mul` and `saturating_add/sub`. The min/max base fee clamping prevents both zero-fee spam and extreme fee spikes. Only the non-consensus `suggested_priority_fee` helper uses f64 (see M-7).

---

## Summary of Recommendations by Priority

### Immediate (Before Mainnet)

1. **C-1, C-2**: Eliminate all f64 from consensus-critical reward distribution and scoring. Convert to integer BPS arithmetic matching the Yuma module pattern.
2. **H-1**: Add `validator.active` check in `fast_finality.rs::add_signature()`.
3. **H-3**: Look up view-change voter stake from `ValidatorSet` instead of trusting `msg.stake`.

### Short-Term

4. **H-2**: Evaluate governance quorum/threshold design. Consider computing approval against total eligible power.
5. **H-5**: Add epoch/height to committee selection seed.
6. **M-1**: Fix dead code branch in halving.
7. **M-2**: Persist governance proposal counter.
8. **M-4**: Pass infra pool amount directly instead of recomputing.
9. **M-8**: Unify slash path through a single lock-coordinated interface.

### Medium-Term

10. **M-3**: Add VTrust score pruning for exited validators.
11. **M-5**: Document lock ordering as module-level invariant in node_tier.
12. **M-6**: Use HashMap for checkpoint lookups.
13. **L-3**: Replace `saturating_sub` with `checked_sub` in total_stake accounting.
14. **L-4**: Consider grace period for commit-reveal non-revealers.
15. **H-4**: Consolidate circuit breaker state transitions under a single write lock.

---

## Files Reviewed

| File | Lines | Category |
|------|-------|----------|
| `Cargo.toml` | 35 | Dependencies |
| `lib.rs` | 130 | Module declarations |
| `error.rs` | 82 | Error types |
| `pos.rs` | 810 | Core PoS + VRF |
| `validator.rs` | 365 | Validator set |
| `scoring.rs` | 611 | Miner/validator scoring |
| `economic_model.rs` | 1253 | Tokenomics analysis |
| `emission.rs` | 372 | Emission controller |
| `halving.rs` | 296 | Halving schedule |
| `token_allocation.rs` | 477 | TGE + vesting |
| `reward_distribution.rs` | 980 | Reward distribution |
| `reward_executor.rs` | 665 | Epoch processing |
| `slashing.rs` | 538 | Slashing + jailing |
| `burn_manager.rs` | 300 | Token burning |
| `governance.rs` | 758 | On-chain governance |
| `fork_choice.rs` | 563 | GHOST fork choice |
| `fork_resolution.rs` | 474 | Reorg detection |
| `fast_finality.rs` | 896 | BFT finality |
| `randao.rs` | 497 | RANDAO entropy |
| `commit_reveal.rs` | 662 | Weight commit-reveal |
| `rotation.rs` | 484 | Validator rotation |
| `circuit_breaker.rs` | 577 | Circuit breaker |
| `liveness.rs` | 377 | Liveness monitoring |
| `long_range_protection.rs` | 419 | Long-range attack prevention |
| `node_tier.rs` | 515 | Node tier system |
| `weight_consensus.rs` | 1463 | Multi-validator weight consensus |
| `yuma_consensus.rs` | 627 | Yuma consensus (SAC) |
| `eip1559.rs` | ~330 | Dynamic fee pricing |
| `vrf_key.rs` | 377 | Production VRF keys |

**Total**: ~12,632 lines reviewed
