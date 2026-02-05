//! LRU Cache Layer for BlockchainDB
//!
//! Provides high-performance in-memory caching for frequently accessed blocks,
//! headers, and transactions to reduce RocksDB read latency.
//!
//! ## Phase 7a Optimization
//! - Block cache: LRU cache for recent blocks (configurable size)
//! - Header cache: LRU cache for block headers
//! - Thread-safe with parking_lot::RwLock

use lru::LruCache;
use luxtensor_core::{Block, BlockHeader, Transaction};
use luxtensor_crypto::Hash;
use parking_lot::RwLock;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::debug;

/// Default cache sizes (optimized for 16GB RAM)
const DEFAULT_BLOCK_CACHE_SIZE: usize = 1024;     // ~1024 blocks * ~10KB = ~10MB
const DEFAULT_HEADER_CACHE_SIZE: usize = 8192;    // ~8192 headers * ~200B = ~1.6MB
const DEFAULT_TX_CACHE_SIZE: usize = 16384;       // ~16384 txs * ~500B = ~8MB

/// Storage cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of blocks to cache
    pub block_cache_size: usize,
    /// Maximum number of headers to cache
    pub header_cache_size: usize,
    /// Maximum number of transactions to cache
    pub tx_cache_size: usize,
    /// Enable cache statistics for monitoring
    pub enable_stats: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            block_cache_size: DEFAULT_BLOCK_CACHE_SIZE,
            header_cache_size: DEFAULT_HEADER_CACHE_SIZE,
            tx_cache_size: DEFAULT_TX_CACHE_SIZE,
            enable_stats: true,
        }
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub block_hits: u64,
    pub block_misses: u64,
    pub header_hits: u64,
    pub header_misses: u64,
    pub tx_hits: u64,
    pub tx_misses: u64,
}

impl CacheStats {
    /// Calculate hit rate for blocks (0.0 to 1.0)
    pub fn block_hit_rate(&self) -> f64 {
        let total = self.block_hits + self.block_misses;
        if total == 0 {
            0.0
        } else {
            self.block_hits as f64 / total as f64
        }
    }

    /// Calculate hit rate for headers (0.0 to 1.0)
    pub fn header_hit_rate(&self) -> f64 {
        let total = self.header_hits + self.header_misses;
        if total == 0 {
            0.0
        } else {
            self.header_hits as f64 / total as f64
        }
    }

    /// Calculate overall hit rate
    pub fn overall_hit_rate(&self) -> f64 {
        let total_hits = self.block_hits + self.header_hits + self.tx_hits;
        let total_misses = self.block_misses + self.header_misses + self.tx_misses;
        let total = total_hits + total_misses;
        if total == 0 {
            0.0
        } else {
            total_hits as f64 / total as f64
        }
    }
}

/// Thread-safe LRU cache for storage layer
pub struct StorageCache {
    /// Block cache: height -> Block
    block_by_height: RwLock<LruCache<u64, Block>>,
    /// Block cache: hash -> Block
    block_by_hash: RwLock<LruCache<Hash, Block>>,
    /// Header cache: hash -> Header
    headers: RwLock<LruCache<Hash, BlockHeader>>,
    /// Transaction cache: hash -> Transaction
    transactions: RwLock<LruCache<Hash, Transaction>>,
    /// Cache statistics
    stats: RwLock<CacheStats>,
    /// Configuration
    config: CacheConfig,
}

impl StorageCache {
    /// Create a new storage cache with default configuration
    pub fn new() -> Self {
        Self::with_config(CacheConfig::default())
    }

    /// Create a new storage cache with custom configuration
    pub fn with_config(config: CacheConfig) -> Self {
        let block_size = NonZeroUsize::new(config.block_cache_size)
            .unwrap_or(NonZeroUsize::new(DEFAULT_BLOCK_CACHE_SIZE).unwrap());
        let header_size = NonZeroUsize::new(config.header_cache_size)
            .unwrap_or(NonZeroUsize::new(DEFAULT_HEADER_CACHE_SIZE).unwrap());
        let tx_size = NonZeroUsize::new(config.tx_cache_size)
            .unwrap_or(NonZeroUsize::new(DEFAULT_TX_CACHE_SIZE).unwrap());

        Self {
            block_by_height: RwLock::new(LruCache::new(block_size)),
            block_by_hash: RwLock::new(LruCache::new(block_size)),
            headers: RwLock::new(LruCache::new(header_size)),
            transactions: RwLock::new(LruCache::new(tx_size)),
            stats: RwLock::new(CacheStats::default()),
            config,
        }
    }

    // ==================== Block Cache ====================

    /// Get a block by height from cache
    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        let mut cache = self.block_by_height.write();
        let result = cache.get(&height).cloned();

        if self.config.enable_stats {
            let mut stats = self.stats.write();
            if result.is_some() {
                stats.block_hits += 1;
                debug!("Cache HIT: block height {}", height);
            } else {
                stats.block_misses += 1;
            }
        }

        result
    }

    /// Get a block by hash from cache
    pub fn get_block_by_hash(&self, hash: &Hash) -> Option<Block> {
        let mut cache = self.block_by_hash.write();
        let result = cache.get(hash).cloned();

        if self.config.enable_stats {
            let mut stats = self.stats.write();
            if result.is_some() {
                stats.block_hits += 1;
            } else {
                stats.block_misses += 1;
            }
        }

        result
    }

    /// Insert a block into cache (by both height and hash)
    pub fn put_block(&self, block: &Block) {
        let height = block.header.height;
        let hash = block.hash();

        {
            let mut cache = self.block_by_height.write();
            cache.put(height, block.clone());
        }
        {
            let mut cache = self.block_by_hash.write();
            cache.put(hash, block.clone());
        }

        debug!("Cache PUT: block height {} hash 0x{}", height, hex::encode(&hash[..8]));
    }

    /// Invalidate a block from cache
    pub fn invalidate_block(&self, height: u64, hash: &Hash) {
        {
            let mut cache = self.block_by_height.write();
            cache.pop(&height);
        }
        {
            let mut cache = self.block_by_hash.write();
            cache.pop(hash);
        }
    }

    // ==================== Header Cache ====================

    /// Get a header by hash from cache
    pub fn get_header(&self, hash: &Hash) -> Option<BlockHeader> {
        let mut cache = self.headers.write();
        let result = cache.get(hash).cloned();

        if self.config.enable_stats {
            let mut stats = self.stats.write();
            if result.is_some() {
                stats.header_hits += 1;
            } else {
                stats.header_misses += 1;
            }
        }

        result
    }

    /// Insert a header into cache
    pub fn put_header(&self, hash: Hash, header: BlockHeader) {
        let mut cache = self.headers.write();
        cache.put(hash, header);
    }

    // ==================== Transaction Cache ====================

    /// Get a transaction by hash from cache
    pub fn get_transaction(&self, hash: &Hash) -> Option<Transaction> {
        let mut cache = self.transactions.write();
        let result = cache.get(hash).cloned();

        if self.config.enable_stats {
            let mut stats = self.stats.write();
            if result.is_some() {
                stats.tx_hits += 1;
            } else {
                stats.tx_misses += 1;
            }
        }

        result
    }

    /// Insert a transaction into cache
    pub fn put_transaction(&self, hash: Hash, tx: Transaction) {
        let mut cache = self.transactions.write();
        cache.put(hash, tx);
    }

    // ==================== Statistics ====================

    /// Get current cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.read().clone()
    }

    /// Reset cache statistics
    pub fn reset_stats(&self) {
        *self.stats.write() = CacheStats::default();
    }

    /// Clear all caches
    pub fn clear(&self) {
        self.block_by_height.write().clear();
        self.block_by_hash.write().clear();
        self.headers.write().clear();
        self.transactions.write().clear();
        self.reset_stats();
    }

    /// Get current cache sizes
    pub fn cache_sizes(&self) -> (usize, usize, usize) {
        (
            self.block_by_height.read().len(),
            self.headers.read().len(),
            self.transactions.read().len(),
        )
    }
}

impl Default for StorageCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper for BlockchainDB with integrated cache
pub struct CachedBlockchainDB {
    /// Inner storage (RocksDB)
    inner: Arc<crate::BlockchainDB>,
    /// Cache layer
    cache: Arc<StorageCache>,
}

impl CachedBlockchainDB {
    /// Create a new cached database wrapper
    pub fn new(inner: Arc<crate::BlockchainDB>) -> Self {
        Self {
            inner,
            cache: Arc::new(StorageCache::new()),
        }
    }

    /// Create with custom cache config
    pub fn with_cache_config(inner: Arc<crate::BlockchainDB>, config: CacheConfig) -> Self {
        Self {
            inner,
            cache: Arc::new(StorageCache::with_config(config)),
        }
    }

    /// Get the inner BlockchainDB reference
    pub fn inner(&self) -> &Arc<crate::BlockchainDB> {
        &self.inner
    }

    /// Get the cache reference
    pub fn cache(&self) -> &Arc<StorageCache> {
        &self.cache
    }

    /// Get a block by height (cache-first)
    pub fn get_block_by_height(&self, height: u64) -> crate::Result<Option<Block>> {
        // Check cache first
        if let Some(block) = self.cache.get_block_by_height(height) {
            return Ok(Some(block));
        }

        // Cache miss - fetch from RocksDB
        let result = self.inner.get_block_by_height(height)?;

        // Populate cache on successful fetch
        if let Some(ref block) = result {
            self.cache.put_block(block);
        }

        Ok(result)
    }

    /// Get a block by hash (cache-first)
    pub fn get_block(&self, hash: &Hash) -> crate::Result<Option<Block>> {
        // Check cache first
        if let Some(block) = self.cache.get_block_by_hash(hash) {
            return Ok(Some(block));
        }

        // Cache miss - fetch from RocksDB
        let result = self.inner.get_block(hash)?;

        // Populate cache on successful fetch
        if let Some(ref block) = result {
            self.cache.put_block(block);
        }

        Ok(result)
    }

    /// Store a block (write-through cache)
    pub fn store_block(&self, block: &Block) -> crate::Result<()> {
        // Write to RocksDB first
        self.inner.store_block(block)?;

        // Update cache
        self.cache.put_block(block);

        Ok(())
    }

    /// Get a header by hash (cache-first)
    pub fn get_header(&self, hash: &Hash) -> crate::Result<Option<BlockHeader>> {
        // Check cache first
        if let Some(header) = self.cache.get_header(hash) {
            return Ok(Some(header));
        }

        // Cache miss - fetch from RocksDB
        let result = self.inner.get_header(hash)?;

        // Populate cache
        if let Some(ref header) = result {
            self.cache.put_header(*hash, header.clone());
        }

        Ok(result)
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.block_cache_size, DEFAULT_BLOCK_CACHE_SIZE);
        assert_eq!(config.header_cache_size, DEFAULT_HEADER_CACHE_SIZE);
        assert!(config.enable_stats);
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let mut stats = CacheStats::default();
        stats.block_hits = 80;
        stats.block_misses = 20;

        assert!((stats.block_hit_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_cache_stats_zero_total() {
        let stats = CacheStats::default();
        assert_eq!(stats.block_hit_rate(), 0.0);
        assert_eq!(stats.overall_hit_rate(), 0.0);
    }

    #[test]
    fn test_storage_cache_new() {
        let cache = StorageCache::new();
        let (blocks, headers, txs) = cache.cache_sizes();
        assert_eq!(blocks, 0);
        assert_eq!(headers, 0);
        assert_eq!(txs, 0);
    }
}
