//! State Synchronization Protocol
//!
//! Enables new and catching-up nodes to quickly sync blockchain state
//! without replaying all historical transactions.
//!
//! # Architecture
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    State Sync Protocol                          │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  1. Discover Peers with State Snapshots                         │
//! │  2. Request State Chunks (account ranges)                       │
//! │  3. Verify Merkle Proofs                                       │
//! │  4. Apply State Chunks                                         │
//! │  5. Continue with Block Sync                                   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Protocol Messages
//! - `GetStateRange` - Request accounts in address range
//! - `StateRange` - Response with accounts and proofs
//! - `GetStorageRange` - Request contract storage
//! - `StorageRange` - Response with storage data

use std::collections::HashMap;
use std::time::Instant;
use luxtensor_core::types::{Address, Hash};
use luxtensor_core::Account;

/// State sync configuration
#[derive(Debug, Clone)]
pub struct StateSyncConfig {
    /// Maximum accounts per chunk
    pub chunk_size: usize,
    /// Parallel download limit
    pub parallel_downloads: usize,
    /// Chunk request timeout
    pub request_timeout_secs: u64,
    /// Maximum retries per chunk
    pub max_retries: u32,
    /// Verify merkle proofs
    pub verify_proofs: bool,
    /// Snapshot interval (blocks)
    pub snapshot_interval: u64,
}

impl Default for StateSyncConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            parallel_downloads: 4,
            request_timeout_secs: 30,
            max_retries: 3,
            verify_proofs: true,
            snapshot_interval: 4096, // ~12 hours at 12s blocks
        }
    }
}

/// State snapshot metadata
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    /// Block number
    pub block_number: u64,
    /// Block hash
    pub block_hash: Hash,
    /// State root
    pub state_root: Hash,
    /// Timestamp
    pub timestamp: u64,
    /// Total accounts
    pub account_count: u64,
    /// Total storage slots
    pub storage_count: u64,
}

/// State range request
#[derive(Debug, Clone)]
pub struct GetStateRange {
    /// Starting address (inclusive)
    pub start: Address,
    /// Ending address (inclusive)
    pub end: Address,
    /// State root to sync from
    pub state_root: Hash,
    /// Maximum accounts to return
    pub limit: usize,
}

/// State range response
#[derive(Debug, Clone)]
pub struct StateRange {
    /// Accounts in range
    pub accounts: Vec<(Address, Account)>,
    /// Merkle proof for the range
    pub proof: Vec<Hash>,
    /// Whether more accounts exist after this range
    pub has_more: bool,
    /// Next address to continue from (if has_more)
    pub continuation: Option<Address>,
}

/// Storage range request
#[derive(Debug, Clone)]
pub struct GetStorageRange {
    /// Contract address
    pub address: Address,
    /// Starting slot
    pub start_slot: [u8; 32],
    /// State root
    pub state_root: Hash,
    /// Maximum slots to return
    pub limit: usize,
}

/// Storage range response
#[derive(Debug, Clone)]
pub struct StorageRange {
    /// Storage slots
    pub slots: Vec<([u8; 32], [u8; 32])>,
    /// Merkle proof
    pub proof: Vec<Hash>,
    /// Whether more slots exist
    pub has_more: bool,
}

/// Sync progress tracking
#[derive(Debug, Clone, Default)]
pub struct SyncProgress {
    /// Current phase
    pub phase: SyncPhase,
    /// Accounts synced
    pub accounts_synced: u64,
    /// Total accounts (if known)
    pub total_accounts: Option<u64>,
    /// Storage slots synced
    pub storage_synced: u64,
    /// Bytes downloaded
    pub bytes_downloaded: u64,
    /// Start time
    pub started_at: Option<Instant>,
    /// Estimated completion
    pub eta_secs: Option<u64>,
}

impl SyncProgress {
    /// Calculate progress percentage
    pub fn percentage(&self) -> f64 {
        if let Some(total) = self.total_accounts {
            if total == 0 {
                return 100.0;
            }
            self.accounts_synced as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    }

    /// Calculate download speed (bytes/sec)
    pub fn download_speed(&self) -> f64 {
        if let Some(started) = self.started_at {
            let elapsed = started.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                return self.bytes_downloaded as f64 / elapsed;
            }
        }
        0.0
    }
}

/// Sync phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncPhase {
    #[default]
    Idle,
    Discovering,
    DownloadingAccounts,
    DownloadingStorage,
    VerifyingState,
    Completed,
    Failed,
}

/// State sync manager
pub struct StateSyncManager {
    /// Configuration
    config: StateSyncConfig,
    /// Target snapshot
    target_snapshot: Option<StateSnapshot>,
    /// Current progress
    progress: SyncProgress,
    /// Downloaded account chunks
    account_chunks: HashMap<Address, Vec<(Address, Account)>>,
    /// Downloaded storage chunks
    storage_chunks: HashMap<Address, Vec<([u8; 32], [u8; 32])>>,
    /// Pending chunk requests
    pending_requests: Vec<ChunkRequest>,
}

/// Chunk request tracking
#[derive(Debug, Clone)]
struct ChunkRequest {
    /// Request type
    request_type: ChunkType,
    /// Start address/slot
    start: [u8; 32],
    /// Sent timestamp
    sent_at: Instant,
    /// Retry count
    retries: u32,
}

#[derive(Debug, Clone, Copy)]
enum ChunkType {
    Account,
    Storage(Address),
}

impl StateSyncManager {
    /// Create new sync manager
    pub fn new(config: StateSyncConfig) -> Self {
        Self {
            config,
            target_snapshot: None,
            progress: SyncProgress::default(),
            account_chunks: HashMap::new(),
            storage_chunks: HashMap::new(),
            pending_requests: Vec::new(),
        }
    }

    /// Start syncing to a snapshot
    pub fn start_sync(&mut self, snapshot: StateSnapshot) {
        self.target_snapshot = Some(snapshot.clone());
        self.progress = SyncProgress {
            phase: SyncPhase::Discovering,
            total_accounts: Some(snapshot.account_count),
            started_at: Some(Instant::now()),
            ..Default::default()
        };
        self.account_chunks.clear();
        self.storage_chunks.clear();
        self.pending_requests.clear();
    }

    /// Process a state range response
    pub fn on_state_range(&mut self, response: StateRange) -> Result<(), SyncError> {
        // Verify proof if enabled
        if self.config.verify_proofs {
            self.verify_account_proof(&response)?;
        }

        // Store accounts
        for (addr, account) in &response.accounts {
            self.account_chunks
                .entry(*addr)
                .or_default()
                .push((*addr, account.clone()));
        }

        // Update progress
        self.progress.accounts_synced += response.accounts.len() as u64;
        self.progress.bytes_downloaded += estimate_size(&response.accounts);

        // Queue next request if more data
        if response.has_more {
            if let Some(next) = response.continuation {
                self.queue_account_request(next);
            }
        } else {
            // Move to storage sync
            self.progress.phase = SyncPhase::DownloadingStorage;
        }

        Ok(())
    }

    /// Process a storage range response
    pub fn on_storage_range(
        &mut self,
        address: Address,
        response: StorageRange,
    ) -> Result<(), SyncError> {
        // Verify proof if enabled
        if self.config.verify_proofs {
            self.verify_storage_proof(&address, &response)?;
        }

        // Store storage slots
        self.storage_chunks.entry(address).or_default().extend(response.slots.iter().cloned());

        // Update progress
        self.progress.storage_synced += response.slots.len() as u64;
        self.progress.bytes_downloaded += response.slots.len() as u64 * 64;

        Ok(())
    }

    /// Get current sync progress
    pub fn progress(&self) -> &SyncProgress {
        &self.progress
    }

    /// Check if sync is complete
    pub fn is_complete(&self) -> bool {
        self.progress.phase == SyncPhase::Completed
    }

    /// Generate initial account range requests
    pub fn generate_initial_requests(&self) -> Vec<GetStateRange> {
        let snapshot = match &self.target_snapshot {
            Some(s) => s,
            None => return vec![],
        };

        // Divide address space into chunks for parallel download
        let mut requests = Vec::new();
        let chunk_count = self.config.parallel_downloads;

        for i in 0..chunk_count {
            let start = address_at_fraction(i, chunk_count);
            let end = if i == chunk_count - 1 {
                Address::from([0xff; 20])
            } else {
                address_at_fraction(i + 1, chunk_count)
            };

            requests.push(GetStateRange {
                start,
                end,
                state_root: snapshot.state_root,
                limit: self.config.chunk_size,
            });
        }

        requests
    }

    /// Finalize sync and verify state root
    pub fn finalize(&mut self) -> Result<Hash, SyncError> {
        self.progress.phase = SyncPhase::VerifyingState;

        // Calculate final state root from synced data
        let calculated_root = self.calculate_state_root()?;

        // Verify against target
        if let Some(snapshot) = &self.target_snapshot {
            if calculated_root != snapshot.state_root {
                self.progress.phase = SyncPhase::Failed;
                return Err(SyncError::StateRootMismatch {
                    expected: snapshot.state_root,
                    got: calculated_root,
                });
            }
        }

        self.progress.phase = SyncPhase::Completed;

        // Calculate ETA
        if let Some(started) = self.progress.started_at {
            self.progress.eta_secs = Some(started.elapsed().as_secs());
        }

        Ok(calculated_root)
    }

    /// Get all synced accounts
    pub fn get_accounts(&self) -> impl Iterator<Item = (&Address, &Account)> {
        self.account_chunks.values()
            .flat_map(|chunk| chunk.iter().map(|(addr, acc)| (addr, acc)))
    }

    /// Get synced storage for an address
    pub fn get_storage(&self, address: &Address) -> Option<&Vec<([u8; 32], [u8; 32])>> {
        self.storage_chunks.get(address)
    }

    // Internal helpers

    fn queue_account_request(&mut self, start: Address) {
        self.pending_requests.push(ChunkRequest {
            request_type: ChunkType::Account,
            start: {
                let mut arr = [0u8; 32];
                arr[12..].copy_from_slice(start.as_bytes());
                arr
            },
            sent_at: Instant::now(),
            retries: 0,
        });
    }

    fn verify_account_proof(&self, _response: &StateRange) -> Result<(), SyncError> {
        // SECURITY: State sync proof verification requires integration with the
        // Merkle Patricia Trie implementation in luxtensor-storage.
        // Until the trie is wired into the snap-sync pivot, reject all unverified data.
        Err(SyncError::InvalidProof(
            "Account proof verification not yet integrated with trie — rejecting for safety. \
             Disable verify_proofs only in trusted/test environments."
                .to_string(),
        ))
    }

    fn verify_storage_proof(&self, _address: &Address, _response: &StorageRange) -> Result<(), SyncError> {
        // SECURITY: Storage proof verification is NOT yet implemented.
        // Reject all unverified storage data.
        Err(SyncError::InvalidProof(
            "Storage proof verification not yet implemented — rejecting for safety. \
             Disable verify_proofs only in trusted/test environments."
                .to_string(),
        ))
    }

    fn calculate_state_root(&self) -> Result<Hash, SyncError> {
        // Collect all accounts
        let mut accounts: Vec<(&Address, &Account)> = self.get_accounts().collect();
        accounts.sort_by_key(|(addr, _)| *addr);

        if accounts.is_empty() {
            return Ok([0u8; 32]);
        }

        // Build Merkle tree
        let leaves: Vec<Hash> = accounts.iter().map(|(addr, acc)| {
            let mut data = Vec::new();
            data.extend_from_slice(addr.as_bytes());
            data.extend_from_slice(&acc.balance.to_le_bytes());
            data.extend_from_slice(&acc.nonce.to_le_bytes());
            luxtensor_crypto::keccak256(&data)
        }).collect();

        Ok(luxtensor_crypto::MerkleTree::new(leaves).root())
    }
}

impl Default for StateSyncManager {
    fn default() -> Self {
        Self::new(StateSyncConfig::default())
    }
}

/// State sync errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum SyncError {
    #[error("State root mismatch: expected {expected:?}, got {got:?}")]
    StateRootMismatch { expected: Hash, got: Hash },

    #[error("Invalid proof: {0}")]
    InvalidProof(String),

    #[error("Chunk request timeout")]
    Timeout,

    #[error("No peers available for sync")]
    NoPeers,

    #[error("Sync cancelled")]
    Cancelled,
}

/// Estimate size of account data
fn estimate_size(accounts: &[(Address, Account)]) -> u64 {
    accounts.len() as u64 * (20 + 16 + 8) // addr + balance + nonce
}

/// Generate address at position in address space
fn address_at_fraction(index: usize, total: usize) -> Address {
    if total == 0 || index >= total {
        return Address::from([0xff; 20]);
    }

    let fraction = index as f64 / total as f64;
    let first_byte = (fraction * 256.0) as u8;

    let mut bytes = [0u8; 20];
    bytes[0] = first_byte;
    Address::from(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_default() {
        let config = StateSyncConfig::default();
        assert_eq!(config.chunk_size, 1000);
        assert_eq!(config.parallel_downloads, 4);
    }

    #[test]
    fn test_sync_progress() {
        let mut progress = SyncProgress {
            accounts_synced: 500,
            total_accounts: Some(1000),
            ..Default::default()
        };
        assert!((progress.percentage() - 50.0).abs() < 0.01);

        progress.accounts_synced = 1000;
        assert!((progress.percentage() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_address_at_fraction() {
        let start = address_at_fraction(0, 4);
        assert_eq!(start.as_bytes()[0], 0);

        let mid = address_at_fraction(2, 4);
        assert_eq!(mid.as_bytes()[0], 128);

        let end = address_at_fraction(3, 4);
        assert_eq!(end.as_bytes()[0], 192);
    }

    #[test]
    fn test_generate_initial_requests() {
        let manager = StateSyncManager::new(StateSyncConfig {
            parallel_downloads: 4,
            chunk_size: 100,
            ..Default::default()
        });

        let snapshot = StateSnapshot {
            block_number: 1000,
            block_hash: [1u8; 32],
            state_root: [2u8; 32],
            timestamp: 12345,
            account_count: 10000,
            storage_count: 50000,
        };

        let mut manager = manager;
        manager.target_snapshot = Some(snapshot);

        let requests = manager.generate_initial_requests();
        assert_eq!(requests.len(), 4);
    }

    #[test]
    fn test_sync_lifecycle() {
        let mut manager = StateSyncManager::default();

        let snapshot = StateSnapshot {
            block_number: 100,
            block_hash: [1u8; 32],
            state_root: [0u8; 32], // Empty state
            timestamp: 12345,
            account_count: 0,
            storage_count: 0,
        };

        manager.start_sync(snapshot);
        assert_eq!(manager.progress.phase, SyncPhase::Discovering);

        // Finalize with empty state
        let result = manager.finalize();
        assert!(result.is_ok());
        assert!(manager.is_complete());
    }
}
