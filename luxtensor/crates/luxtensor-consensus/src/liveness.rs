//! Liveness Monitor
//!
//! Monitors network liveness and block production to detect and recover from stalls.
//! This module helps prevent network hangs by:
//! - Tracking last block height (deterministic, consensus-safe)
//! - Detecting stalled validators based on block height gap
//! - Triggering recovery actions when network is stuck
//!
//! **IMPORTANT**: All consensus decisions are based on block height, NOT wall-clock
//! time. Wall-clock `Instant` is retained only for informational logging.

use std::time::{Duration, Instant};
use parking_lot::RwLock;
use tracing::{info, warn, error};

/// Configuration for liveness monitoring
#[derive(Debug, Clone)]
pub struct LivenessConfig {
    /// Maximum number of blocks to fall behind before considering network stalled
    pub block_stall_threshold: u64,
    /// Number of consecutive missed blocks before skipping a validator
    pub max_consecutive_misses: u32,
    /// Extended stall threshold in blocks (e.g., 3x normal) triggers sync
    pub extended_stall_threshold: u64,
    /// Minimum peers required for healthy network
    pub min_healthy_peers: usize,
}

impl Default for LivenessConfig {
    fn default() -> Self {
        Self {
            block_stall_threshold: 10,                   // 10 blocks behind = stall
            max_consecutive_misses: 3,                   // Skip after 3 misses
            extended_stall_threshold: 30,                 // 30 blocks behind = sync needed
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
    /// Longest block height gap observed
    pub longest_block_gap: u64,
    /// Current consecutive missed blocks
    pub current_misses: u32,
    /// Whether network is currently healthy
    pub is_healthy: bool,
}

/// Liveness monitor for detecting and recovering from network stalls.
///
/// **Consensus-critical**: All liveness decisions use block height differences,
/// not wall-clock time. The `last_block_time` field is retained only for
/// informational logging and metrics.
pub struct LivenessMonitor {
    config: LivenessConfig,
    /// Last time a block was observed (informational/logging only — NOT used for decisions)
    last_block_time: RwLock<Instant>,
    /// Last block height (consensus-authoritative)
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

        // Calculate height gap since last recorded block
        let gap = height.saturating_sub(*last_height);
        if gap > stats.longest_block_gap {
            stats.longest_block_gap = gap;
        }

        // Update informational wall-clock timestamp (logging only)
        *last_time = now;
        // Update consensus-authoritative height
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

    /// Check network liveness and return recommended action.
    ///
    /// **Consensus-critical**: Staleness is computed as `current_height - last_block_height`
    /// (block height difference), ensuring all nodes reach the same decision deterministically.
    pub fn check_liveness(&self, current_height: u64) -> LivenessAction {
        let last_height = *self.last_block_height.read();
        let height_gap = current_height.saturating_sub(last_height);
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

        // Check for extended stall first (higher threshold) — possible network partition
        if height_gap > self.config.extended_stall_threshold {
            let mut stats = self.stats.write();
            stats.is_healthy = false;
            stats.syncs_triggered += 1;

            error!(
                "🚨 {} blocks behind (extended threshold: {}), triggering sync",
                height_gap, self.config.extended_stall_threshold
            );
            return LivenessAction::RequestSync;
        }

        // Check block stall threshold
        if height_gap > self.config.block_stall_threshold {
            if misses >= self.config.max_consecutive_misses {
                // Too many misses, skip this validator
                let mut stats = self.stats.write();
                stats.validators_skipped += 1;
                stats.is_healthy = false;

                error!(
                    "❌ Validator missed {} blocks ({} blocks behind), skipping",
                    misses, height_gap
                );
                return LivenessAction::SkipValidator;
            }

            // Need to wait more, but log warning
            warn!(
                "⏳ {} blocks behind (threshold: {}), misses: {}/{}",
                height_gap, self.config.block_stall_threshold, misses, self.config.max_consecutive_misses
            );
            return LivenessAction::WaitMore;
        }

        // Network is healthy
        LivenessAction::Healthy
    }

    /// Get current liveness statistics
    pub fn get_stats(&self) -> LivenessStats {
        self.stats.read().clone()
    }

    /// Get wall-clock time since last block (informational only — NOT for consensus decisions)
    pub fn time_since_last_block(&self) -> Duration {
        Instant::now().duration_since(*self.last_block_time.read())
    }

    /// Get current block height
    pub fn current_height(&self) -> u64 {
        *self.last_block_height.read()
    }

    /// Check if network is healthy based on block height gap.
    ///
    /// `current_height` is the latest known network height.
    pub fn is_healthy(&self, current_height: u64) -> bool {
        let last_height = *self.last_block_height.read();
        let height_gap = current_height.saturating_sub(last_height);
        let peers = *self.peer_count.read();

        height_gap <= self.config.block_stall_threshold && peers >= self.config.min_healthy_peers
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

    #[test]
    fn test_liveness_monitor_creation() {
        let monitor = LivenessMonitor::default();
        monitor.update_peer_count(5);
        // Height 0, current_height 0 → gap = 0, healthy
        assert!(monitor.is_healthy(0));
    }

    #[test]
    fn test_record_block() {
        let monitor = LivenessMonitor::default();
        monitor.update_peer_count(5);
        monitor.record_block(100);

        assert_eq!(monitor.current_height(), 100);
        assert!(monitor.is_healthy(100));
    }

    #[test]
    fn test_missed_blocks() {
        let config = LivenessConfig {
            block_stall_threshold: 5,
            max_consecutive_misses: 2,
            extended_stall_threshold: 15,
            min_healthy_peers: 1,
        };

        let monitor = LivenessMonitor::new(config);
        monitor.update_peer_count(5);
        monitor.record_block(10);

        // Height gap = 0, healthy
        assert_eq!(monitor.check_liveness(10), LivenessAction::Healthy);

        // Height gap = 6 > threshold 5, should trigger WaitMore
        assert_eq!(monitor.check_liveness(16), LivenessAction::WaitMore);

        // Record miss
        monitor.record_missed_block();
        assert_eq!(monitor.check_liveness(16), LivenessAction::WaitMore);

        // Another miss should trigger skip (2 >= max_consecutive_misses=2)
        monitor.record_missed_block();
        assert_eq!(monitor.check_liveness(16), LivenessAction::SkipValidator);
    }

    #[test]
    fn test_low_peer_count() {
        let config = LivenessConfig {
            min_healthy_peers: 5,
            ..LivenessConfig::default()
        };

        let monitor = LivenessMonitor::new(config);
        monitor.update_peer_count(2);

        assert_eq!(monitor.check_liveness(0), LivenessAction::DiscoverPeers);
    }

    #[test]
    fn test_stats() {
        let monitor = LivenessMonitor::default();
        monitor.update_peer_count(10);

        // Perform some checks at height 0
        monitor.check_liveness(0);
        monitor.check_liveness(0);
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

    #[test]
    fn test_extended_stall_triggers_sync() {
        let config = LivenessConfig {
            block_stall_threshold: 5,
            max_consecutive_misses: 3,
            extended_stall_threshold: 15,
            min_healthy_peers: 1,
        };

        let monitor = LivenessMonitor::new(config);
        monitor.update_peer_count(5);
        monitor.record_block(10);

        // Height gap = 26 > extended_stall_threshold=15, should trigger sync
        assert_eq!(monitor.check_liveness(36), LivenessAction::RequestSync);
    }

    #[test]
    fn test_is_healthy_height_based() {
        let config = LivenessConfig {
            block_stall_threshold: 10,
            min_healthy_peers: 1,
            ..LivenessConfig::default()
        };

        let monitor = LivenessMonitor::new(config);
        monitor.update_peer_count(3);
        monitor.record_block(100);

        // Within threshold
        assert!(monitor.is_healthy(105));
        // At threshold boundary
        assert!(monitor.is_healthy(110));
        // Beyond threshold
        assert!(!monitor.is_healthy(111));
    }
}
