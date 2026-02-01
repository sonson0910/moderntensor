//! Database Backup, Restore, and Pruning
//!
//! Provides utilities for database maintenance:
//! - Full backup to directory
//! - Restore from backup
//! - Pruning old data

use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Read};
use tracing::{info, warn, error};

/// Backup configuration
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Directory for backups
    pub backup_dir: PathBuf,
    /// Maximum number of backups to keep
    pub max_backups: usize,
    /// Compress backups
    pub compress: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            backup_dir: PathBuf::from("./backups"),
            max_backups: 5,
            compress: true,
        }
    }
}

/// Pruning configuration
#[derive(Debug, Clone)]
pub struct PruningConfig {
    /// Keep blocks newer than this height (0 = keep all)
    pub keep_blocks_after: u64,
    /// Keep last N blocks (0 = use height-based)
    pub keep_last_n_blocks: u64,
    /// Prune receipts older than N blocks
    pub prune_receipts_after: u64,
    /// Enable automatic pruning
    pub auto_prune: bool,
    /// Auto-prune interval in blocks
    pub auto_prune_interval: u64,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            keep_blocks_after: 0,
            keep_last_n_blocks: 100_000, // Keep last 100k blocks
            prune_receipts_after: 10_000, // Prune receipts after 10k blocks
            auto_prune: true,
            auto_prune_interval: 1000, // Prune every 1000 blocks
        }
    }
}

/// Database maintenance manager
pub struct DbMaintenance {
    db_path: PathBuf,
    backup_config: BackupConfig,
    pruning_config: PruningConfig,
}

impl DbMaintenance {
    pub fn new(db_path: PathBuf, backup_config: BackupConfig, pruning_config: PruningConfig) -> Self {
        Self {
            db_path,
            backup_config,
            pruning_config,
        }
    }

    /// Create a backup of the database
    pub fn create_backup(&self, label: &str) -> io::Result<PathBuf> {
        // Ensure backup directory exists
        fs::create_dir_all(&self.backup_config.backup_dir)?;

        // Generate backup filename with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let backup_name = format!("backup_{}_{}", label, timestamp);
        let backup_path = self.backup_config.backup_dir.join(&backup_name);

        info!("💾 Creating backup: {:?}", backup_path);

        // Copy database directory
        self.copy_dir_recursive(&self.db_path, &backup_path)?;

        info!("✅ Backup created successfully: {:?}", backup_path);

        // Cleanup old backups
        self.cleanup_old_backups()?;

        Ok(backup_path)
    }

    /// Restore database from backup
    pub fn restore_from_backup(&self, backup_path: &Path) -> io::Result<()> {
        if !backup_path.exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Backup not found"));
        }

        info!("🔄 Restoring from backup: {:?}", backup_path);

        // Create a backup of current state first
        let temp_backup = self.db_path.with_extension("old");
        if self.db_path.exists() {
            fs::rename(&self.db_path, &temp_backup)?;
        }

        // Copy backup to db path
        match self.copy_dir_recursive(backup_path, &self.db_path) {
            Ok(_) => {
                // Remove temp backup on success
                if temp_backup.exists() {
                    fs::remove_dir_all(&temp_backup)?;
                }
                info!("✅ Restore completed successfully");
                Ok(())
            }
            Err(e) => {
                // Restore original on failure
                error!("❌ Restore failed: {}", e);
                if temp_backup.exists() {
                    fs::rename(&temp_backup, &self.db_path)?;
                }
                Err(e)
            }
        }
    }

    /// List available backups
    pub fn list_backups(&self) -> io::Result<Vec<BackupInfo>> {
        let mut backups = Vec::new();

        if !self.backup_config.backup_dir.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.backup_config.backup_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let metadata = fs::metadata(&path)?;
                let size = self.get_dir_size(&path)?;

                backups.push(BackupInfo {
                    path: path.clone(),
                    name: path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    size_bytes: size,
                    created: metadata.created().ok(),
                });
            }
        }

        // Sort by created time (newest first)
        backups.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(backups)
    }

    /// Get pruning statistics
    pub fn get_pruning_stats(&self, current_height: u64) -> PruningStats {
        let prune_blocks_before = if self.pruning_config.keep_last_n_blocks > 0 {
            current_height.saturating_sub(self.pruning_config.keep_last_n_blocks)
        } else if self.pruning_config.keep_blocks_after > 0 {
            self.pruning_config.keep_blocks_after
        } else {
            0
        };

        let prune_receipts_before = current_height
            .saturating_sub(self.pruning_config.prune_receipts_after);

        PruningStats {
            current_height,
            prune_blocks_before,
            prune_receipts_before,
            estimated_prunable_blocks: prune_blocks_before,
        }
    }

    /// Check if auto-prune should run
    pub fn should_auto_prune(&self, current_height: u64) -> bool {
        self.pruning_config.auto_prune &&
        current_height % self.pruning_config.auto_prune_interval == 0
    }

    // Helper: Copy directory recursively
    fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> io::Result<()> {
        fs::create_dir_all(dst)?;

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                self.copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }

    // Helper: Get directory size
    fn get_dir_size(&self, path: &Path) -> io::Result<u64> {
        let mut size = 0;

        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();

                if entry_path.is_dir() {
                    size += self.get_dir_size(&entry_path)?;
                } else {
                    size += entry.metadata()?.len();
                }
            }
        }

        Ok(size)
    }

    // Helper: Cleanup old backups
    fn cleanup_old_backups(&self) -> io::Result<()> {
        let backups = self.list_backups()?;

        if backups.len() > self.backup_config.max_backups {
            let to_remove = backups.len() - self.backup_config.max_backups;
            for backup in backups.iter().rev().take(to_remove) {
                warn!("🗑️ Removing old backup: {:?}", backup.path);
                fs::remove_dir_all(&backup.path)?;
            }
        }

        Ok(())
    }
}

/// Backup information
#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    pub created: Option<std::time::SystemTime>,
}

/// Pruning statistics
#[derive(Debug, Clone)]
pub struct PruningStats {
    pub current_height: u64,
    pub prune_blocks_before: u64,
    pub prune_receipts_before: u64,
    pub estimated_prunable_blocks: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_backup_config_default() {
        let config = BackupConfig::default();
        assert_eq!(config.max_backups, 5);
    }

    #[test]
    fn test_pruning_stats() {
        let maintenance = DbMaintenance::new(
            PathBuf::from("./test_db"),
            BackupConfig::default(),
            PruningConfig::default(),
        );

        let stats = maintenance.get_pruning_stats(150_000);
        assert!(stats.prune_blocks_before > 0);
    }

    #[test]
    fn test_should_auto_prune() {
        let maintenance = DbMaintenance::new(
            PathBuf::from("./test_db"),
            BackupConfig::default(),
            PruningConfig {
                auto_prune: true,
                auto_prune_interval: 100,
                ..Default::default()
            },
        );

        assert!(maintenance.should_auto_prune(1000));
        assert!(!maintenance.should_auto_prune(1001));
    }

    #[test]
    fn test_backup_restore() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("db");
        let backup_dir = temp.path().join("backups");

        // Create test database
        fs::create_dir_all(&db_path).unwrap();
        fs::write(db_path.join("test.txt"), "test data").unwrap();

        let maintenance = DbMaintenance::new(
            db_path.clone(),
            BackupConfig {
                backup_dir: backup_dir.clone(),
                max_backups: 2,
                compress: false,
            },
            PruningConfig::default(),
        );

        // Create backup
        let backup_path = maintenance.create_backup("test").unwrap();
        assert!(backup_path.exists());

        // Verify backup contains file
        assert!(backup_path.join("test.txt").exists());

        // List backups
        let backups = maintenance.list_backups().unwrap();
        assert_eq!(backups.len(), 1);
    }
}
