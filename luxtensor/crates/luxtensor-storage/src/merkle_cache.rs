//! Merkle Root Caching for StateDB
//!
//! Phase 7d optimization: Provides caching layer for Merkle tree computations
//! to avoid expensive recomputation of state roots.
//!
//! ## Strategy
//! - Cache computed Merkle roots by block height
//! - Incremental root updates when only few accounts change
//! - Optional tree snapshots for fast rollback
//!
//! ## NOTE: Incremental Computation Limitation
//! Current "incremental" mode recomputes the full root from all cached account
//! hashes, rather than maintaining a persistent tree structure across updates.
//! True incremental computation (updating only changed branches) is planned
//! for a future optimization pass.

use crate::{StateDB, Result};
use luxtensor_core::{Account, Address};
use luxtensor_crypto::Hash;
use lru::LruCache;
use parking_lot::RwLock;
use std::num::NonZeroUsize;


/// Configuration for Merkle caching
#[derive(Debug, Clone)]
pub struct MerkleCacheConfig {
    /// Maximum number of cached state roots by height
    pub max_height_cache: usize,
    /// Maximum cached account hashes for incremental updates
    pub max_account_hashes: usize,
    /// Enable incremental root computation
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

/// Statistics for Merkle caching
#[derive(Debug, Clone, Default)]
pub struct MerkleCacheStats {
    /// Number of full root computations
    pub full_computations: u64,
    /// Number of incremental root computations
    pub incremental_computations: u64,
    /// Number of cache hits for state roots
    pub root_cache_hits: u64,
    /// Number of cache misses
    pub root_cache_misses: u64,
    /// Number of account hash cache hits
    pub hash_cache_hits: u64,
}

impl MerkleCacheStats {
    /// Calculate cache hit ratio
    pub fn hit_ratio(&self) -> f64 {
        let total = self.root_cache_hits + self.root_cache_misses;
        if total == 0 {
            0.0
        } else {
            self.root_cache_hits as f64 / total as f64
        }
    }

    /// Calculate incremental ratio
    pub fn incremental_ratio(&self) -> f64 {
        let total = self.full_computations + self.incremental_computations;
        if total == 0 {
            0.0
        } else {
            self.incremental_computations as f64 / total as f64
        }
    }
}

/// Cached state root with metadata
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields preserved for future diagnostics
struct CachedStateRoot {
    /// The computed state root hash
    root: Hash,
    /// Block height this root was computed at
    height: u64,
    /// Number of accounts included
    account_count: usize,
}

/// Merkle-cached StateDB wrapper
///
/// Provides optimized state root computation with:
/// - Height-based root caching
/// - Account hash caching for incremental updates
/// - Stats monitoring
pub struct CachedStateDB {
    /// Underlying StateDB
    inner: StateDB,
    /// Cached state roots by height
    root_cache: RwLock<LruCache<u64, CachedStateRoot>>,
    /// Cached account hashes for incremental computation
    account_hash_cache: RwLock<LruCache<Address, Hash>>,
    /// Last computed state root
    last_root: RwLock<Option<(u64, Hash)>>,
    /// Configuration
    config: MerkleCacheConfig,
    /// Statistics
    stats: RwLock<MerkleCacheStats>,
}

impl CachedStateDB {
    /// Create a new cached StateDB
    pub fn new(inner: StateDB, config: MerkleCacheConfig) -> Self {
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

    /// Create with default config
    pub fn with_defaults(inner: StateDB) -> Self {
        Self::new(inner, MerkleCacheConfig::default())
    }

    /// Get cached state root for a specific height
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

    /// Get last computed state root
    pub fn last_state_root(&self) -> Option<(u64, Hash)> {
        *self.last_root.read()
    }

    /// Commit changes and compute state root with caching
    pub fn commit(&self, height: u64) -> Result<Hash> {
        // Check if we have any dirty accounts
        let dirty_count = self.inner.dirty_count();

        if dirty_count == 0 {
            // No changes, return cached root if available
            if let Some((last_height, root)) = *self.last_root.read() {
                if last_height == height {
                    return Ok(root);
                }
            }
        }

        // Compute root
        let root = if self.config.incremental_enabled && dirty_count < 100 {
            self.compute_incremental_root(height)?
        } else {
            self.compute_full_root(height)?
        };

        // Cache the result
        let mut root_cache = self.root_cache.write();
        root_cache.put(
            height,
            CachedStateRoot {
                root,
                height,
                account_count: dirty_count,
            },
        );

        // Update last root
        *self.last_root.write() = Some((height, root));

        Ok(root)
    }

    /// Full root computation (delegates to inner StateDB)
    fn compute_full_root(&self, _height: u64) -> Result<Hash> {
        self.stats.write().full_computations += 1;

        // Actually compute via inner commit
        let root = self.inner.commit()?;

        Ok(root)
    }

    /// Incremental root computation using cached account hashes.
    ///
    /// Current implementation: delegates to full `inner.commit()` which
    /// recomputes the entire trie.  This is correct but not yet optimized.
    /// The statistics tracker records this path separately so operators
    /// can gauge how often the incremental path is hit and prioritize
    /// the optimization (recomputing only dirty branches using the
    /// `account_hash_cache`).
    ///
    /// Correctness note: falling back to full commit is always safe —
    /// the optimization would only reduce latency, not change the result.
    fn compute_incremental_root(&self, _height: u64) -> Result<Hash> {
        self.stats.write().incremental_computations += 1;

        // Delegate to full trie commit.
        // Future optimization: walk only the dirty-account paths in the
        // Merkle trie, using `self.account_hash_cache` for unchanged siblings.
        let root = self.inner.commit()?;

        Ok(root)
    }

    /// Rollback uncommitted changes
    pub fn rollback(&self) {
        self.inner.rollback();
    }

    /// Get configuration
    pub fn config(&self) -> &MerkleCacheConfig {
        &self.config
    }

    /// Get current statistics
    pub fn stats(&self) -> MerkleCacheStats {
        self.stats.read().clone()
    }

    /// Clear all caches
    pub fn clear_caches(&self) {
        self.root_cache.write().clear();
        self.account_hash_cache.write().clear();
        *self.last_root.write() = None;
    }

    // === Delegate methods to inner StateDB ===

    /// Get account by address
    pub fn get_account(&self, address: &Address) -> Result<Account> {
        self.inner.get_account(address)
    }

    /// Set account
    pub fn set_account(&self, address: Address, account: Account) {
        // Invalidate account hash cache for this address
        self.account_hash_cache.write().pop(&address);
        self.inner.set_account(address, account);
    }

    /// Get balance
    pub fn get_balance(&self, address: &Address) -> Result<u128> {
        self.inner.get_balance(address)
    }

    /// Set balance
    pub fn set_balance(&self, address: &Address, balance: u128) -> Result<()> {
        self.account_hash_cache.write().pop(address);
        self.inner.set_balance(address, balance)
    }

    /// Get nonce
    pub fn get_nonce(&self, address: &Address) -> Result<u64> {
        self.inner.get_nonce(address)
    }

    /// Set nonce
    pub fn set_nonce(&self, address: &Address, nonce: u64) -> Result<()> {
        self.account_hash_cache.write().pop(address);
        self.inner.set_nonce(address, nonce)
    }

    /// Increment nonce
    pub fn increment_nonce(&self, address: &Address) -> Result<u64> {
        self.account_hash_cache.write().pop(address);
        self.inner.increment_nonce(address)
    }

    /// Transfer between accounts
    pub fn transfer(&self, from: &Address, to: &Address, value: u128) -> Result<()> {
        self.account_hash_cache.write().pop(from);
        self.account_hash_cache.write().pop(to);
        self.inner.transfer(from, to, value)
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.inner.cache_size()
    }

    /// Get dirty count
    pub fn dirty_count(&self) -> usize {
        self.inner.dirty_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocksdb::{DB, Options};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_cached_db() -> (TempDir, CachedStateDB) {
        let temp_dir = TempDir::new().unwrap();
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = Arc::new(DB::open(&opts, temp_dir.path()).unwrap());
        let state_db = StateDB::new(db);
        let cached = CachedStateDB::with_defaults(state_db);
        (temp_dir, cached)
    }

    #[test]
    fn test_cached_state_db_creation() {
        let (_dir, cached_db) = create_test_cached_db();
        assert_eq!(cached_db.cache_size(), 0);
        assert_eq!(cached_db.dirty_count(), 0);
    }

    #[test]
    fn test_commit_with_cache() {
        let (_dir, cached_db) = create_test_cached_db();
        let address = Address::try_from_slice(&[1u8; 20]).unwrap();

        cached_db.set_balance(&address, 1000).unwrap();
        let root1 = cached_db.commit(1).unwrap();

        // Root should be cached
        let cached_root = cached_db.get_state_root(1);
        assert_eq!(cached_root, Some(root1));

        // Stats should show one computation
        let stats = cached_db.stats();
        assert!(stats.full_computations + stats.incremental_computations == 1);
    }

    #[test]
    fn test_cache_hit() {
        let (_dir, cached_db) = create_test_cached_db();
        let address = Address::try_from_slice(&[1u8; 20]).unwrap();

        cached_db.set_balance(&address, 500).unwrap();
        let _root = cached_db.commit(1).unwrap();

        // Second access should be a cache hit
        let _ = cached_db.get_state_root(1);
        let _ = cached_db.get_state_root(1);

        let stats = cached_db.stats();
        assert_eq!(stats.root_cache_hits, 2);
    }

    #[test]
    fn test_incremental_update() {
        let (_dir, cached_db) = create_test_cached_db();
        let address = Address::try_from_slice(&[1u8; 20]).unwrap();

        // First commit
        cached_db.set_balance(&address, 100).unwrap();
        let _root1 = cached_db.commit(1).unwrap();

        // Small change should use incremental
        cached_db.set_balance(&address, 200).unwrap();
        let _root2 = cached_db.commit(2).unwrap();

        let stats = cached_db.stats();
        // Both should be counted as either full or incremental
        assert!(stats.full_computations + stats.incremental_computations == 2);
    }
}
