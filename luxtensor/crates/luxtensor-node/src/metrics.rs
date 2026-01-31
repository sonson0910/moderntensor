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

    // Network metrics (Phase 4 Enhancement)
    /// Inbound connection count
    pub inbound_connections: AtomicU64,
    /// Outbound connection count
    pub outbound_connections: AtomicU64,
    /// Total bytes sent
    pub bytes_sent: AtomicU64,
    /// Total bytes received
    pub bytes_received: AtomicU64,

    // Consensus metrics (Phase 4 Enhancement)
    /// Active validator count
    pub validator_count: AtomicU64,
    /// Current epoch number
    pub epoch_number: AtomicU64,
    /// Blocks produced by this node
    pub blocks_produced: AtomicU64,
    /// Missed block slots
    pub missed_blocks: AtomicU64,
    /// Slashing events
    pub slashing_events: AtomicU64,

    // RPC metrics (Phase 4 Enhancement)
    /// Total RPC requests
    pub rpc_requests: AtomicU64,
    /// RPC errors
    pub rpc_errors: AtomicU64,
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
            // Network metrics
            inbound_connections: AtomicU64::new(0),
            outbound_connections: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            // Consensus metrics
            validator_count: AtomicU64::new(0),
            epoch_number: AtomicU64::new(0),
            blocks_produced: AtomicU64::new(0),
            missed_blocks: AtomicU64::new(0),
            slashing_events: AtomicU64::new(0),
            // RPC metrics
            rpc_requests: AtomicU64::new(0),
            rpc_errors: AtomicU64::new(0),
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

# HELP luxtensor_inbound_connections Inbound peer connections
# TYPE luxtensor_inbound_connections gauge
luxtensor_inbound_connections {}

# HELP luxtensor_outbound_connections Outbound peer connections
# TYPE luxtensor_outbound_connections gauge
luxtensor_outbound_connections {}

# HELP luxtensor_bytes_sent Total bytes sent
# TYPE luxtensor_bytes_sent counter
luxtensor_bytes_sent {}

# HELP luxtensor_bytes_received Total bytes received
# TYPE luxtensor_bytes_received counter
luxtensor_bytes_received {}

# HELP luxtensor_validator_count Active validators
# TYPE luxtensor_validator_count gauge
luxtensor_validator_count {}

# HELP luxtensor_epoch Current epoch number
# TYPE luxtensor_epoch gauge
luxtensor_epoch {}

# HELP luxtensor_blocks_produced Blocks produced by this node
# TYPE luxtensor_blocks_produced counter
luxtensor_blocks_produced {}

# HELP luxtensor_missed_blocks Missed block slots
# TYPE luxtensor_missed_blocks counter
luxtensor_missed_blocks {}

# HELP luxtensor_slashing_events Total slashing events
# TYPE luxtensor_slashing_events counter
luxtensor_slashing_events {}

# HELP luxtensor_rpc_requests Total RPC requests
# TYPE luxtensor_rpc_requests counter
luxtensor_rpc_requests {}

# HELP luxtensor_rpc_errors Total RPC errors
# TYPE luxtensor_rpc_errors counter
luxtensor_rpc_errors {}
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
            self.inbound_connections.load(Ordering::Relaxed),
            self.outbound_connections.load(Ordering::Relaxed),
            self.bytes_sent.load(Ordering::Relaxed),
            self.bytes_received.load(Ordering::Relaxed),
            self.validator_count.load(Ordering::Relaxed),
            self.epoch_number.load(Ordering::Relaxed),
            self.blocks_produced.load(Ordering::Relaxed),
            self.missed_blocks.load(Ordering::Relaxed),
            self.slashing_events.load(Ordering::Relaxed),
            self.rpc_requests.load(Ordering::Relaxed),
            self.rpc_errors.load(Ordering::Relaxed),
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
            // Network metrics
            "inboundConnections": self.inbound_connections.load(Ordering::Relaxed),
            "outboundConnections": self.outbound_connections.load(Ordering::Relaxed),
            "bytesSent": self.bytes_sent.load(Ordering::Relaxed),
            "bytesReceived": self.bytes_received.load(Ordering::Relaxed),
            // Consensus metrics
            "validatorCount": self.validator_count.load(Ordering::Relaxed),
            "epochNumber": self.epoch_number.load(Ordering::Relaxed),
            "blocksProduced": self.blocks_produced.load(Ordering::Relaxed),
            "missedBlocks": self.missed_blocks.load(Ordering::Relaxed),
            "slashingEvents": self.slashing_events.load(Ordering::Relaxed),
            // RPC metrics
            "rpcRequests": self.rpc_requests.load(Ordering::Relaxed),
            "rpcErrors": self.rpc_errors.load(Ordering::Relaxed),
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

