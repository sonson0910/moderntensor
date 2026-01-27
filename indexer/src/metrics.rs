//! Indexer Metrics Module
//!
//! Provides real-time metrics for indexer operations:
//! - Blocks indexed
//! - Transactions processed
//! - Events decoded
//! - Sync status

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::time::Instant;

/// Indexer metrics collector
pub struct IndexerMetrics {
    /// Last indexed block number
    pub last_indexed_block: AtomicU64,
    /// Total blocks indexed
    pub total_blocks_indexed: AtomicU64,
    /// Total transactions indexed
    pub total_transactions: AtomicU64,
    /// Total token transfers indexed
    pub total_transfers: AtomicU64,
    /// Total stake events indexed
    pub total_stake_events: AtomicU64,
    /// Whether indexer is currently syncing
    pub is_syncing: AtomicBool,
    /// Indexer start time
    start_time: Instant,
}

impl Default for IndexerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexerMetrics {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self {
            last_indexed_block: AtomicU64::new(0),
            total_blocks_indexed: AtomicU64::new(0),
            total_transactions: AtomicU64::new(0),
            total_transfers: AtomicU64::new(0),
            total_stake_events: AtomicU64::new(0),
            is_syncing: AtomicBool::new(false),
            start_time: Instant::now(),
        }
    }

    /// Record a block being indexed
    pub fn record_block(&self, block_number: u64, tx_count: usize) {
        self.last_indexed_block.store(block_number, Ordering::Relaxed);
        self.total_blocks_indexed.fetch_add(1, Ordering::Relaxed);
        self.total_transactions.fetch_add(tx_count as u64, Ordering::Relaxed);
    }

    /// Record a token transfer
    pub fn record_transfer(&self) {
        self.total_transfers.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a stake event
    pub fn record_stake_event(&self) {
        self.total_stake_events.fetch_add(1, Ordering::Relaxed);
    }

    /// Set syncing status
    pub fn set_syncing(&self, syncing: bool) {
        self.is_syncing.store(syncing, Ordering::Relaxed);
    }

    /// Get uptime in seconds
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Get indexing rate (blocks per second since start)
    pub fn blocks_per_sec(&self) -> f64 {
        let uptime = self.uptime_secs();
        if uptime == 0 {
            return 0.0;
        }
        self.total_blocks_indexed.load(Ordering::Relaxed) as f64 / uptime as f64
    }

    /// Get metrics as JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "lastIndexedBlock": self.last_indexed_block.load(Ordering::Relaxed),
            "totalBlocksIndexed": self.total_blocks_indexed.load(Ordering::Relaxed),
            "totalTransactions": self.total_transactions.load(Ordering::Relaxed),
            "totalTransfers": self.total_transfers.load(Ordering::Relaxed),
            "totalStakeEvents": self.total_stake_events.load(Ordering::Relaxed),
            "isSyncing": self.is_syncing.load(Ordering::Relaxed),
            "uptimeSecs": self.uptime_secs(),
            "blocksPerSec": self.blocks_per_sec(),
        })
    }

    /// Generate Prometheus-compatible metrics output
    pub fn export(&self) -> String {
        format!(
            r#"# HELP indexer_last_block Last indexed block number
# TYPE indexer_last_block gauge
indexer_last_block {}

# HELP indexer_blocks_total Total blocks indexed
# TYPE indexer_blocks_total counter
indexer_blocks_total {}

# HELP indexer_transactions_total Total transactions indexed
# TYPE indexer_transactions_total counter
indexer_transactions_total {}

# HELP indexer_transfers_total Total token transfers indexed
# TYPE indexer_transfers_total counter
indexer_transfers_total {}

# HELP indexer_stake_events_total Total stake events indexed
# TYPE indexer_stake_events_total counter
indexer_stake_events_total {}

# HELP indexer_syncing Is indexer currently syncing
# TYPE indexer_syncing gauge
indexer_syncing {}

# HELP indexer_uptime_seconds Indexer uptime in seconds
# TYPE indexer_uptime_seconds counter
indexer_uptime_seconds {}

# HELP indexer_blocks_per_second Average blocks indexed per second
# TYPE indexer_blocks_per_second gauge
indexer_blocks_per_second {:.4}
"#,
            self.last_indexed_block.load(Ordering::Relaxed),
            self.total_blocks_indexed.load(Ordering::Relaxed),
            self.total_transactions.load(Ordering::Relaxed),
            self.total_transfers.load(Ordering::Relaxed),
            self.total_stake_events.load(Ordering::Relaxed),
            if self.is_syncing.load(Ordering::Relaxed) { 1 } else { 0 },
            self.uptime_secs(),
            self.blocks_per_sec(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = IndexerMetrics::new();
        assert_eq!(metrics.last_indexed_block.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_record_block() {
        let metrics = IndexerMetrics::new();
        metrics.record_block(100, 10);
        metrics.record_block(101, 15);

        assert_eq!(metrics.last_indexed_block.load(Ordering::Relaxed), 101);
        assert_eq!(metrics.total_blocks_indexed.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.total_transactions.load(Ordering::Relaxed), 25);
    }

    #[test]
    fn test_to_json() {
        let metrics = IndexerMetrics::new();
        metrics.record_block(50, 5);
        let json = metrics.to_json();
        assert_eq!(json["lastIndexedBlock"], 50);
    }
}
