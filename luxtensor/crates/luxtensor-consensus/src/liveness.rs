//! Liveness Monitor
//!
//! Monitors network liveness and block production to detect and recover from stalls.
//! This module helps prevent network hangs by:
//! - Tracking last block production time
//! - Detecting stalled validators
//! - Triggering recovery actions when network is stuck

use std::time::{Duration, Instant};
use parking_lot::RwLock;
use tracing::{info, warn, error};

/// Configuration for liveness monitoring
#[derive(Debug, Clone)]
pub struct LivenessConfig {
    /// Maximum time to wait for a new block before considering network stalled
    pub block_timeout: Duration,
    /// Number of consecutive missed blocks before skipping a validator
    pub max_consecutive_misses: u32,
    /// Time between liveness checks
    pub check_interval: Duration,
    /// Minimum peers required for healthy network
    pub min_healthy_peers: usize,
}

impl Default for LivenessConfig {
    fn default() -> Self {
        Self {
            block_timeout: Duration::from_secs(30),     // 30 seconds max wait
            max_consecutive_misses: 3,                   // Skip after 3 misses
            check_interval: Duration::from_secs(5),      // Check every 5 seconds
            min_healthy_peers: 3,                        // At least 3 peers
        }
    }
}

/// Actions that can be taken when liveness issues are detected
#[derive(Debug, Clone, PartialEq)]
pub enum LivenessAction {
    /// Network is healthy, no action needed
    Healthy,
    /// Waiting for block, but not yet critical
    WaitMore,
    /// Skip the current validator and move to next
    SkipValidator,
    /// Trigger peer discovery to find more nodes
    DiscoverPeers,
    /// Request sync from other nodes
    RequestSync,
    /// Critical: Network appears to be stalled
    NetworkStalled,
}

/// Statistics about liveness monitoring
#[derive(Debug, Clone, Default)]
pub struct LivenessStats {
    /// Total number of liveness checks performed
    pub checks_performed: u64,
    /// Number of times validator was skipped
    pub validators_skipped: u64,
    /// Number of times sync was triggered
    pub syncs_triggered: u64,
    /// Longest period without a block (seconds)
    pub longest_block_gap: u64,
    /// Current consecutive missed blocks
    pub current_misses: u32,
    /// Whether network is currently healthy
    pub is_healthy: bool,
}

/// Liveness monitor for detecting and recovering from network stalls
pub struct LivenessMonitor {
    config: LivenessConfig,
    /// Last time a block was produced
    last_block_time: RwLock<Instant>,
    /// Last block height
    last_block_height: RwLock<u64>,
    /// Consecutive missed blocks by current validator
    consecutive_misses: RwLock<u32>,
    /// Statistics
    stats: RwLock<LivenessStats>,
    /// Current peer count
    peer_count: RwLock<usize>,
}

impl LivenessMonitor {
    /// Create a new liveness monitor with the given configuration
    pub fn new(config: LivenessConfig) -> Self {
        Self {
            config,
            last_block_time: RwLock::new(Instant::now()),
            last_block_height: RwLock::new(0),
            consecutive_misses: RwLock::new(0),
            stats: RwLock::new(LivenessStats::default()),
            peer_count: RwLock::new(0),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(LivenessConfig::default())
    }

    /// Record that a new block was produced
    pub fn record_block(&self, height: u64) {
        let now = Instant::now();
        let mut last_time = self.last_block_time.write();
        let mut last_height = self.last_block_height.write();
        let mut misses = self.consecutive_misses.write();
        let mut stats = self.stats.write();

        // Calculate gap since last block
        let gap = now.duration_since(*last_time).as_secs();
        if gap > stats.longest_block_gap {
            stats.longest_block_gap = gap;
        }

        // Reset counters
        *last_time = now;
        *last_height = height;
        *misses = 0;
        stats.is_healthy = true;

        info!("📦 Block {} produced, network healthy", height);
    }

    /// Record that we expected a block but didn't get one
    pub fn record_missed_block(&self) {
        let mut misses = self.consecutive_misses.write();
        let mut stats = self.stats.write();

        *misses += 1;
        stats.current_misses = *misses;

        warn!("⏰ Missed block, consecutive misses: {}", *misses);
    }

    /// Update the current peer count
    pub fn update_peer_count(&self, count: usize) {
        *self.peer_count.write() = count;
    }

    /// Check network liveness and return recommended action
    pub fn check_liveness(&self) -> LivenessAction {
        let last_time = *self.last_block_time.read();
        let elapsed = Instant::now().duration_since(last_time);
        let misses = *self.consecutive_misses.read();
        let peers = *self.peer_count.read();

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.checks_performed += 1;
        }

        // Check peer count first
        if peers < self.config.min_healthy_peers {
            warn!("🔍 Low peer count: {} (min: {})", peers, self.config.min_healthy_peers);
            return LivenessAction::DiscoverPeers;
        }

        // Check block timeout
        if elapsed > self.config.block_timeout {
            if misses >= self.config.max_consecutive_misses {
                // Too many misses, skip this validator
                let mut stats = self.stats.write();
                stats.validators_skipped += 1;
                stats.is_healthy = false;

                error!(
                    "❌ Validator missed {} blocks (timeout: {:?}), skipping",
                    misses, elapsed
                );
                return LivenessAction::SkipValidator;
            }

            // Need to wait more, but log warning
            warn!(
                "⏳ No block for {:?} (timeout: {:?}), misses: {}/{}",
                elapsed, self.config.block_timeout, misses, self.config.max_consecutive_misses
            );
            return LivenessAction::WaitMore;
        }

        // Check for extended period without blocks (possible network partition)
        let extended_timeout = self.config.block_timeout * 3;
        if elapsed > extended_timeout {
            let mut stats = self.stats.write();
            stats.is_healthy = false;
            stats.syncs_triggered += 1;

            error!(
                "🚨 No block for {:?} (extended timeout), triggering sync",
                elapsed
            );
            return LivenessAction::RequestSync;
        }

        // Network is healthy
        LivenessAction::Healthy
    }

    /// Get current liveness statistics
    pub fn get_stats(&self) -> LivenessStats {
        self.stats.read().clone()
    }

    /// Get time since last block
    pub fn time_since_last_block(&self) -> Duration {
        Instant::now().duration_since(*self.last_block_time.read())
    }

    /// Get current block height
    pub fn current_height(&self) -> u64 {
        *self.last_block_height.read()
    }

    /// Check if network is healthy
    pub fn is_healthy(&self) -> bool {
        let elapsed = self.time_since_last_block();
        let peers = *self.peer_count.read();

        elapsed < self.config.block_timeout && peers >= self.config.min_healthy_peers
    }

    /// Reset the liveness monitor (e.g., after recovery)
    pub fn reset(&self) {
        *self.last_block_time.write() = Instant::now();
        *self.consecutive_misses.write() = 0;
        self.stats.write().is_healthy = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_liveness_monitor_creation() {
        let monitor = LivenessMonitor::default();
        monitor.update_peer_count(5); // Need peers for healthy status
        assert!(monitor.is_healthy());
    }

    #[test]
    fn test_record_block() {
        let monitor = LivenessMonitor::default();
        monitor.update_peer_count(5); // Need peers for healthy status
        monitor.record_block(100);

        assert_eq!(monitor.current_height(), 100);
        assert!(monitor.is_healthy());
    }

    #[test]
    fn test_missed_blocks() {
        let config = LivenessConfig {
            block_timeout: Duration::from_millis(50),
            max_consecutive_misses: 2,
            check_interval: Duration::from_millis(10),
            min_healthy_peers: 1,
        };

        let monitor = LivenessMonitor::new(config);
        monitor.update_peer_count(5);

        // Initially healthy
        assert_eq!(monitor.check_liveness(), LivenessAction::Healthy);

        // Wait for timeout
        thread::sleep(Duration::from_millis(60));

        // Should be waiting
        assert_eq!(monitor.check_liveness(), LivenessAction::WaitMore);

        // Record miss
        monitor.record_missed_block();
        assert_eq!(monitor.check_liveness(), LivenessAction::WaitMore);

        // Another miss should trigger skip
        monitor.record_missed_block();
        assert_eq!(monitor.check_liveness(), LivenessAction::SkipValidator);
    }

    #[test]
    fn test_low_peer_count() {
        let config = LivenessConfig {
            min_healthy_peers: 5,
            ..LivenessConfig::default()
        };

        let monitor = LivenessMonitor::new(config);
        monitor.update_peer_count(2);

        assert_eq!(monitor.check_liveness(), LivenessAction::DiscoverPeers);
    }

    #[test]
    fn test_stats() {
        let monitor = LivenessMonitor::default();
        monitor.update_peer_count(10);

        // Perform some checks
        monitor.check_liveness();
        monitor.check_liveness();
        monitor.record_block(1);

        let stats = monitor.get_stats();
        assert_eq!(stats.checks_performed, 2);
        assert!(stats.is_healthy);
    }

    #[test]
    fn test_reset() {
        let monitor = LivenessMonitor::default();
        monitor.record_missed_block();
        monitor.record_missed_block();

        let stats_before = monitor.get_stats();
        assert_eq!(stats_before.current_misses, 2);

        monitor.reset();

        let misses = *monitor.consecutive_misses.read();
        assert_eq!(misses, 0);
    }
}
