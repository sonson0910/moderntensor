//! Health Monitor
//!
//! Provides health check endpoints and auto-recovery mechanisms.
//! This module helps ensure network stability by:
//! - Exposing health check endpoints for monitoring
//! - Detecting common issues (low peers, stalled sync, etc.)
//! - Triggering automatic recovery actions

use std::time::{Duration, Instant};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};
use tracing::{info, warn};

/// Get available disk space in bytes for the given path using sysinfo.
/// Finds the disk whose mount point best matches the given path.
fn get_available_disk_space(path: &std::path::Path) -> Option<u64> {
    let disks = Disks::new_with_refreshed_list();
    let mut best_match: Option<(usize, u64)> = None;
    for disk in disks.list() {
        let mount = disk.mount_point();
        if path.starts_with(mount) {
            let mount_len = mount.as_os_str().len();
            if best_match.is_none() || mount_len > best_match.unwrap().0 {
                best_match = Some((mount_len, disk.available_space()));
            }
        }
    }
    best_match.map(|(_, space)| space)
}

/// Health status of the node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Whether the node is healthy overall
    pub healthy: bool,
    /// Current block height
    pub block_height: u64,
    /// Number of connected peers
    pub peer_count: usize,
    /// Whether node is syncing
    pub is_syncing: bool,
    /// Sync progress (0-100)
    pub sync_progress: u8,
    /// Time since last block (seconds)
    pub seconds_since_last_block: u64,
    /// Mempool size
    pub mempool_size: usize,
    /// List of current issues
    pub issues: Vec<HealthIssue>,
    /// Node uptime in seconds
    pub uptime_seconds: u64,
}

/// Possible health issues
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthIssue {
    /// Not enough peers connected
    LowPeerCount { current: usize, minimum: usize },
    /// Block production has stalled
    BlockProductionStalled { seconds: u64 },
    /// Mempool is overloaded
    MempoolOverloaded { size: usize, max: usize },
    /// Sync is lagging behind
    SyncLagging { lag: u64 },
    /// Disk space is low
    LowDiskSpace { available_mb: u64 },
    /// High memory usage
    HighMemoryUsage { percent: u8 },
}

impl HealthIssue {
    /// Get severity level (1-10)
    pub fn severity(&self) -> u8 {
        match self {
            Self::LowPeerCount { current, minimum } => {
                if *current == 0 { 10 }
                else if *current < *minimum / 2 { 7 }
                else { 4 }
            }
            Self::BlockProductionStalled { seconds } => {
                if *seconds > 300 { 10 }
                else if *seconds > 120 { 7 }
                else { 5 }
            }
            Self::MempoolOverloaded { .. } => 5,
            Self::SyncLagging { lag } => {
                if *lag > 1000 { 8 }
                else if *lag > 100 { 5 }
                else { 3 }
            }
            Self::LowDiskSpace { available_mb } => {
                if *available_mb < 100 { 10 }
                else if *available_mb < 1000 { 7 }
                else { 4 }
            }
            Self::HighMemoryUsage { percent } => {
                if *percent > 95 { 9 }
                else if *percent > 85 { 6 }
                else { 3 }
            }
        }
    }

    /// Check if this issue is critical
    pub fn is_critical(&self) -> bool {
        self.severity() >= 8
    }
}

/// Configuration for health monitoring
#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Minimum peers for healthy status
    pub min_peers: usize,
    /// Maximum seconds without block before warning
    pub max_block_gap_seconds: u64,
    /// Maximum mempool size before warning
    pub max_mempool_size: usize,
    /// Maximum sync lag before warning
    pub max_sync_lag: u64,
    /// Minimum disk space in MB
    pub min_disk_space_mb: u64,
    /// Maximum memory usage percent
    pub max_memory_percent: u8,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            min_peers: 3,
            max_block_gap_seconds: 60,
            max_mempool_size: 50000,
            max_sync_lag: 100,
            min_disk_space_mb: 1000,
            max_memory_percent: 90,
        }
    }
}

/// Health metrics collector
pub struct HealthMonitor {
    config: HealthConfig,
    start_time: Instant,
    /// Current metrics
    block_height: RwLock<u64>,
    peer_count: RwLock<usize>,
    is_syncing: RwLock<bool>,
    sync_progress: RwLock<u8>,
    last_block_time: RwLock<Instant>,
    mempool_size: RwLock<usize>,
    /// Data directory path for disk space checks
    data_dir: Option<std::path::PathBuf>,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(config: HealthConfig) -> Self {
        Self {
            config,
            start_time: Instant::now(),
            block_height: RwLock::new(0),
            peer_count: RwLock::new(0),
            is_syncing: RwLock::new(false),
            sync_progress: RwLock::new(100),
            last_block_time: RwLock::new(Instant::now()),
            mempool_size: RwLock::new(0),
            data_dir: None,
        }
    }

    /// Create a new health monitor with data directory for disk space checks
    pub fn with_data_dir(config: HealthConfig, data_dir: std::path::PathBuf) -> Self {
        Self {
            config,
            start_time: Instant::now(),
            block_height: RwLock::new(0),
            peer_count: RwLock::new(0),
            is_syncing: RwLock::new(false),
            sync_progress: RwLock::new(100),
            last_block_time: RwLock::new(Instant::now()),
            mempool_size: RwLock::new(0),
            data_dir: Some(data_dir),
        }
    }

    /// Update block height
    pub fn update_block_height(&self, height: u64) {
        *self.block_height.write() = height;
        *self.last_block_time.write() = Instant::now();
    }

    /// Update peer count
    pub fn update_peer_count(&self, count: usize) {
        *self.peer_count.write() = count;
    }

    /// Update sync status
    #[allow(dead_code)]
    pub fn update_sync_status(&self, is_syncing: bool, progress: u8) {
        *self.is_syncing.write() = is_syncing;
        *self.sync_progress.write() = progress;
    }

    /// Update mempool size
    #[allow(dead_code)]
    pub fn update_mempool_size(&self, size: usize) {
        *self.mempool_size.write() = size;
    }

    /// Get current health status
    pub fn get_health(&self) -> HealthStatus {
        let block_height = *self.block_height.read();
        let peer_count = *self.peer_count.read();
        let is_syncing = *self.is_syncing.read();
        let sync_progress = *self.sync_progress.read();
        let mempool_size = *self.mempool_size.read();
        let seconds_since_last_block = self.last_block_time.read().elapsed().as_secs();

        let mut issues = Vec::new();

        // Check peer count
        if peer_count < self.config.min_peers {
            issues.push(HealthIssue::LowPeerCount {
                current: peer_count,
                minimum: self.config.min_peers,
            });
        }

        // Check block production (only if not syncing)
        if !is_syncing && seconds_since_last_block > self.config.max_block_gap_seconds {
            issues.push(HealthIssue::BlockProductionStalled {
                seconds: seconds_since_last_block,
            });
        }

        // Check mempool
        if mempool_size > self.config.max_mempool_size {
            issues.push(HealthIssue::MempoolOverloaded {
                size: mempool_size,
                max: self.config.max_mempool_size,
            });
        }

        // Check sync lag
        if is_syncing && (100 - sync_progress) as u64 > self.config.max_sync_lag {
            issues.push(HealthIssue::SyncLagging {
                lag: (100 - sync_progress) as u64,
            });
        }

        // Check disk space on data directory
        if let Some(ref data_dir) = self.data_dir {
            if let Some(available) = get_available_disk_space(data_dir) {
                let available_mb = available / (1024 * 1024);
                if available_mb < self.config.min_disk_space_mb {
                    issues.push(HealthIssue::LowDiskSpace { available_mb });
                }
            }
        }

        // Check memory usage via sysinfo
        {
            let mut sys = System::new();
            sys.refresh_memory();
            let total = sys.total_memory();
            let used = sys.used_memory();
            if total > 0 {
                let percent = ((used as f64 / total as f64) * 100.0) as u8;
                if percent > self.config.max_memory_percent {
                    issues.push(HealthIssue::HighMemoryUsage { percent });
                }
            }
        }

        let healthy = issues.is_empty() || !issues.iter().any(|i| i.is_critical());

        HealthStatus {
            healthy,
            block_height,
            peer_count,
            is_syncing,
            sync_progress,
            seconds_since_last_block,
            mempool_size,
            issues,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    /// Simple health check (just returns healthy/unhealthy)
    #[allow(dead_code)]
    pub fn is_healthy(&self) -> bool {
        self.get_health().healthy
    }

    /// Get uptime
    #[allow(dead_code)]
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Log current health status
    #[allow(dead_code)]
    pub fn log_health(&self) {
        let status = self.get_health();
        if status.healthy {
            info!(
                "💚 Node healthy: height={}, peers={}, mempool={}",
                status.block_height, status.peer_count, status.mempool_size
            );
        } else {
            warn!(
                "💛 Node has issues: {:?}",
                status.issues
            );
        }
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new(HealthConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_health_monitor_creation() {
        let monitor = HealthMonitor::default();
        assert!(!monitor.is_healthy()); // No peers = unhealthy
    }

    #[test]
    fn test_healthy_node() {
        let monitor = HealthMonitor::default();
        monitor.update_peer_count(5);
        monitor.update_block_height(100);

        let status = monitor.get_health();
        assert!(status.healthy);
        assert_eq!(status.block_height, 100);
        assert_eq!(status.peer_count, 5);
        assert!(status.issues.is_empty());
    }

    #[test]
    fn test_low_peer_count_issue() {
        let monitor = HealthMonitor::default();
        monitor.update_peer_count(1);

        let status = monitor.get_health();
        // peer_count=1 creates a LowPeerCount issue but it's not critical (severity=4)
        // So node is still "healthy" but has issues
        assert!(status.healthy); // Low peer count is not critical
        assert!(status.issues.iter().any(|i| matches!(i, HealthIssue::LowPeerCount { .. })));
    }

    #[test]
    fn test_mempool_overload() {
        let config = HealthConfig {
            max_mempool_size: 100,
            min_peers: 0, // Disable peer check
            ..HealthConfig::default()
        };

        let monitor = HealthMonitor::new(config);
        monitor.update_mempool_size(200);

        let status = monitor.get_health();
        assert!(status.issues.iter().any(|i| matches!(i, HealthIssue::MempoolOverloaded { .. })));
    }

    #[test]
    fn test_issue_severity() {
        let low_severity = HealthIssue::LowPeerCount { current: 2, minimum: 3 };
        let high_severity = HealthIssue::LowPeerCount { current: 0, minimum: 3 };

        assert!(low_severity.severity() < high_severity.severity());
        assert!(high_severity.is_critical());
        assert!(!low_severity.is_critical());
    }

    #[test]
    fn test_uptime() {
        let monitor = HealthMonitor::default();
        std::thread::sleep(Duration::from_millis(10));

        assert!(monitor.uptime().as_millis() >= 10);
    }
}
