//! Graceful Shutdown & Recovery
//!
//! Provides mechanisms for clean node shutdown with state persistence
//! and recovery on restart.

use std::path::Path;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Shutdown state for coordinating graceful shutdown
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownState {
    /// Normal operation
    Running,
    /// Shutdown initiated, stopping new work
    Stopping,
    /// Saving state to disk
    SavingState,
    /// Notifying peers
    NotifyingPeers,
    /// Shutdown complete
    Stopped,
}

/// Configuration for graceful shutdown
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ShutdownConfig {
    /// Maximum time to wait for pending transactions
    pub max_pending_tx_wait: Duration,
    /// Maximum time for full shutdown
    pub max_shutdown_time: Duration,
    /// Whether to save mempool on shutdown
    pub save_mempool: bool,
    /// Whether to save peer list on shutdown
    pub save_peer_list: bool,
    /// Whether to notify peers before disconnect
    pub notify_peers: bool,
    /// State backup directory
    pub backup_dir: String,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            max_pending_tx_wait: Duration::from_secs(10),
            max_shutdown_time: Duration::from_secs(30),
            save_mempool: true,
            save_peer_list: true,
            notify_peers: true,
            backup_dir: "state_backup".to_string(),
        }
    }
}

/// Checkpoint data saved during shutdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownCheckpoint {
    /// Block height at shutdown
    pub block_height: u64,
    /// Block hash at shutdown
    pub block_hash: [u8; 32],
    /// Timestamp of shutdown
    pub shutdown_time: u64,
    /// Node version
    pub node_version: String,
    /// Number of pending transactions saved
    pub pending_tx_count: usize,
    /// Number of peers saved
    pub peer_count: usize,
}

/// Result of shutdown operation
#[allow(dead_code)]
#[derive(Debug)]
pub struct ShutdownResult {
    /// Whether shutdown was successful
    pub success: bool,
    /// State at shutdown
    pub state: ShutdownState,
    /// Checkpoint saved (if any)
    pub checkpoint: Option<ShutdownCheckpoint>,
    /// Errors encountered
    pub errors: Vec<String>,
    /// Duration of shutdown
    pub duration: Duration,
}

/// Graceful shutdown manager
pub struct GracefulShutdown {
    config: ShutdownConfig,
    state: RwLock<ShutdownState>,
    shutdown_start: RwLock<Option<Instant>>,
    accepting_new_work: RwLock<bool>,
}

#[allow(dead_code)]
impl GracefulShutdown {
    /// Create new shutdown manager
    pub fn new(config: ShutdownConfig) -> Self {
        Self {
            config,
            state: RwLock::new(ShutdownState::Running),
            shutdown_start: RwLock::new(None),
            accepting_new_work: RwLock::new(true),
        }
    }

    /// Create with default config
    pub fn default() -> Self {
        Self::new(ShutdownConfig::default())
    }

    /// Check if shutdown has been initiated
    pub fn is_shutting_down(&self) -> bool {
        *self.state.read() != ShutdownState::Running
    }

    /// Check if accepting new work
    pub fn is_accepting_new_work(&self) -> bool {
        *self.accepting_new_work.read()
    }

    /// Get current state
    pub fn get_state(&self) -> ShutdownState {
        *self.state.read()
    }

    /// Initiate graceful shutdown - phase 1
    pub fn initiate_shutdown(&self) {
        info!("🛑 Initiating graceful shutdown...");
        *self.state.write() = ShutdownState::Stopping;
        *self.shutdown_start.write() = Some(Instant::now());
        *self.accepting_new_work.write() = false;
    }

    /// Update state to saving
    pub fn begin_state_save(&self) {
        info!("💾 Saving node state...");
        *self.state.write() = ShutdownState::SavingState;
    }

    /// Update state to notifying peers
    pub fn begin_peer_notification(&self) {
        info!("📡 Notifying peers of disconnect...");
        *self.state.write() = ShutdownState::NotifyingPeers;
    }

    /// Mark shutdown as complete
    pub fn complete_shutdown(&self) {
        info!("✅ Shutdown complete");
        *self.state.write() = ShutdownState::Stopped;
    }

    /// Check if shutdown timeout exceeded
    pub fn is_timeout_exceeded(&self) -> bool {
        if let Some(start) = *self.shutdown_start.read() {
            start.elapsed() > self.config.max_shutdown_time
        } else {
            false
        }
    }

    /// Get shutdown duration
    pub fn shutdown_duration(&self) -> Option<Duration> {
        self.shutdown_start.read().map(|s| s.elapsed())
    }

    /// Save checkpoint to disk
    pub fn save_checkpoint(
        &self,
        checkpoint: &ShutdownCheckpoint,
    ) -> Result<(), std::io::Error> {
        let backup_dir = Path::new(&self.config.backup_dir);
        if !backup_dir.exists() {
            std::fs::create_dir_all(backup_dir)?;
        }

        let checkpoint_path = backup_dir.join("shutdown_checkpoint.json");
        let json = serde_json::to_string_pretty(checkpoint)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        std::fs::write(&checkpoint_path, json)?;
        info!("💾 Saved shutdown checkpoint to {:?}", checkpoint_path);
        Ok(())
    }

    /// Load checkpoint from disk
    pub fn load_checkpoint(&self) -> Result<Option<ShutdownCheckpoint>, std::io::Error> {
        let checkpoint_path = Path::new(&self.config.backup_dir).join("shutdown_checkpoint.json");

        if !checkpoint_path.exists() {
            return Ok(None);
        }

        let json = std::fs::read_to_string(&checkpoint_path)?;
        let checkpoint: ShutdownCheckpoint = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        info!("📂 Loaded shutdown checkpoint from height {}", checkpoint.block_height);
        Ok(Some(checkpoint))
    }

    /// Clear checkpoint after successful recovery
    pub fn clear_checkpoint(&self) -> Result<(), std::io::Error> {
        let checkpoint_path = Path::new(&self.config.backup_dir).join("shutdown_checkpoint.json");

        if checkpoint_path.exists() {
            std::fs::remove_file(&checkpoint_path)?;
            info!("🗑️ Cleared old shutdown checkpoint");
        }
        Ok(())
    }

    /// Execute full graceful shutdown sequence (sync version for offline use)
    pub fn execute_shutdown_sync(
        &self,
        current_height: u64,
        current_hash: [u8; 32],
        pending_tx_count: usize,
        peer_count: usize,
    ) -> ShutdownResult {
        let start = Instant::now();
        let mut errors = Vec::new();

        // Phase 1: Stop accepting new work
        self.initiate_shutdown();

        // Phase 2: Save state
        self.begin_state_save();

        let checkpoint = ShutdownCheckpoint {
            block_height: current_height,
            block_hash: current_hash,
            shutdown_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            node_version: env!("CARGO_PKG_VERSION").to_string(),
            pending_tx_count,
            peer_count,
        };

        if let Err(e) = self.save_checkpoint(&checkpoint) {
            errors.push(format!("Failed to save checkpoint: {}", e));
        }

        // Phase 3: Notify peers (simulated for sync version)
        if self.config.notify_peers {
            self.begin_peer_notification();
        }

        // Complete
        self.complete_shutdown();

        ShutdownResult {
            success: errors.is_empty(),
            state: *self.state.read(),
            checkpoint: Some(checkpoint),
            errors,
            duration: start.elapsed(),
        }
    }

    /// Get config
    pub fn config(&self) -> &ShutdownConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_shutdown_creation() {
        let shutdown = GracefulShutdown::default();
        assert!(!shutdown.is_shutting_down());
        assert!(shutdown.is_accepting_new_work());
        assert_eq!(shutdown.get_state(), ShutdownState::Running);
    }

    #[test]
    fn test_initiate_shutdown() {
        let shutdown = GracefulShutdown::default();

        shutdown.initiate_shutdown();

        assert!(shutdown.is_shutting_down());
        assert!(!shutdown.is_accepting_new_work());
        assert_eq!(shutdown.get_state(), ShutdownState::Stopping);
    }

    #[test]
    fn test_shutdown_state_transitions() {
        let shutdown = GracefulShutdown::default();

        assert_eq!(shutdown.get_state(), ShutdownState::Running);

        shutdown.initiate_shutdown();
        assert_eq!(shutdown.get_state(), ShutdownState::Stopping);

        shutdown.begin_state_save();
        assert_eq!(shutdown.get_state(), ShutdownState::SavingState);

        shutdown.begin_peer_notification();
        assert_eq!(shutdown.get_state(), ShutdownState::NotifyingPeers);

        shutdown.complete_shutdown();
        assert_eq!(shutdown.get_state(), ShutdownState::Stopped);
    }

    #[test]
    fn test_checkpoint_save_load() {
        let config = ShutdownConfig {
            backup_dir: "test_backup_graceful".to_string(),
            ..Default::default()
        };
        let shutdown = GracefulShutdown::new(config);

        let checkpoint = ShutdownCheckpoint {
            block_height: 12345,
            block_hash: [1u8; 32],
            shutdown_time: 1000000,
            node_version: "1.0.0".to_string(),
            pending_tx_count: 50,
            peer_count: 10,
        };

        // Save
        shutdown.save_checkpoint(&checkpoint).unwrap();

        // Load
        let loaded = shutdown.load_checkpoint().unwrap().unwrap();
        assert_eq!(loaded.block_height, 12345);
        assert_eq!(loaded.pending_tx_count, 50);

        // Cleanup
        let _ = fs::remove_dir_all("test_backup_graceful");
    }

    #[test]
    fn test_execute_shutdown_sync() {
        let config = ShutdownConfig {
            backup_dir: "test_backup_shutdown".to_string(),
            ..Default::default()
        };
        let shutdown = GracefulShutdown::new(config);

        let result = shutdown.execute_shutdown_sync(
            1000,
            [5u8; 32],
            25,
            8,
        );

        assert!(result.success);
        assert_eq!(result.state, ShutdownState::Stopped);
        assert!(result.checkpoint.is_some());
        assert!(result.errors.is_empty());

        // Cleanup
        let _ = fs::remove_dir_all("test_backup_shutdown");
    }

    #[test]
    fn test_shutdown_duration() {
        let shutdown = GracefulShutdown::default();

        assert!(shutdown.shutdown_duration().is_none());

        shutdown.initiate_shutdown();
        std::thread::sleep(Duration::from_millis(10));

        let duration = shutdown.shutdown_duration().unwrap();
        assert!(duration.as_millis() >= 10);
    }
}
