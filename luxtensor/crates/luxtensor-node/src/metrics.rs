// Prometheus metrics for Luxtensor node
// Add to Cargo.toml: prometheus = "0.13"

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use parking_lot::RwLock;
use std::collections::VecDeque;

/// Maximum history points for moving averages
const MAX_HISTORY: usize = 100;

/// Node metrics for Prometheus with enhanced monitoring
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

    // Enhanced metrics
    /// Block production times in ms (for average calculation)
    block_times: RwLock<VecDeque<u64>>,
    /// Start time for uptime calculation
    start_time: Instant,
    /// Last block production time in ms
    last_block_time_ms: AtomicU64,
}

impl Default for NodeMetrics {
    fn default() -> Self {
        Self {
            block_height: AtomicU64::new(0),
            peer_count: AtomicU64::new(0),
            tx_count: AtomicU64::new(0),
            mempool_size: AtomicU64::new(0),
            total_stake: AtomicU64::new(0),
            block_times: RwLock::new(VecDeque::with_capacity(MAX_HISTORY)),
            start_time: Instant::now(),
            last_block_time_ms: AtomicU64::new(0),
        }
    }
}

impl NodeMetrics {
    /// Record a block production with timing
    pub fn record_block(&self, height: u64, tx_count: usize, production_time_ms: u64) {
        self.block_height.store(height, Ordering::Relaxed);
        self.tx_count.fetch_add(tx_count as u64, Ordering::Relaxed);
        self.last_block_time_ms.store(production_time_ms, Ordering::Relaxed);

        let mut times = self.block_times.write();
        if times.len() >= MAX_HISTORY {
            times.pop_front();
        }
        times.push_back(production_time_ms);
    }

    /// Get average block production time (last N blocks)
    pub fn avg_block_time_ms(&self) -> f64 {
        let times = self.block_times.read();
        if times.is_empty() {
            return 0.0;
        }
        let sum: u64 = times.iter().sum();
        sum as f64 / times.len() as f64
    }

    /// Get uptime in seconds
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Get TX throughput (tx/sec based on recent blocks)
    pub fn tx_throughput(&self) -> f64 {
        let avg_time = self.avg_block_time_ms();
        if avg_time == 0.0 {
            return 0.0;
        }
        // Blocks per second * avg tx per block (estimate from total)
        let block_height = self.block_height.load(Ordering::Relaxed);
        let tx_count = self.tx_count.load(Ordering::Relaxed);
        if block_height == 0 {
            return 0.0;
        }
        let avg_tx_per_block = tx_count as f64 / block_height as f64;
        let blocks_per_sec = 1000.0 / avg_time;
        blocks_per_sec * avg_tx_per_block
    }

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

# HELP luxtensor_avg_block_time_ms Average block production time in ms
# TYPE luxtensor_avg_block_time_ms gauge
luxtensor_avg_block_time_ms {:.2}

# HELP luxtensor_last_block_time_ms Last block production time in ms
# TYPE luxtensor_last_block_time_ms gauge
luxtensor_last_block_time_ms {}

# HELP luxtensor_uptime_seconds Node uptime in seconds
# TYPE luxtensor_uptime_seconds counter
luxtensor_uptime_seconds {}

# HELP luxtensor_tx_throughput Estimated transactions per second
# TYPE luxtensor_tx_throughput gauge
luxtensor_tx_throughput {:.4}
"#,
            self.block_height.load(Ordering::Relaxed),
            self.peer_count.load(Ordering::Relaxed),
            self.tx_count.load(Ordering::Relaxed),
            self.mempool_size.load(Ordering::Relaxed),
            self.total_stake.load(Ordering::Relaxed),
            self.avg_block_time_ms(),
            self.last_block_time_ms.load(Ordering::Relaxed),
            self.uptime_secs(),
            self.tx_throughput(),
        )
    }

    /// Get metrics as JSON for RPC
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "blockHeight": self.block_height.load(Ordering::Relaxed),
            "peerCount": self.peer_count.load(Ordering::Relaxed),
            "txCount": self.tx_count.load(Ordering::Relaxed),
            "mempoolSize": self.mempool_size.load(Ordering::Relaxed),
            "totalStake": self.total_stake.load(Ordering::Relaxed).to_string(),
            "avgBlockTimeMs": self.avg_block_time_ms(),
            "lastBlockTimeMs": self.last_block_time_ms.load(Ordering::Relaxed),
            "uptimeSecs": self.uptime_secs(),
            "txThroughput": self.tx_throughput(),
        })
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

    #[test]
    fn test_record_block() {
        let metrics = NodeMetrics::default();
        metrics.record_block(1, 10, 100);
        metrics.record_block(2, 15, 150);

        assert_eq!(metrics.block_height.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.tx_count.load(Ordering::Relaxed), 25);
        assert_eq!(metrics.avg_block_time_ms(), 125.0);
    }

    #[test]
    fn test_uptime() {
        let metrics = NodeMetrics::default();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(metrics.uptime_secs() >= 0);
    }
}

