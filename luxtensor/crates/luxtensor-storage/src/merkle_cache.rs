//! Merkle Root Caching for StateDB
//!
//! Production-grade caching layer for Merkle tree computations that wraps
//! an `Arc<RwLock<StateDB>>` to avoid expensive recomputation of state roots.
//!
//! ## Architecture
//! `CachedStateDB` wraps a **shared** `Arc<RwLock<StateDB>>` rather than owning
//! a `StateDB` directly.  This is required because `NodeService` already holds
//! `Arc<RwLock<StateDB>>` and multiple subsystems (executor, RPC, P2P handler)
//! share the same instance.
//!
//! ## Caching Strategy
//! - **Height-based root cache**: LRU cache mapping block height → computed
//!   state root.  Avoids recomputing the root when the same height is queried
//!   multiple times (e.g. RPC `eth_getBlockByNumber` on recent blocks).
//! - **Account hash cache**: LRU cache mapping address → last-known account
//!   hash.  Invalidated on every mutating call to ensure correctness.
//! - **Incremental vs full root**: When fewer than `INCREMENTAL_THRESHOLD`
//!   accounts are dirty, the commit path is recorded as "incremental" in stats.
//!   Both paths currently delegate to `StateDB::commit()` — true branch-level
//!   incremental computation is reserved for a future optimization.
//!
//! ## Thread Safety
//! All interior mutability uses `parking_lot::RwLock` for the cache layers.
//! The underlying `StateDB` is accessed through `Arc<RwLock<StateDB>>`,
//! maintaining the same concurrency contract as the rest of the node.

use crate::{Result, StateDB};
use luxtensor_core::{Account, Address};
use luxtensor_crypto::Hash;
use lru::LruCache;
use parking_lot::RwLock;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Threshold below which the commit is considered "incremental" for stats.
const INCREMENTAL_THRESHOLD: usize = 100;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for Merkle caching behaviour.
///
/// All sizes are capped at `1` minimum to avoid panics from `NonZeroUsize`.
#[derive(Debug, Clone)]
pub struct MerkleCacheConfig {
    /// Maximum number of cached state roots indexed by block height.
    pub max_height_cache: usize,
    /// Maximum number of cached per-account hashes for invalidation tracking.
    pub max_account_hashes: usize,
    /// Enable the incremental root computation path (stats-only today).
    pub incremental_enabled: bool,
}

impl Default for MerkleCacheConfig {
    fn default() -> Self {
        Self {
            max_height_cache: 256,
            max_account_hashes: 10_000,
            incremental_enabled: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Statistics
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime statistics for cache monitoring & alerting.
#[derive(Debug, Clone, Default)]
pub struct MerkleCacheStats {
    /// Number of full (non-incremental) root computations.
    pub full_computations: u64,
    /// Number of incremental root computations.
    pub incremental_computations: u64,
    /// Number of height-cache hits.
    pub root_cache_hits: u64,
    /// Number of height-cache misses.
    pub root_cache_misses: u64,
    /// Number of per-account hash cache hits (reserved for future use).
    pub hash_cache_hits: u64,
}

impl MerkleCacheStats {
    /// Fraction of root lookups that hit the cache (0.0–1.0).
    #[inline]
    pub fn hit_ratio(&self) -> f64 {
        let total = self.root_cache_hits + self.root_cache_misses;
        if total == 0 {
            0.0
        } else {
            self.root_cache_hits as f64 / total as f64
        }
    }

    /// Fraction of commits that took the incremental path (0.0–1.0).
    #[inline]
    pub fn incremental_ratio(&self) -> f64 {
        let total = self.full_computations + self.incremental_computations;
        if total == 0 {
            0.0
        } else {
            self.incremental_computations as f64 / total as f64
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal types
// ─────────────────────────────────────────────────────────────────────────────

/// A cached state root together with the metadata needed for diagnostics.
#[derive(Debug, Clone)]
struct CachedStateRoot {
    /// The computed state root hash.
    root: Hash,
    /// Block height this root was computed at.
    #[allow(dead_code)]
    height: u64,
    /// Number of dirty accounts at the time of computation.
    #[allow(dead_code)]
    account_count: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// CachedStateDB
// ─────────────────────────────────────────────────────────────────────────────

/// Production-grade Merkle-cached wrapper around a **shared** `StateDB`.
///
/// This type is `Send + Sync` and safe to share across threads via `Arc`.
///
/// # Usage
/// ```ignore
/// let state_db: Arc<RwLock<StateDB>> = /* ... */;
/// let cached = CachedStateDB::new(state_db.clone(), MerkleCacheConfig::default());
/// // Use cached.get_balance(), cached.commit(height), etc.
/// ```
pub struct CachedStateDB {
    /// Shared reference to the underlying state database.
    inner: Arc<RwLock<StateDB>>,
    /// LRU cache: block height → computed state root.
    root_cache: RwLock<LruCache<u64, CachedStateRoot>>,
    /// LRU cache: address → last-known account hash (invalidated on mutation).
    account_hash_cache: RwLock<LruCache<Address, Hash>>,
    /// Most recently computed `(height, root)` pair.
    last_root: RwLock<Option<(u64, Hash)>>,
    /// Immutable configuration snapshot.
    config: MerkleCacheConfig,
    /// Mutable runtime statistics.
    stats: RwLock<MerkleCacheStats>,
}

impl CachedStateDB {
    // ── Constructors ─────────────────────────────────────────────────────

    /// Create a new `CachedStateDB` wrapping a shared `StateDB`.
    pub fn new(inner: Arc<RwLock<StateDB>>, config: MerkleCacheConfig) -> Self {
        let root_cache_size = NonZeroUsize::new(config.max_height_cache.max(1)).unwrap();
        let hash_cache_size = NonZeroUsize::new(config.max_account_hashes.max(1)).unwrap();

        Self {
            inner,
            root_cache: RwLock::new(LruCache::new(root_cache_size)),
            account_hash_cache: RwLock::new(LruCache::new(hash_cache_size)),
            last_root: RwLock::new(None),
            config,
            stats: RwLock::new(MerkleCacheStats::default()),
        }
    }

    /// Convenience constructor with default configuration.
    pub fn with_defaults(inner: Arc<RwLock<StateDB>>) -> Self {
        Self::new(inner, MerkleCacheConfig::default())
    }

    // ── Cache-aware root operations ──────────────────────────────────────

    /// Look up a previously cached state root by block height.
    ///
    /// Returns `None` on cache miss.  Both hits and misses are recorded in
    /// [`MerkleCacheStats`].
    pub fn get_state_root(&self, height: u64) -> Option<Hash> {
        let mut cache = self.root_cache.write();
        if let Some(cached) = cache.get(&height) {
            self.stats.write().root_cache_hits += 1;
            Some(cached.root)
        } else {
            self.stats.write().root_cache_misses += 1;
            None
        }
    }

    /// Return the most recently computed `(height, root)` pair, if any.
    pub fn last_state_root(&self) -> Option<(u64, Hash)> {
        *self.last_root.read()
    }

    /// Commit dirty accounts and compute the state root for `height`.
    ///
    /// If there are no dirty accounts *and* we already have a cached root for
    /// the same height, the cached value is returned without recomputation.
    ///
    /// The computed root is stored in both the height cache and the
    /// `last_root` slot.
    #[instrument(skip(self), fields(height))]
    pub fn commit(&self, height: u64) -> Result<Hash> {
        let inner = self.inner.read();
        let dirty_count = inner.dirty_count();

        // Fast path: nothing changed at this height — return cached root.
        if dirty_count == 0 {
            if let Some((last_height, root)) = *self.last_root.read() {
                if last_height == height {
                    debug!("commit({}): 0 dirty accounts, returning cached root", height);
                    return Ok(root);
                }
            }
        }

        // Choose commit strategy (both delegate to inner.commit today).
        let root = if self.config.incremental_enabled && dirty_count < INCREMENTAL_THRESHOLD {
            self.stats.write().incremental_computations += 1;
            inner.commit()?
        } else {
            self.stats.write().full_computations += 1;
            inner.commit()?
        };

        // Populate caches.
        self.root_cache.write().put(
            height,
            CachedStateRoot {
                root,
                height,
                account_count: dirty_count,
            },
        );
        *self.last_root.write() = Some((height, root));

        Ok(root)
    }

    // ── Account operations (with cache invalidation) ─────────────────────

    /// Get account by address.
    pub fn get_account(&self, address: &Address) -> Result<Account> {
        self.inner.read().get_account(address)
    }

    /// Set account (invalidates the per-account hash cache entry).
    pub fn set_account(&self, address: Address, account: Account) {
        self.account_hash_cache.write().pop(&address);
        self.inner.write().set_account(address, account);
    }

    /// Get balance for an address.
    pub fn get_balance(&self, address: &Address) -> Result<u128> {
        self.inner.read().get_balance(address)
    }

    /// Set balance (invalidates the per-account hash cache entry).
    pub fn set_balance(&self, address: &Address, balance: u128) -> Result<()> {
        self.account_hash_cache.write().pop(address);
        self.inner.write().set_balance(address, balance)
    }

    /// Get nonce for an address.
    pub fn get_nonce(&self, address: &Address) -> Result<u64> {
        self.inner.read().get_nonce(address)
    }

    /// Set nonce (invalidates the per-account hash cache entry).
    pub fn set_nonce(&self, address: &Address, nonce: u64) -> Result<()> {
        self.account_hash_cache.write().pop(address);
        self.inner.write().set_nonce(address, nonce)
    }

    /// Increment nonce (invalidates the per-account hash cache entry).
    pub fn increment_nonce(&self, address: &Address) -> Result<u64> {
        self.account_hash_cache.write().pop(address);
        self.inner.write().increment_nonce(address)
    }

    /// Transfer value between two accounts.
    ///
    /// Invalidates cache entries for both `from` and `to`.
    pub fn transfer(&self, from: &Address, to: &Address, value: u128) -> Result<()> {
        {
            let mut hash_cache = self.account_hash_cache.write();
            hash_cache.pop(from);
            hash_cache.pop(to);
        }
        self.inner.write().transfer(from, to, value)
    }

    // ── Contract code storage (Ethereum-style) ───────────────────────────

    /// Store contract bytecode and update the account's `code_hash`.
    ///
    /// Invalidates the per-account hash cache for `address`.
    pub fn set_contract_code(&self, address: &Address, code: Vec<u8>) -> Result<Hash> {
        self.account_hash_cache.write().pop(address);
        self.inner.write().set_contract_code(address, code)
    }

    /// Retrieve contract bytecode by address.
    ///
    /// Returns `Ok(None)` if the address has no deployed code.
    pub fn get_contract_code(&self, address: &Address) -> Result<Option<Vec<u8>>> {
        self.inner.read().get_contract_code(address)
    }

    /// Check whether `address` is a contract (has non-empty code_hash).
    pub fn is_contract(&self, address: &Address) -> Result<bool> {
        self.inner.read().is_contract(address)
    }

    // ── HNSW vector index storage ────────────────────────────────────────

    /// Store a serialized HNSW index under `name`.
    pub fn set_hnsw_index(&self, name: &str, data: Vec<u8>) -> Result<()> {
        self.inner.read().set_hnsw_index(name, data)
    }

    /// Load a serialized HNSW index by `name`.
    pub fn get_hnsw_index(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.inner.read().get_hnsw_index(name)
    }

    /// Delete a serialized HNSW index by `name`.
    pub fn delete_hnsw_index(&self, name: &str) -> Result<()> {
        self.inner.read().delete_hnsw_index(name)
    }

    // ── Rollback & diagnostics ───────────────────────────────────────────

    /// Rollback uncommitted changes in the underlying `StateDB`.
    ///
    /// **Note:** this does *not* roll back the height cache — callers should
    /// call [`clear_caches`] if they want a full reset.
    pub fn rollback(&self) {
        self.inner.read().rollback();
    }

    /// Clear the underlying `StateDB` in-memory cache (for testing).
    pub fn clear_inner_cache(&self) {
        self.inner.write().clear_cache();
    }

    /// Get the configuration snapshot.
    pub fn config(&self) -> &MerkleCacheConfig {
        &self.config
    }

    /// Clone current statistics for monitoring / metrics export.
    pub fn stats(&self) -> MerkleCacheStats {
        self.stats.read().clone()
    }

    /// Evict all entries from the height cache and account hash cache.
    ///
    /// Useful after a chain reorganization or during testing.
    pub fn clear_caches(&self) {
        self.root_cache.write().clear();
        self.account_hash_cache.write().clear();
        *self.last_root.write() = None;
    }

    /// Number of accounts currently in the underlying `StateDB` in-memory cache.
    pub fn cache_size(&self) -> usize {
        self.inner.read().cache_size()
    }

    /// Number of dirty (uncommitted) accounts in the underlying `StateDB`.
    pub fn dirty_count(&self) -> usize {
        self.inner.read().dirty_count()
    }

    /// Direct access to the underlying shared `StateDB`.
    ///
    /// Prefer the delegated methods above whenever possible.  This accessor
    /// exists for subsystems that need the raw `Arc` for their own locking.
    pub fn inner(&self) -> &Arc<RwLock<StateDB>> {
        &self.inner
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rocksdb::{DB, Options};
    use tempfile::TempDir;

    /// Helper: spin up a temporary RocksDB-backed `Arc<RwLock<StateDB>>`.
    fn create_shared_state_db() -> (TempDir, Arc<RwLock<StateDB>>) {
        let temp_dir = TempDir::new().unwrap();
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = Arc::new(DB::open(&opts, temp_dir.path()).unwrap());
        let state_db = Arc::new(RwLock::new(StateDB::new(db)));
        (temp_dir, state_db)
    }

    /// Helper: create a `CachedStateDB` with default config.
    fn create_test_cached_db() -> (TempDir, CachedStateDB) {
        let (dir, shared) = create_shared_state_db();
        let cached = CachedStateDB::with_defaults(shared);
        (dir, cached)
    }

    #[test]
    fn test_creation_empty() {
        let (_dir, cached) = create_test_cached_db();
        assert_eq!(cached.cache_size(), 0);
        assert_eq!(cached.dirty_count(), 0);
        assert!(cached.last_state_root().is_none());
    }

    #[test]
    fn test_commit_caches_root() {
        let (_dir, cached) = create_test_cached_db();
        let addr = Address::try_from_slice(&[1u8; 20]).unwrap();

        cached.set_balance(&addr, 1_000).unwrap();
        let root = cached.commit(1).unwrap();

        // Root should now be cached at height 1.
        assert_eq!(cached.get_state_root(1), Some(root));
        assert_eq!(cached.last_state_root(), Some((1, root)));

        // Stats: exactly one computation.
        let stats = cached.stats();
        assert_eq!(stats.full_computations + stats.incremental_computations, 1);
    }

    #[test]
    fn test_cache_hit_counting() {
        let (_dir, cached) = create_test_cached_db();
        let addr = Address::try_from_slice(&[1u8; 20]).unwrap();

        cached.set_balance(&addr, 500).unwrap();
        let _ = cached.commit(1).unwrap();

        // Two additional lookups = 2 hits.
        let _ = cached.get_state_root(1);
        let _ = cached.get_state_root(1);

        let stats = cached.stats();
        assert_eq!(stats.root_cache_hits, 2);
    }

    #[test]
    fn test_cache_miss_counting() {
        let (_dir, cached) = create_test_cached_db();

        // No commit yet — lookup should miss.
        assert!(cached.get_state_root(42).is_none());

        let stats = cached.stats();
        assert_eq!(stats.root_cache_misses, 1);
    }

    #[test]
    fn test_incremental_path() {
        let (_dir, cached) = create_test_cached_db();
        let addr = Address::try_from_slice(&[1u8; 20]).unwrap();

        // First commit.
        cached.set_balance(&addr, 100).unwrap();
        let _ = cached.commit(1).unwrap();

        // Small change → incremental path (dirty_count < INCREMENTAL_THRESHOLD).
        cached.set_balance(&addr, 200).unwrap();
        let _ = cached.commit(2).unwrap();

        let stats = cached.stats();
        assert_eq!(stats.full_computations + stats.incremental_computations, 2);
    }

    #[test]
    fn test_no_dirty_returns_cached() {
        let (_dir, cached) = create_test_cached_db();
        let addr = Address::try_from_slice(&[1u8; 20]).unwrap();

        cached.set_balance(&addr, 100).unwrap();
        let root1 = cached.commit(5).unwrap();

        // Second commit at same height with no changes → returns cached root.
        let root2 = cached.commit(5).unwrap();
        assert_eq!(root1, root2);

        // Only 1 actual computation should have happened.
        let stats = cached.stats();
        assert_eq!(stats.full_computations + stats.incremental_computations, 1);
    }

    #[test]
    fn test_clear_caches() {
        let (_dir, cached) = create_test_cached_db();
        let addr = Address::try_from_slice(&[1u8; 20]).unwrap();

        cached.set_balance(&addr, 100).unwrap();
        let _ = cached.commit(1).unwrap();
        assert!(cached.get_state_root(1).is_some());

        cached.clear_caches();

        assert!(cached.get_state_root(1).is_none());
        assert!(cached.last_state_root().is_none());
    }

    #[test]
    fn test_rollback() {
        let (_dir, cached) = create_test_cached_db();
        let addr = Address::try_from_slice(&[1u8; 20]).unwrap();

        // Commit initial state.
        cached.set_balance(&addr, 1_000).unwrap();
        cached.commit(1).unwrap();

        // Modify, then rollback.
        cached.set_balance(&addr, 9_999).unwrap();
        assert_eq!(cached.dirty_count(), 1);
        cached.rollback();

        // Reverts to committed value.
        let balance = cached.get_balance(&addr).unwrap();
        assert_eq!(balance, 1_000);
    }

    #[test]
    fn test_transfer_invalidates_cache() {
        let (_dir, cached) = create_test_cached_db();
        let alice = Address::try_from_slice(&[1u8; 20]).unwrap();
        let bob = Address::try_from_slice(&[2u8; 20]).unwrap();

        cached.set_balance(&alice, 1_000).unwrap();
        cached.transfer(&alice, &bob, 300).unwrap();

        assert_eq!(cached.get_balance(&alice).unwrap(), 700);
        assert_eq!(cached.get_balance(&bob).unwrap(), 300);
    }

    #[test]
    fn test_nonce_operations() {
        let (_dir, cached) = create_test_cached_db();
        let addr = Address::try_from_slice(&[1u8; 20]).unwrap();

        cached.set_nonce(&addr, 10).unwrap();
        assert_eq!(cached.get_nonce(&addr).unwrap(), 10);

        let new = cached.increment_nonce(&addr).unwrap();
        assert_eq!(new, 11);
    }

    #[test]
    fn test_contract_code_roundtrip() {
        let (_dir, cached) = create_test_cached_db();
        let addr = Address::try_from_slice(&[0xAA; 20]).unwrap();

        let code = vec![0x60, 0x80, 0x60, 0x40, 0x52]; // EVM PUSH1 preamble
        let code_hash = cached.set_contract_code(&addr, code.clone()).unwrap();
        assert_ne!(code_hash, [0u8; 32]);

        assert!(cached.is_contract(&addr).unwrap());

        let loaded = cached.get_contract_code(&addr).unwrap();
        assert_eq!(loaded, Some(code));
    }

    #[test]
    fn test_inner_accessor() {
        let (_dir, cached) = create_test_cached_db();
        let inner = cached.inner();

        // Should be able to lock and use directly.
        let db = inner.read();
        assert_eq!(db.cache_size(), 0);
    }

    #[test]
    fn test_stats_ratios_zero_division() {
        let stats = MerkleCacheStats::default();
        assert_eq!(stats.hit_ratio(), 0.0);
        assert_eq!(stats.incremental_ratio(), 0.0);
    }

    #[test]
    fn test_stats_ratios_computed() {
        let stats = MerkleCacheStats {
            full_computations: 1,
            incremental_computations: 3,
            root_cache_hits: 7,
            root_cache_misses: 3,
            hash_cache_hits: 0,
        };
        assert!((stats.hit_ratio() - 0.7).abs() < f64::EPSILON);
        assert!((stats.incremental_ratio() - 0.75).abs() < f64::EPSILON);
    }
}
