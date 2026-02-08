# LuxTensor Storage & State Management Audit Report

**Date:** 2026-02-08
**Scope:** `luxtensor-storage`, `luxtensor-core` (state/types/block/transaction/account/unified_state), `luxtensor-node` (service/genesis/shutdown)
**Auditor:** Storage & State Management Security Audit

---

## Executive Summary

The LuxTensor storage layer is built on RocksDB with column families for namespace separation, uses `WriteBatch` for atomic block writes, and features a proper Merkle Patricia Trie for state root computation. The architecture is generally sound, but there are **several critical and high-severity issues** around state atomicity, dual StateDB confusion, unbounded caches, missing reorg support, and key collision risks.

---

## FINDINGS

### [CRITICAL-01] Non-Atomic State Commit During Block Production

**File:** `luxtensor-node/src/service.rs` lines 1466–1536
**Severity:** CRITICAL

Block production performs **three separate write operations** that are not wrapped in a single atomic batch:

1. `state.commit()` → writes dirty accounts to RocksDB via `WriteBatch` (state_db.rs)
2. `state.flush_to_db(storage)` → writes `state:account:` prefixed keys individually via `db.put()`
3. `storage.store_block(&block)` → writes block + headers + tx index via `WriteBatch`

A crash between step 1 and step 3 leaves the database with **updated account state but no corresponding block**, causing state/block height mismatch on recovery. Similarly, a crash between step 2 and step 3 means account data is persisted under two different key schemes without the block to explain it.

**Recommendation:** Combine all state writes and block storage into a **single `WriteBatch`** committed atomically. Alternatively, implement a write-ahead log (WAL) with two-phase commit.

---

### [CRITICAL-02] Dual StateDB Architecture Creates State Divergence Risk

**Files:** `luxtensor-core/src/state.rs` (in-memory `StateDB`) vs `luxtensor-storage/src/state_db.rs` (RocksDB-backed `StateDB`)
**Severity:** CRITICAL

There are **two completely different StateDB implementations**:

| | `core::StateDB` | `storage::StateDB` |
|---|---|---|
| Backend | `HashMap<Address, Account>` (in-memory only) | `Arc<DB>` (RocksDB) + `HashMap` cache |
| Persistence | Via `flush_to_db()` with `state:account:` prefix | Direct `db.put(address.as_bytes(), ...)` |
| Key format | `state:account:<20-byte addr>` | Raw `<20-byte addr>` |
| Root computation | `MerkleTree` (simple binary) | `MerkleTrie` (Merkle Patricia) |
| Exports | `luxtensor_core::StateDB` | `luxtensor_storage::StateDB` |

The node uses `core::StateDB` for block production and `flush_to_db()` to persist with `state:account:` prefix keys. The `storage::StateDB` writes with raw address keys. **If both are used on the same DB, data exists under two different key formats** with potentially divergent state roots (different Merkle algorithms).

**Recommendation:** Eliminate one StateDB implementation. The `storage::StateDB` with its proper MPT trie should be the canonical one. Remove or deprecate `core::StateDB`.

---

### [CRITICAL-03] State Root Non-Determinism Risk in `core::StateDB`

**File:** `luxtensor-core/src/state.rs` lines 60–82
**Severity:** CRITICAL

The `root_hash()` method in `core::StateDB` iterates `self.cache` (a `HashMap`) to build the Merkle tree. While elements are sorted by address before hashing, the vector_store root is also incorporated:

```rust
let vector_root = self.vector_store.root_hash();
```

The HNSW vector store is a mutable, probabilistic data structure. If its internal state differs across nodes (due to insertion order, random seeds, or floating-point differences), **different nodes will compute different state roots for the same logical state**, breaking consensus.

**Recommendation:** The HNSW vector store root should NOT be included in the consensus state root unless its determinism is formally proven. Separate it into a non-consensus auxiliary root.

---

### [HIGH-01] No Reorg/Rollback Support for Persisted State

**Files:** `luxtensor-storage/src/state_db.rs`, `luxtensor-storage/src/db.rs`
**Severity:** HIGH

The `storage::StateDB::rollback()` method only reverts **in-memory** dirty accounts by removing them from cache:

```rust
pub fn rollback(&self) {
    let dirty = self.dirty.read();
    let mut cache = self.cache.write();
    for address in dirty.iter() {
        cache.remove(address);
    }
    ...
}
```

Once `commit()` has been called (writing to RocksDB), there is **no mechanism to reverse persisted state**. This means:
- Chain reorganizations cannot revert account state
- If an invalid block is accidentally committed, the state is permanently corrupted
- No snapshot/journal system exists for state rollback

The `BlockchainDB` also lacks any `revert_block()` or `delete_block_at_height()` method.

**Recommendation:** Implement state journaling (undo log) that records previous values before each state transition. On reorg, replay the undo log to restore prior state. Consider storing state diffs per block.

---

### [HIGH-02] `StateDB` Account Cache is Unbounded (`HashMap`)

**File:** `luxtensor-storage/src/state_db.rs` line 20
**Severity:** HIGH

```rust
pub struct StateDB {
    db: Arc<DB>,
    cache: RwLock<HashMap<Address, Account>>,  // UNBOUNDED
    dirty: RwLock<HashSet<Address>>,
    contract_code: RwLock<HashMap<Hash, Vec<u8>>>,  // UNBOUNDED
    dirty_codes: RwLock<HashSet<Hash>>,
}
```

Both `cache` and `contract_code` are unbounded `HashMap`s. As the blockchain grows:
- Every unique account ever accessed stays in memory forever
- Every contract bytecode loaded stays cached indefinitely
- With millions of accounts, this could exhaust available RAM

The `commit()` method compounds this: it inserts **all cached accounts** into a fresh `MerkleTrie`:
```rust
for (address, account) in cache.iter() {
    // builds O(n) trie EVERY commit
}
```

**Recommendation:** Replace `HashMap` caches with bounded LRU caches. Only dirty accounts should be in the trie; use an incremental trie that persists between commits.

---

### [HIGH-03] `commit()` Rebuilds Entire Merkle Trie From Scratch on Every Block

**File:** `luxtensor-storage/src/state_db.rs` lines 268–295
**Severity:** HIGH (Performance + Correctness)

```rust
pub fn commit(&self) -> Result<Hash> {
    // ... write dirty accounts to RocksDB ...

    let mut trie = MerkleTrie::new();  // NEW trie every time

    for (address, account) in cache.iter() {  // ALL accounts, not just dirty
        let key = keccak256(address.as_bytes());
        let value = bincode::serialize(account)?;
        trie.insert(&key, &value)?;
    }

    let state_root = trie.root_hash();
    Ok(state_root)
}
```

Problems:
1. **O(n) per block** where n = total accounts ever touched, not just modified
2. Trie is discarded after each commit, losing all structure
3. Only accounts in the cache are included — accounts that were evicted or never loaded are **excluded from the state root**, making it depend on access patterns rather than actual state

**Recommendation:** Persist the trie to RocksDB (trie node database). On commit, only update dirty paths. This brings it to O(k log n) where k = dirty accounts.

---

### [HIGH-04] Receipt Pruning is O(n·m) and Non-Atomic

**File:** `luxtensor-storage/src/db.rs` lines 447–470
**Severity:** HIGH

```rust
pub fn prune_receipts_before_height(&self, before_height: u64) -> Result<usize> {
    for height in 0..before_height {
        let block = match self.get_block_by_height(height)? { ... };
        for tx in &block.transactions {
            let tx_hash = tx.hash();
            if self.db.get_cf(cf, &tx_hash)?.is_some() {
                self.db.delete_cf(cf, &tx_hash)?;  // Individual delete, NOT batched
            }
        }
    }
}
```

- Each receipt is deleted individually (not via `WriteBatch`)
- A crash mid-pruning leaves partially-pruned state
- Iterating all blocks from 0 to height is extremely slow for mature chains
- Blocks themselves are loaded from disk for each height

**Recommendation:** Use `WriteBatch` for atomic pruning. Consider maintaining a secondary index (height → receipt_keys) to avoid full block deserialization.

---

### [HIGH-05] Block Storage Allows Height Overwrite (No Orphan Handling)

**File:** `luxtensor-storage/src/db.rs` lines 85–118
**Severity:** HIGH

`store_block()` unconditionally writes `height → hash` mapping:

```rust
batch.put_cf(cf_height, height.to_be_bytes(), block_hash);
```

If two blocks arrive at the same height (fork scenario), the **later write silently overwrites the earlier block's height index**. The earlier block's data still exists in the `blocks` and `headers` CFs but becomes unreachable by height. There is:
- No orphan block tracking
- No fork detection at the storage level
- No way to enumerate all blocks at a given height

The node-level `compare_exchange` on `best_height` (service.rs ~line 700) is a mitigation but only prevents race conditions, not legitimate forks.

**Recommendation:** Store height → hash as a **set** (e.g., `height:hash → block_hash`), or maintain a canonical chain pointer separate from a block-by-height index.

---

### [MEDIUM-01] Contract Code Not Committed in `StateDB::commit()`

**File:** `luxtensor-storage/src/state_db.rs` lines 268–295
**Severity:** MEDIUM

The `commit()` method writes dirty **accounts** to RocksDB but **never writes dirty contract codes**:

```rust
pub fn commit(&self) -> Result<Hash> {
    let dirty = self.dirty.read();
    let cache = self.cache.read();
    let mut batch = rocksdb::WriteBatch::default();

    for address in dirty.iter() {
        if let Some(account) = cache.get(address) {
            let bytes = bincode::serialize(account)?;
            batch.put(address.as_bytes(), bytes);
        }
    }
    self.db.write(batch)?;
    // NOTE: dirty_codes is NEVER flushed!
    ...
}
```

The `dirty_codes` set and `contract_code` HashMap are populated by `set_contract_code()` but never persisted to RocksDB in `commit()`. Contract bytecode stored via the `CONTRACT_CODE_PREFIX` key format is only written if someone explicitly calls the RocksDB write path elsewhere.

**Recommendation:** Add contract code flushing to `commit()`:
```rust
let dirty_codes = self.dirty_codes.read();
let codes = self.contract_code.read();
for code_hash in dirty_codes.iter() {
    if let Some(code) = codes.get(code_hash) {
        let mut key = CONTRACT_CODE_PREFIX.to_vec();
        key.extend_from_slice(code_hash);
        batch.put(&key, code);
    }
}
```

---

### [MEDIUM-02] MetagraphDB Uses Individual Puts (Not WriteBatch) for Most Operations

**File:** `luxtensor-storage/src/metagraph_store.rs`
**Severity:** MEDIUM

Most metagraph operations (`store_subnet`, `store_neuron`, `store_stake`, `store_delegation`, `register_validator`) use individual `db.put_cf()` calls. Only `store_weights()` uses `WriteBatch`.

For operations that update multiple related entities (e.g., registering a validator and updating their stake), non-atomic writes can leave inconsistent state if a crash occurs between writes.

**Recommendation:** Provide batch-write methods for compound operations, especially for epoch-boundary state transitions.

---

### [MEDIUM-03] Missing WAL Sync Configuration for RocksDB

**File:** `luxtensor-storage/src/db.rs` lines 36–48
**Severity:** MEDIUM

RocksDB is opened with default write options. The default `WriteOptions` has `sync = false`, meaning writes are buffered in the OS page cache and not fsynced to disk. A power failure (not just process crash) could lose recent writes.

```rust
let db = DB::open_cf_descriptors(&opts, path, cfs)?;
// No WriteOptions::set_sync(true) anywhere
```

**Recommendation:** For critical writes (block storage, state commits), use `WriteOptions { sync: true }` to ensure durability. Alternatively, configure RocksDB WAL with `set_wal_dir()` and appropriate flush policies.

---

### [MEDIUM-04] Key Collision Risk Between `state:account:` and Raw Address Keys

**Files:** `luxtensor-core/src/state.rs` line 6, `luxtensor-storage/src/state_db.rs` line 51
**Severity:** MEDIUM

`core::StateDB::flush_to_db()` writes keys as `state:account:<20-byte address>`.
`storage::StateDB::commit()` writes keys as raw `<20-byte address>`.

Both write to the **default column family** of the same RocksDB instance (via `BlockchainDB::inner_db()`). While the different prefixes prevent direct key collision, it means account data is stored **twice** under different key schemes. This wastes space and creates risk of stale data if only one path is updated.

Additionally, `StateDB` in `state_db.rs` also uses the default CF for:
- Account data: raw 20-byte address keys
- Contract code: `code:<32-byte hash>` keys
- HNSW indexes: `hnsw:<name>` keys

These share the same column family and rely solely on prefix to avoid collision. While safe in practice (different key lengths), this violates the separation-of-concerns principle of column families.

**Recommendation:** Move contract code and HNSW storage to dedicated column families. Eliminate the dual key scheme for account data.

---

### [MEDIUM-05] `MerkleTrie::delete()` Rebuilds Entire Trie

**File:** `luxtensor-storage/src/trie.rs` lines 464–477
**Severity:** MEDIUM

```rust
pub fn delete(&mut self, key: &[u8]) -> Result<()> {
    if self.keys.remove(key).is_some() {
        let entries: Vec<(Vec<u8>, Vec<u8>)> = self.keys.drain().collect();
        self.root = TrieNode::Empty;
        for (k, v) in &entries {
            // Re-insert ALL remaining keys
        }
    }
    Ok(())
}
```

Deleting a single key rebuilds the entire trie from scratch (O(n) insertions). For a trie with millions of entries, this is prohibitively expensive.

**Recommendation:** Implement proper MPT node deletion that restructures only the affected path (collapsing branch→extension/leaf as needed).

---

### [MEDIUM-06] Trie Maintains Redundant `keys` HashMap

**File:** `luxtensor-storage/src/trie.rs` lines 393–396
**Severity:** MEDIUM

```rust
pub struct MerkleTrie {
    root: TrieNode,
    keys: HashMap<Vec<u8>, Vec<u8>>,  // Duplicates all data
}
```

Every key-value pair is stored **both** in the trie nodes and in the `keys` HashMap. This doubles memory usage and exists solely to support (a) `get_all_keys()` iteration and (b) the `delete()` rebuild. If the trie stored millions of accounts, this overhead is significant.

**Recommendation:** Implement trie iteration directly and proper deletion to eliminate the shadow map.

---

### [MEDIUM-07] No Database Close / Flush on Shutdown

**File:** `luxtensor-node/src/graceful_shutdown.rs`
**Severity:** MEDIUM

The `GracefulShutdown::execute_shutdown_sync()` saves a JSON checkpoint but does **not** explicitly flush or close the RocksDB instance. RocksDB's `Drop` implementation does call `Close()`, but relying on implicit drop ordering in a multi-threaded async context risks:
- Memtables not flushed to SST files
- WAL not synced
- Column family handles used after DB close

**Recommendation:** Explicitly call `DB::flush()` and ensure the `Arc<DB>` reference count drops to zero before process exit. Add `flush_wal(true)` to the shutdown sequence.

---

### [LOW-01] `get_best_height()` Iterates From End — Correct but Fragile

**File:** `luxtensor-storage/src/db.rs` lines 211–225
**Severity:** LOW

```rust
pub fn get_best_height(&self) -> Result<Option<u64>> {
    let mut iter = self.db.iterator_cf(cf_height, rocksdb::IteratorMode::End);
    if let Some(Ok((key, _))) = iter.next() {
        // parse height from key
    }
}
```

This works because `u64::to_be_bytes()` preserves sort order in RocksDB's lexicographic ordering. However, there is no cached "best height" value, so every call requires an iterator seek. This is called frequently in block production.

**Recommendation:** Cache the best height in an `AtomicU64` and update it on `store_block()`.

---

### [LOW-02] Checkpoint Import Path Traversal Validation Has TOCTOU Race

**File:** `luxtensor-storage/src/checkpoint.rs` lines 315–340
**Severity:** LOW

The checkpoint import validates paths against traversal attacks, but there's a TOCTOU (time-of-check-time-of-use) gap:

```rust
let canonical_snapshot = snapshot_path.canonicalize()?;
// ... for each entry:
if let Ok(canonical_target) = target.canonicalize() {
    if !canonical_target.starts_with(&canonical_snapshot) {
        return Err(...);
    }
}
entry.unpack_in(&snapshot_path)?;  // Race: directory could change between check and unpack
```

The check on `canonical_target` succeeds only if the target already exists (for `canonicalize()` to work). For new files, `canonicalize()` fails and the check is bypassed (the `if let Ok(...)` silently passes on error).

**Recommendation:** Use `Path::starts_with()` on the non-canonicalized path after stripping `..` components, or validate that no `..` exists in the relative path (which the code partially does earlier).

---

### [LOW-03] `EVM_STORAGE` Prefix Scan Relies on RocksDB Prefix Behavior

**File:** `luxtensor-storage/src/db.rs` lines 550–565
**Severity:** LOW

`get_all_evm_storage_for_address()` uses `prefix_iterator_cf()` with a 20-byte address prefix. RocksDB's prefix iterator behavior depends on whether a prefix extractor is configured. Without one, the iterator may scan the entire CF.

**Recommendation:** Configure a `SliceTransform` prefix extractor on the `evm_storage` CF for the first 20 bytes, or use `iterator_cf()` with bounds.

---

### [LOW-04] Genesis Block State Root is `[0u8; 32]`

**File:** `luxtensor-core/src/block.rs` lines 136–145
**Severity:** LOW

```rust
pub fn genesis() -> Self {
    let header = BlockHeader {
        state_root: [0u8; 32],  // Not computed from actual genesis state
```

The genesis block's `state_root` is hardcoded to zeros regardless of genesis account configuration. If genesis accounts are pre-funded (dev mode), the state root doesn't reflect the actual state, making state proof verification impossible for genesis state.

**Recommendation:** Compute genesis state root from the actual genesis account balances before creating the genesis block.

---

### [LOW-05] Delegation Storage Only Supports One Delegation Per Delegator

**File:** `luxtensor-storage/src/metagraph_store.rs` lines 488–503
**Severity:** LOW

```rust
pub fn store_delegation(&self, data: &DelegationData) -> Result<()> {
    self.db.put_cf(&cf, data.delegator, value)  // Key = delegator address
}
```

Using the delegator address as the key means a delegator can only have **one** delegation entry. A second delegation overwrites the first. This is a design limitation that prevents split-delegation across multiple validators.

**Recommendation:** Use a composite key: `delegator(20) || validator(20)` to support multiple delegations per delegator.

---

### [LOW-06] No Log/Event Indexing

**Files:** `luxtensor-storage/src/db.rs`, `luxtensor-core/src/receipt.rs`
**Severity:** LOW

`Receipt` contains `logs: Vec<Log>` with `address`, `topics`, and `data`, but there is no secondary index for:
- Logs by contract address
- Logs by topic (event signature)
- Logs by block range

The RPC cannot efficiently serve `eth_getLogs` filter queries without scanning all receipts.

**Recommendation:** Add column families for log indexing: `CF_LOG_BY_ADDRESS`, `CF_LOG_BY_TOPIC0`, with keys that include block height for range queries.

---

## Summary Matrix

| ID | Severity | Category | Status |
|---|---|---|---|
| CRITICAL-01 | CRITICAL | Data Corruption | Non-atomic multi-step block commit |
| CRITICAL-02 | CRITICAL | Architecture | Dual StateDB with divergent storage |
| CRITICAL-03 | CRITICAL | Consensus | Non-deterministic state root (HNSW) |
| HIGH-01 | HIGH | State Management | No reorg/rollback for persisted state |
| HIGH-02 | HIGH | Memory | Unbounded StateDB caches |
| HIGH-03 | HIGH | Performance | O(n) trie rebuild every block |
| HIGH-04 | HIGH | Data Corruption | Non-atomic receipt pruning |
| HIGH-05 | HIGH | Block Storage | Silent height→hash overwrite on forks |
| MEDIUM-01 | MEDIUM | Data Corruption | Contract code never persisted in commit() |
| MEDIUM-02 | MEDIUM | Atomicity | MetagraphDB non-atomic compound writes |
| MEDIUM-03 | MEDIUM | Durability | No fsync on critical writes |
| MEDIUM-04 | MEDIUM | Key Encoding | Dual key scheme + shared default CF |
| MEDIUM-05 | MEDIUM | Performance | O(n) trie delete |
| MEDIUM-06 | MEDIUM | Memory | Redundant key HashMap in trie |
| MEDIUM-07 | MEDIUM | Shutdown | No explicit DB flush on shutdown |
| LOW-01 | LOW | Performance | Uncached best_height |
| LOW-02 | LOW | Security | TOCTOU in checkpoint import |
| LOW-03 | LOW | Performance | Missing prefix extractor for EVM storage |
| LOW-04 | LOW | Correctness | Genesis state root is zeros |
| LOW-05 | LOW | Design | Single delegation per delegator |
| LOW-06 | LOW | Feature Gap | No log/event indexing |

---

## Positive Findings

1. **Block storage uses `WriteBatch`** — The `store_block()` method correctly writes block, header, height index, and all transaction indexes in a single atomic batch.
2. **EVM state flush uses `WriteBatch`** — `flush_evm_state()` atomically writes accounts, storage, and deletions.
3. **Fork choice persistence uses `WriteBatch`** — `flush_fork_choice_state()` atomically writes head hash, scores, and attestation stakes.
4. **Proper column family separation** — 15+ CFs (blocks, headers, txs, receipts, EVM state, fork choice, etc.) provide good namespace isolation for most data types.
5. **Real Merkle Patricia Trie** — The trie implementation has proper Branch/Extension/Leaf nodes, hex-prefix encoding, and cryptographic proofs.
6. **Checkpoint system** — RocksDB native checkpoints with checksums, export/import, and pruning.
7. **Path traversal protection** — Checkpoint import validates archive entries against directory escape attacks.
8. **Transaction signing message** — Includes chain_id and length-prefixed data to prevent replay and collision attacks.
9. **LRU cache layer** — `StorageCache` and `CachedBlockchainDB` use bounded LRU caches for blocks, headers, and transactions with hit-rate monitoring.
10. **Compression enabled** — LZ4 compression is configured for RocksDB, reducing disk usage.
