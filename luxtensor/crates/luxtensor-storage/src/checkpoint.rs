// Checkpoint snapshot system for rapid node synchronization
// Enables new nodes to download state snapshots instead of replaying all blocks

use luxtensor_core::types::Hash;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

/// Interval between checkpoint snapshots (in blocks)
pub const CHECKPOINT_INTERVAL: u64 = 10_000;

/// Maximum number of checkpoints to keep
pub const MAX_CHECKPOINTS: usize = 5;

/// Checkpoint configuration
#[derive(Debug, Clone)]
pub struct CheckpointConfig {
    /// Interval between checkpoint snapshots (in blocks)
    pub checkpoint_interval: u64,
    /// Maximum number of checkpoints to keep
    pub max_checkpoints: usize,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            checkpoint_interval: CHECKPOINT_INTERVAL,
            max_checkpoints: MAX_CHECKPOINTS,
        }
    }
}

impl CheckpointConfig {
    /// Create a new checkpoint config with custom values
    pub fn new(checkpoint_interval: u64, max_checkpoints: usize) -> Self {
        Self {
            checkpoint_interval,
            max_checkpoints,
        }
    }
}

/// Checkpoint metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    /// Block height at checkpoint
    pub height: u64,
    /// Block hash at checkpoint
    pub block_hash: Hash,
    /// State root hash at checkpoint
    pub state_root: Hash,
    /// Timestamp of checkpoint creation
    pub created_at: u64,
    /// Size of checkpoint in bytes
    pub size_bytes: u64,
    /// Checksum of snapshot file
    pub checksum: String,
}

/// Checkpoint manager for creating and restoring state snapshots
pub struct CheckpointManager {
    /// Directory to store checkpoints
    checkpoint_dir: PathBuf,
    /// Reference to blockchain DB
    db: Arc<DB>,
    /// Available checkpoints
    checkpoints: HashMap<u64, CheckpointMetadata>,
    /// Configuration
    config: CheckpointConfig,
}

impl CheckpointManager {
    /// Create a new checkpoint manager with default configuration
    pub fn new<P: AsRef<Path>>(checkpoint_dir: P, db: Arc<DB>) -> Self {
        Self::new_with_config(checkpoint_dir, db, CheckpointConfig::default())
    }

    /// Create a new checkpoint manager with custom configuration
    pub fn new_with_config<P: AsRef<Path>>(checkpoint_dir: P, db: Arc<DB>, config: CheckpointConfig) -> Self {
        let checkpoint_dir = checkpoint_dir.as_ref().to_path_buf();

        // Ensure checkpoint directory exists
        if !checkpoint_dir.exists() {
            if let Err(e) = fs::create_dir_all(&checkpoint_dir) {
                warn!("Failed to create checkpoint directory: {}", e);
            }
        }

        let mut manager = Self {
            checkpoint_dir,
            db,
            checkpoints: HashMap::new(),
            config,
        };

        // Load existing checkpoints
        manager.scan_checkpoints();

        manager
    }

    /// Get the checkpoint configuration
    pub fn config(&self) -> &CheckpointConfig {
        &self.config
    }

    /// Scan for existing checkpoint files
    fn scan_checkpoints(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.checkpoint_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "meta") {
                    if let Ok(file) = File::open(&path) {
                        let reader = BufReader::new(file);
                        if let Ok(meta) = serde_json::from_reader::<_, CheckpointMetadata>(reader) {
                            info!("Found checkpoint at height {}", meta.height);
                            self.checkpoints.insert(meta.height, meta);
                        }
                    }
                }
            }
        }
    }

    /// Check if a checkpoint should be created at this height
    pub fn should_create_checkpoint(&self, height: u64) -> bool {
        height > 0 && height % CHECKPOINT_INTERVAL == 0
    }

    /// Create a checkpoint at the current state
    pub fn create_checkpoint(
        &mut self,
        height: u64,
        block_hash: Hash,
        state_root: Hash,
    ) -> Result<CheckpointMetadata, CheckpointError> {
        info!("Creating checkpoint at height {}", height);

        let timestamp = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs(),
            Err(e) => {
                tracing::warn!("System clock before UNIX epoch: {}, using 0", e);
                0
            }
        };

        let snapshot_path = self.checkpoint_dir.join(format!("checkpoint_{}", height));
        let meta_path = self.checkpoint_dir.join(format!("checkpoint_{}.meta", height));

        // Create RocksDB checkpoint (native optimized snapshot) in a scope to drop borrow before prune
        {
            let checkpoint = rocksdb::checkpoint::Checkpoint::new(&self.db)
                .map_err(|e| CheckpointError::CreateFailed(e.to_string()))?;

            checkpoint
                .create_checkpoint(&snapshot_path)
                .map_err(|e| CheckpointError::CreateFailed(e.to_string()))?;
        } // checkpoint dropped here, releasing immutable borrow

        // Calculate snapshot size
        let size_bytes = Self::dir_size(&snapshot_path).unwrap_or(0);

        // Calculate checksum of the snapshot directory
        let checksum = Self::calculate_dir_checksum(&snapshot_path).unwrap_or_default();

        let metadata = CheckpointMetadata {
            height,
            block_hash,
            state_root,
            created_at: timestamp,
            size_bytes,
            checksum,
        };

        // Save metadata
        let meta_file = File::create(&meta_path)
            .map_err(|e| CheckpointError::CreateFailed(e.to_string()))?;
        let writer = BufWriter::new(meta_file);
        serde_json::to_writer_pretty(writer, &metadata)
            .map_err(|e| CheckpointError::CreateFailed(e.to_string()))?;

        self.checkpoints.insert(height, metadata.clone());

        // Prune old checkpoints
        self.prune_old_checkpoints();

        info!("Checkpoint created: {} bytes", size_bytes);

        Ok(metadata)
    }

    /// Get the nearest checkpoint at or below the given height
    pub fn get_nearest_checkpoint(&self, height: u64) -> Option<&CheckpointMetadata> {
        self.checkpoints
            .iter()
            .filter(|(h, _)| **h <= height)
            .max_by_key(|(h, _)| *h)
            .map(|(_, meta)| meta)
    }

    /// Get checkpoint at exact height
    pub fn get_checkpoint(&self, height: u64) -> Option<&CheckpointMetadata> {
        self.checkpoints.get(&height)
    }

    /// Get all available checkpoints
    pub fn list_checkpoints(&self) -> Vec<&CheckpointMetadata> {
        let mut checkpoints: Vec<_> = self.checkpoints.values().collect();
        checkpoints.sort_by_key(|c| c.height);
        checkpoints
    }

    /// Restore from a checkpoint
    pub fn restore_checkpoint(
        &self,
        height: u64,
        target_db_path: &Path,
    ) -> Result<(), CheckpointError> {
        let meta = self.checkpoints.get(&height)
            .ok_or(CheckpointError::NotFound(height))?;

        info!("Restoring from checkpoint at height {}", height);

        let snapshot_path = self.checkpoint_dir.join(format!("checkpoint_{}", height));

        // Verify checksum
        let current_checksum = Self::calculate_dir_checksum(&snapshot_path)
            .map_err(|e| CheckpointError::RestoreFailed(e.to_string()))?;

        if current_checksum != meta.checksum {
            return Err(CheckpointError::ChecksumMismatch);
        }

        // Copy snapshot to target
        Self::copy_dir_all(&snapshot_path, target_db_path)
            .map_err(|e| CheckpointError::RestoreFailed(e.to_string()))?;

        info!("Checkpoint restored successfully");

        Ok(())
    }

    /// Export checkpoint for network transfer
    pub fn export_checkpoint(&self, height: u64, output_path: &Path) -> Result<u64, CheckpointError> {
        let _meta = self.checkpoints.get(&height)
            .ok_or(CheckpointError::NotFound(height))?;

        let snapshot_path = self.checkpoint_dir.join(format!("checkpoint_{}", height));

        // Create tar.gz archive
        let file = File::create(output_path)
            .map_err(|e| CheckpointError::ExportFailed(e.to_string()))?;

        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);

        archive.append_dir_all("checkpoint", &snapshot_path)
            .map_err(|e| CheckpointError::ExportFailed(e.to_string()))?;

        archive.finish()
            .map_err(|e| CheckpointError::ExportFailed(e.to_string()))?;

        let size = fs::metadata(output_path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(size)
    }

    /// Import checkpoint from network transfer
    pub fn import_checkpoint(
        &mut self,
        archive_path: &Path,
        metadata: CheckpointMetadata,
    ) -> Result<(), CheckpointError> {
        let height = metadata.height;
        let snapshot_path = self.checkpoint_dir.join(format!("checkpoint_{}", height));

        // Extract archive
        let file = File::open(archive_path)
            .map_err(|e| CheckpointError::ImportFailed(e.to_string()))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        fs::create_dir_all(&snapshot_path)
            .map_err(|e| CheckpointError::ImportFailed(e.to_string()))?;

        // SECURITY: Validate all archive entries to prevent path traversal attacks.
        // A malicious archive could contain entries like "../../../etc/crontab"
        // that would write files outside the intended directory.
        let canonical_snapshot = snapshot_path.canonicalize()
            .map_err(|e| CheckpointError::ImportFailed(format!("Cannot canonicalize snapshot path: {}", e)))?;

        for entry_result in archive.entries()
            .map_err(|e| CheckpointError::ImportFailed(e.to_string()))? {
            let mut entry = entry_result
                .map_err(|e| CheckpointError::ImportFailed(e.to_string()))?;

            let entry_path = entry.path()
                .map_err(|e| CheckpointError::ImportFailed(e.to_string()))?
                .into_owned();

            // Check for absolute paths or path traversal sequences
            if entry_path.is_absolute() || entry_path.components().any(|c| c == std::path::Component::ParentDir) {
                return Err(CheckpointError::ImportFailed(
                    format!("Refusing to extract archive entry with path traversal: {:?}", entry_path)
                ));
            }

            let target = snapshot_path.join(&entry_path);
            // Verify the resolved target is still within our snapshot directory
            if let Ok(canonical_target) = target.canonicalize() {
                if !canonical_target.starts_with(&canonical_snapshot) {
                    return Err(CheckpointError::ImportFailed(
                        format!("Archive entry escapes snapshot directory: {:?}", entry_path)
                    ));
                }
            }

            // Safe to extract this entry
            entry.unpack_in(&snapshot_path)
                .map_err(|e| CheckpointError::ImportFailed(e.to_string()))?;
        }

        // Save metadata
        let meta_path = self.checkpoint_dir.join(format!("checkpoint_{}.meta", height));
        let meta_file = File::create(&meta_path)
            .map_err(|e| CheckpointError::ImportFailed(e.to_string()))?;
        serde_json::to_writer_pretty(meta_file, &metadata)
            .map_err(|e| CheckpointError::ImportFailed(e.to_string()))?;

        self.checkpoints.insert(height, metadata);

        Ok(())
    }

    fn prune_old_checkpoints(&mut self) {
        if self.checkpoints.len() <= MAX_CHECKPOINTS {
            return;
        }

        let mut heights: Vec<u64> = self.checkpoints.keys().copied().collect();
        heights.sort();

        while heights.len() > MAX_CHECKPOINTS {
            if let Some(oldest) = heights.first().copied() {
                heights.remove(0);
                self.checkpoints.remove(&oldest);

                // Delete files
                let snapshot_path = self.checkpoint_dir.join(format!("checkpoint_{}", oldest));
                let meta_path = self.checkpoint_dir.join(format!("checkpoint_{}.meta", oldest));
                let _ = fs::remove_dir_all(snapshot_path);
                let _ = fs::remove_file(meta_path);

                info!("Pruned old checkpoint at height {}", oldest);
            }
        }
    }

    fn dir_size(path: &Path) -> std::io::Result<u64> {
        let mut size = 0;
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();
                if entry_path.is_file() {
                    size += fs::metadata(&entry_path)?.len();
                } else if entry_path.is_dir() {
                    size += Self::dir_size(&entry_path)?;
                }
            }
        }
        Ok(size)
    }

    fn calculate_dir_checksum(path: &Path) -> std::io::Result<String> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();

        fn hash_dir(hasher: &mut sha2::Sha256, path: &Path) -> std::io::Result<()> {
            if path.is_dir() {
                let mut entries: Vec<_> = fs::read_dir(path)?.collect::<Result<_, _>>()?;
                entries.sort_by_key(|e| e.path());

                for entry in entries {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        let mut file = File::open(&entry_path)?;
                        let mut buffer = Vec::new();
                        file.read_to_end(&mut buffer)?;
                        hasher.update(&buffer);
                    } else if entry_path.is_dir() {
                        hash_dir(hasher, &entry_path)?;
                    }
                }
            }
            Ok(())
        }

        hash_dir(&mut hasher, path)?;
        Ok(hex::encode(hasher.finalize()))
    }

    fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if ty.is_dir() {
                Self::copy_dir_all(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }
}

/// Checkpoint errors
#[derive(Debug, Clone)]
pub enum CheckpointError {
    NotFound(u64),
    CreateFailed(String),
    RestoreFailed(String),
    ExportFailed(String),
    ImportFailed(String),
    ChecksumMismatch,
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(h) => write!(f, "Checkpoint not found at height {}", h),
            Self::CreateFailed(e) => write!(f, "Failed to create checkpoint: {}", e),
            Self::RestoreFailed(e) => write!(f, "Failed to restore checkpoint: {}", e),
            Self::ExportFailed(e) => write!(f, "Failed to export checkpoint: {}", e),
            Self::ImportFailed(e) => write!(f, "Failed to import checkpoint: {}", e),
            Self::ChecksumMismatch => write!(f, "Checkpoint checksum mismatch"),
        }
    }
}

impl std::error::Error for CheckpointError {}
