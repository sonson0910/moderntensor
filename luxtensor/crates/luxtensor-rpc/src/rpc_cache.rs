//! # RPC State Cache
//!
//! Zero-lock and atomic caching layer for hot-path RPC queries.
//!
//! ## Problem
//! Every RPC call (including `eth_chainId`, `eth_blockNumber`) acquires a
//! `RwLock<UnifiedStateDB>`, causing contention under high load.
//!
//! ## Solution
//! - **Immutable data** (`chain_id`): stored as plain `u64`, zero-lock reads
//! - **Slowly-changing data** (`block_number`, `base_fee`): `AtomicU64`, updated per block
//!
//! ## Usage
//! ```ignore
//! let cache = RpcStateCache::new(8898);
//! // Zero-lock reads:
//! cache.chain_id();      // → 8898
//! cache.block_number();  // → current height (atomic)
//! cache.base_fee();      // → current gas price (atomic)
//! // Update after block production:
//! cache.update_block(new_height, new_base_fee);
//! ```

use std::sync::atomic::{AtomicU64, Ordering};

/// Lightweight RPC cache that eliminates lock acquisition for frequently-queried values.
///
/// All methods are `&self` and safe for concurrent access from multiple RPC handler tasks.
pub struct RpcStateCache {
    /// Chain ID — immutable after initialization (no synchronization needed)
    chain_id: u64,

    /// Current block number — updated atomically after each block is produced/synced
    block_number: AtomicU64,

    /// Current EIP-1559 base fee in wei — updated atomically after each block
    base_fee: AtomicU64,
}

impl RpcStateCache {
    /// Create a new RPC state cache.
    ///
    /// - `chain_id`: Network chain ID (immutable for the lifetime of the node)
    /// - `initial_block_number`: Current best block height
    /// - `initial_base_fee`: Current EIP-1559 base fee in wei
    pub fn new(chain_id: u64, initial_block_number: u64, initial_base_fee: u64) -> Self {
        Self {
            chain_id,
            block_number: AtomicU64::new(initial_block_number),
            base_fee: AtomicU64::new(initial_base_fee),
        }
    }

    // ────────────────────── Zero-lock reads ──────────────────────

    /// Get chain ID. **Zero-lock** — returns an immutable value.
    #[inline]
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get current block number. **Zero-lock** — atomic load.
    #[inline]
    pub fn block_number(&self) -> u64 {
        self.block_number.load(Ordering::Relaxed)
    }

    /// Get current base fee in wei. **Zero-lock** — atomic load.
    #[inline]
    pub fn base_fee(&self) -> u64 {
        self.base_fee.load(Ordering::Relaxed)
    }

    // ────────────────────── Atomic updates ──────────────────────

    /// Update cached block number (called after block production or P2P sync).
    #[inline]
    pub fn set_block_number(&self, height: u64) {
        self.block_number.store(height, Ordering::Relaxed);
    }

    /// Update cached base fee (called after FeeMarket recalculation).
    #[inline]
    pub fn set_base_fee(&self, fee: u64) {
        self.base_fee.store(fee, Ordering::Relaxed);
    }

    /// Convenience: update both block number and base fee after a new block.
    #[inline]
    pub fn update_block(&self, height: u64, base_fee: u64) {
        self.set_block_number(height);
        self.set_base_fee(base_fee);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = RpcStateCache::new(8898, 0, 500_000_000);
        assert_eq!(cache.chain_id(), 8898);
        assert_eq!(cache.block_number(), 0);
        assert_eq!(cache.base_fee(), 500_000_000);
    }

    #[test]
    fn test_cache_update() {
        let cache = RpcStateCache::new(8898, 100, 1_000_000_000);
        cache.update_block(101, 1_100_000_000);
        assert_eq!(cache.block_number(), 101);
        assert_eq!(cache.base_fee(), 1_100_000_000);
    }

    #[test]
    fn test_chain_id_immutable() {
        let cache = RpcStateCache::new(1337, 0, 0);
        // chain_id cannot change — this is by design
        assert_eq!(cache.chain_id(), 1337);
    }
}
