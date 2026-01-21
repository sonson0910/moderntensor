// Prometheus metrics for Luxtensor node
// Add to Cargo.toml: prometheus = "0.13"

use std::sync::atomic::{AtomicU64, Ordering};

/// Node metrics for Prometheus
pub struct NodeMetrics {
    /// Current block height
    pub block_height: AtomicU64,
    /// Number of peers connected
    pub peer_count: AtomicU64,
    /// Total transactions processed
    pub tx_count: AtomicU64,
    /// Pending transactions in mempool
    pub mempool_size: AtomicU64,
    /// Total staked amount (in smallest unit)
    pub total_stake: AtomicU64,
}

impl Default for NodeMetrics {
    fn default() -> Self {
        Self {
            block_height: AtomicU64::new(0),
            peer_count: AtomicU64::new(0),
            tx_count: AtomicU64::new(0),
            mempool_size: AtomicU64::new(0),
            total_stake: AtomicU64::new(0),
        }
    }
}

impl NodeMetrics {
    /// Generate Prometheus-compatible metrics output
    pub fn export(&self) -> String {
        format!(
            r#"# HELP luxtensor_block_height Current block height
# TYPE luxtensor_block_height gauge
luxtensor_block_height {}

# HELP luxtensor_peer_count Number of connected peers
# TYPE luxtensor_peer_count gauge
luxtensor_peer_count {}

# HELP luxtensor_tx_total Total transactions processed
# TYPE luxtensor_tx_total counter
luxtensor_tx_total {}

# HELP luxtensor_mempool_size Current mempool size
# TYPE luxtensor_mempool_size gauge
luxtensor_mempool_size {}

# HELP luxtensor_total_stake_wei Total staked tokens in wei
# TYPE luxtensor_total_stake_wei gauge
luxtensor_total_stake_wei {}
"#,
            self.block_height.load(Ordering::Relaxed),
            self.peer_count.load(Ordering::Relaxed),
            self.tx_count.load(Ordering::Relaxed),
            self.mempool_size.load(Ordering::Relaxed),
            self.total_stake.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_export() {
        let metrics = NodeMetrics::default();
        metrics.block_height.store(100, Ordering::Relaxed);
        metrics.peer_count.store(5, Ordering::Relaxed);

        let output = metrics.export();
        assert!(output.contains("luxtensor_block_height 100"));
        assert!(output.contains("luxtensor_peer_count 5"));
    }
}
