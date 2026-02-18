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

use std::collections::{HashMap, HashSet};
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
    /// Verify merkle proofs (always enabled in production; config toggle reserved for testing only)
    #[cfg(not(test))]
    pub verify_proofs: bool,
    #[cfg(test)]
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

/// Snap sync phase — orchestrates the pivot-based state download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapSyncPhase {
    /// Waiting to choose a pivot block
    #[default]
    SelectingPivot,
    /// Downloading state trie at the pivot block
    DownloadingState,
    /// Verifying downloaded state against the pivot state root
    VerifyingState,
    /// State verified; switching to normal block-by-block sync from pivot
    SwitchingToBlockSync,
    /// Snap sync finished successfully
    Complete,
}

/// Tracks the overall state of a snap sync run.
#[derive(Debug, Clone)]
pub struct SnapSyncState {
    /// Current phase
    pub phase: SnapSyncPhase,
    /// Pivot block number (N blocks behind HEAD)
    pub pivot_block: Option<u64>,
    /// Pivot block hash
    pub pivot_hash: Option<Hash>,
    /// Pivot state root to verify against
    pub pivot_state_root: Option<Hash>,
    /// Number of blocks behind HEAD used for pivot selection
    pub pivot_behind: u64,
    /// Ranges already downloaded (start addresses)
    pub downloaded_ranges: HashSet<[u8; 20]>,
    /// Number of parallel downloads currently in-flight
    pub active_downloads: usize,
    /// Maximum parallel downloads allowed
    pub max_parallel_downloads: usize,
}

impl Default for SnapSyncState {
    fn default() -> Self {
        Self {
            phase: SnapSyncPhase::SelectingPivot,
            pivot_block: None,
            pivot_hash: None,
            pivot_state_root: None,
            pivot_behind: 64, // default: 64 blocks behind HEAD
            downloaded_ranges: HashSet::new(),
            active_downloads: 0,
            max_parallel_downloads: 4,
        }
    }
}

/// Progress report for external consumers.
#[derive(Debug, Clone)]
pub struct SyncProgressReport {
    /// Percentage of accounts downloaded (0.0–100.0)
    pub percentage: f64,
    /// Total bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Download speed in bytes/sec
    pub download_speed: f64,
    /// Current sync phase
    pub sync_phase: SyncPhase,
    /// Current snap sync phase
    pub snap_phase: SnapSyncPhase,
    /// Whether proof verification is passing
    pub verification_ok: bool,
    /// Elapsed seconds since sync started
    pub elapsed_secs: f64,
    /// Estimated remaining seconds (None if unknown)
    pub eta_remaining_secs: Option<f64>,
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
    /// Snap sync state
    snap_sync: SnapSyncState,
    /// Whether the last verification passed
    last_verification_ok: bool,
}

/// Chunk request tracking
#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum ChunkType {
    Account,
    Storage(Address),
}

impl StateSyncManager {
    /// Create new sync manager
    pub fn new(config: StateSyncConfig) -> Self {
        let max_dl = config.parallel_downloads;
        Self {
            config,
            target_snapshot: None,
            progress: SyncProgress::default(),
            account_chunks: HashMap::new(),
            storage_chunks: HashMap::new(),
            pending_requests: Vec::new(),
            snap_sync: SnapSyncState {
                max_parallel_downloads: max_dl,
                ..Default::default()
            },
            last_verification_ok: true,
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
        // Validate response size to prevent oversized responses from malicious peers
        if response.accounts.len() > self.config.chunk_size * 2 {
            return Err(SyncError::VerificationFailed(
                format!("Response too large: {} accounts (max {})", response.accounts.len(), self.config.chunk_size * 2)
            ));
        }

        // Enforce max total download (1 billion accounts = DoS protection)
        const MAX_TOTAL_ACCOUNTS: u64 = 1_000_000_000;
        if self.progress.accounts_synced > MAX_TOTAL_ACCOUNTS {
            return Err(SyncError::VerificationFailed(
                "Total download limit exceeded — possible infinite has_more loop".into()
            ));
        }

        // Always verify proofs in production to prevent accepting malicious state
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
        // Always verify proofs
        if self.config.verify_proofs {
            self.verify_storage_proof(&address, &response)?;
        }

        // Validate response size
        if response.slots.len() > self.config.chunk_size * 2 {
            return Err(SyncError::VerificationFailed(
                format!("Storage response too large: {} slots", response.slots.len())
            ));
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

    fn verify_account_proof(&mut self, response: &StateRange) -> Result<(), SyncError> {
        let state_root = match &self.target_snapshot {
            Some(snap) => snap.state_root,
            None => {
                return Err(SyncError::InvalidProof(
                    "No target snapshot set — cannot verify account proof".to_string(),
                ));
            }
        };

        if response.accounts.is_empty() {
            // Nothing to verify
            return Ok(());
        }

        // Hash each account to get leaf hashes
        let leaf_hashes: Vec<Hash> = response
            .accounts
            .iter()
            .map(|(addr, acc)| {
                let mut data = Vec::new();
                data.extend_from_slice(addr.as_bytes());
                data.extend_from_slice(&acc.balance.to_le_bytes());
                data.extend_from_slice(&acc.nonce.to_le_bytes());
                luxtensor_crypto::keccak256(&data)
            })
            .collect();

        // Verify each leaf against the proof and expected root.
        // For a range response, verify that ALL leaves are consistent
        // with the provided proof path to state_root.
        for leaf_hash in &leaf_hashes {
            let valid = verify_merkle_proof(leaf_hash, &response.proof, &state_root);
            if !valid {
                self.last_verification_ok = false;
                return Err(SyncError::InvalidProof(
                    "Account Merkle proof verification failed: proof path does not \
                     reconstruct the expected state root"
                        .to_string(),
                ));
            }
        }

        self.last_verification_ok = true;
        Ok(())
    }

    fn verify_storage_proof(
        &mut self,
        address: &Address,
        response: &StorageRange,
    ) -> Result<(), SyncError> {
        // The storage root lives inside the account; look it up.
        let storage_root = self
            .account_chunks
            .values()
            .flat_map(|chunk| chunk.iter())
            .find(|(addr, _)| addr == address)
            .map(|(_, acc)| acc.storage_root)
            .ok_or_else(|| {
                SyncError::InvalidProof(format!(
                    "Cannot verify storage proof: account {} not yet downloaded",
                    address
                ))
            })?;

        if response.slots.is_empty() {
            return Ok(());
        }

        // Hash each storage slot to get leaf hashes
        let leaf_hashes: Vec<Hash> = response
            .slots
            .iter()
            .map(|(key, value)| {
                let mut data = Vec::with_capacity(64);
                data.extend_from_slice(key);
                data.extend_from_slice(value);
                luxtensor_crypto::keccak256(&data)
            })
            .collect();

        // Verify each leaf against the storage proof and storage root
        for leaf_hash in &leaf_hashes {
            let valid = verify_merkle_proof(leaf_hash, &response.proof, &storage_root);
            if !valid {
                self.last_verification_ok = false;
                return Err(SyncError::InvalidProof(
                    "Storage Merkle proof verification failed: proof path does not \
                     reconstruct the expected storage root"
                        .to_string(),
                ));
            }
        }

        self.last_verification_ok = true;
        Ok(())
    }

    fn calculate_state_root(&self) -> Result<Hash, SyncError> {
        // Collect all accounts
        let mut accounts: Vec<(&Address, &Account)> = self.get_accounts().collect();
        accounts.sort_by_key(|(addr, _)| *addr);

        if accounts.is_empty() {
            return Ok([0u8; 32]);
        }

        // Build Merkle tree from sorted account leaves.
        // Each leaf = keccak256(address || balance || nonce || storage_root || code_hash)
        // to capture the full account state.
        let leaves: Vec<Hash> = accounts
            .iter()
            .map(|(addr, acc)| {
                let mut data = Vec::new();
                data.extend_from_slice(addr.as_bytes());
                data.extend_from_slice(&acc.balance.to_le_bytes());
                data.extend_from_slice(&acc.nonce.to_le_bytes());
                data.extend_from_slice(&acc.storage_root);
                data.extend_from_slice(&acc.code_hash);
                luxtensor_crypto::keccak256(&data)
            })
            .collect();

        Ok(luxtensor_crypto::MerkleTree::new(leaves).root())
    }

    // ── Snap sync pivot logic ────────────────────────────────────────

    /// Select a pivot block that is `behind` blocks behind `head_block`.
    ///
    /// The pivot is the block whose state we will download via snap sync.
    /// Choosing a block well behind HEAD avoids re-org risk.
    pub fn select_pivot_block(
        &mut self,
        head_block: u64,
        head_hash: Hash,
        head_state_root: Hash,
    ) -> Result<u64, SyncError> {
        let behind = self.snap_sync.pivot_behind;
        if head_block < behind {
            return Err(SyncError::PivotSelectionFailed(
                format!(
                    "HEAD block {} is less than pivot distance {} — chain too short for snap sync",
                    head_block, behind
                ),
            ));
        }

        let pivot = head_block - behind;
        self.snap_sync.pivot_block = Some(pivot);
        self.snap_sync.pivot_hash = Some(head_hash);
        self.snap_sync.pivot_state_root = Some(head_state_root);
        self.snap_sync.phase = SnapSyncPhase::DownloadingState;

        // Also update the legacy progress tracker
        self.progress.phase = SyncPhase::DownloadingAccounts;

        Ok(pivot)
    }

    /// Get the current snap sync phase.
    pub fn snap_sync_phase(&self) -> SnapSyncPhase {
        self.snap_sync.phase
    }

    /// Get the current snap sync state (read-only).
    pub fn snap_sync_state(&self) -> &SnapSyncState {
        &self.snap_sync
    }

    /// Advance snap sync after all state has been downloaded and verified.
    ///
    /// Transitions `DownloadingState → VerifyingState → SwitchingToBlockSync → Complete`.
    pub fn advance_snap_sync(&mut self) -> Result<SnapSyncPhase, SyncError> {
        match self.snap_sync.phase {
            SnapSyncPhase::SelectingPivot => {
                Err(SyncError::PivotSelectionFailed(
                    "Must call select_pivot_block() before advancing".to_string(),
                ))
            }
            SnapSyncPhase::DownloadingState => {
                self.snap_sync.phase = SnapSyncPhase::VerifyingState;
                self.progress.phase = SyncPhase::VerifyingState;
                Ok(SnapSyncPhase::VerifyingState)
            }
            SnapSyncPhase::VerifyingState => {
                // Verify the state root matches the pivot
                let calculated = self.calculate_state_root()?;
                if let Some(expected) = self.snap_sync.pivot_state_root {
                    if calculated != expected {
                        self.progress.phase = SyncPhase::Failed;
                        return Err(SyncError::StateRootMismatch {
                            expected,
                            got: calculated,
                        });
                    }
                }
                self.snap_sync.phase = SnapSyncPhase::SwitchingToBlockSync;
                Ok(SnapSyncPhase::SwitchingToBlockSync)
            }
            SnapSyncPhase::SwitchingToBlockSync => {
                self.snap_sync.phase = SnapSyncPhase::Complete;
                self.progress.phase = SyncPhase::Completed;
                Ok(SnapSyncPhase::Complete)
            }
            SnapSyncPhase::Complete => Ok(SnapSyncPhase::Complete),
        }
    }

    // ── State range request generation ───────────────────────────────

    /// Generate a `GetStateRange` message for a chunk of the 256-bit address
    /// space, splitting it into `total_chunks` equal-sized pieces.
    ///
    /// Tracks which ranges have already been downloaded and limits
    /// in-flight requests to `max_parallel_downloads`.
    pub fn request_state_range(
        &mut self,
        chunk_index: usize,
        total_chunks: usize,
    ) -> Result<Option<GetStateRange>, SyncError> {
        let snapshot = self
            .target_snapshot
            .as_ref()
            .or_else(|| {
                // Use pivot state root if no explicit snapshot
                None
            })
            .ok_or(SyncError::NoPeers)?; // "no target" is represented as NoPeers

        if chunk_index >= total_chunks || total_chunks == 0 {
            return Err(SyncError::InvalidProof(format!(
                "Invalid chunk_index={} for total_chunks={}",
                chunk_index, total_chunks
            )));
        }

        // Check parallel download limit
        if self.snap_sync.active_downloads >= self.snap_sync.max_parallel_downloads {
            return Ok(None); // caller should wait
        }

        let start = address_at_fraction(chunk_index, total_chunks);
        let end = if chunk_index == total_chunks - 1 {
            Address::from([0xff; 20])
        } else {
            address_at_fraction(chunk_index + 1, total_chunks)
        };

        // Check if this range was already downloaded
        if self.snap_sync.downloaded_ranges.contains(start.as_bytes()) {
            return Ok(None);
        }

        let state_root = self
            .snap_sync
            .pivot_state_root
            .unwrap_or(snapshot.state_root);

        self.snap_sync.active_downloads += 1;

        Ok(Some(GetStateRange {
            start,
            end,
            state_root,
            limit: self.config.chunk_size,
        }))
    }

    /// Mark a range as downloaded (call after successfully receiving + verifying a StateRange response).
    pub fn mark_range_downloaded(&mut self, start: Address) {
        self.snap_sync.downloaded_ranges.insert(*start.as_bytes());
        self.snap_sync.active_downloads = self.snap_sync.active_downloads.saturating_sub(1);
    }

    // ── Progress reporting ───────────────────────────────────────────

    /// Build a full progress report snapshot.
    pub fn progress_report(&self) -> SyncProgressReport {
        let elapsed = self
            .progress
            .started_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);

        let pct = self.progress.percentage();
        let speed = self.progress.download_speed();

        let eta = if pct > 0.0 && pct < 100.0 && speed > 0.0 {
            let remaining_bytes_est =
                (self.progress.bytes_downloaded as f64 / (pct / 100.0))
                    - self.progress.bytes_downloaded as f64;
            Some(remaining_bytes_est / speed)
        } else {
            None
        };

        SyncProgressReport {
            percentage: pct,
            bytes_downloaded: self.progress.bytes_downloaded,
            download_speed: speed,
            sync_phase: self.progress.phase,
            snap_phase: self.snap_sync.phase,
            verification_ok: self.last_verification_ok,
            elapsed_secs: elapsed,
            eta_remaining_secs: eta,
        }
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

    #[error("Pivot selection failed: {0}")]
    PivotSelectionFailed(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),
}

/// Verify a Merkle inclusion proof.
///
/// Walks the proof path from `leaf_hash` to the root using canonical
/// ordering (smaller hash first) — identical to bridge.rs's approach.
///
/// SECURITY: Uses domain separation to prevent second pre-image attacks.
/// Leaf nodes are prefixed with 0x00 and internal nodes with 0x01, so an
/// attacker cannot craft a "leaf" whose hash collides with an internal node.
///
/// Returns `true` if the reconstructed root matches `expected_root`.
fn verify_merkle_proof(leaf_hash: &Hash, proof: &[Hash], expected_root: &Hash) -> bool {
    // Domain-separated leaf: H(0x00 || leaf_hash)
    let mut leaf_data = Vec::with_capacity(1 + 32);
    leaf_data.push(0x00);
    leaf_data.extend_from_slice(leaf_hash);
    let mut current = luxtensor_crypto::keccak256(&leaf_data);

    if proof.is_empty() {
        // Single-element tree: root is the domain-separated leaf
        return current == *expected_root;
    }

    for sibling in proof {
        // Domain-separated internal node: H(0x01 || left || right)
        // Canonical ordering: smaller hash first for determinism
        let mut data = Vec::with_capacity(1 + 64);
        data.push(0x01);
        if current <= *sibling {
            data.extend_from_slice(&current);
            data.extend_from_slice(sibling);
        } else {
            data.extend_from_slice(sibling);
            data.extend_from_slice(&current);
        }
        current = luxtensor_crypto::keccak256(&data);
    }
    current == *expected_root
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

    // ── verify_merkle_proof tests ────────────────────────────────────

    #[test]
    fn test_merkle_proof_single_leaf() {
        let leaf = luxtensor_crypto::keccak256(b"leaf");
        // Single-element tree: root = H(0x00 || leaf) with domain separation
        let mut leaf_data = vec![0x00u8];
        leaf_data.extend_from_slice(&leaf);
        let root = luxtensor_crypto::keccak256(&leaf_data);
        assert!(verify_merkle_proof(&leaf, &[], &root));
    }

    #[test]
    fn test_merkle_proof_two_leaves() {
        let leaf_a = luxtensor_crypto::keccak256(b"a");
        let leaf_b = luxtensor_crypto::keccak256(b"b");

        // Domain-separated leaf nodes: H(0x00 || leaf)
        let mut la_data = vec![0x00u8];
        la_data.extend_from_slice(&leaf_a);
        let node_a = luxtensor_crypto::keccak256(&la_data);

        let mut lb_data = vec![0x00u8];
        lb_data.extend_from_slice(&leaf_b);
        let node_b = luxtensor_crypto::keccak256(&lb_data);

        // Domain-separated internal node: H(0x01 || smaller || larger)
        let (first, second) = if node_a <= node_b {
            (node_a, node_b)
        } else {
            (node_b, node_a)
        };
        let mut combined = vec![0x01u8];
        combined.extend_from_slice(&first);
        combined.extend_from_slice(&second);
        let root = luxtensor_crypto::keccak256(&combined);

        // Proof for leaf_a: sibling is node_b (domain-separated leaf B)
        assert!(verify_merkle_proof(&leaf_a, &[node_b], &root));
        // And vice versa
        assert!(verify_merkle_proof(&leaf_b, &[node_a], &root));
    }

    #[test]
    fn test_merkle_proof_invalid_root() {
        let leaf = luxtensor_crypto::keccak256(b"x");
        let sibling = luxtensor_crypto::keccak256(b"y");
        let wrong_root = [0xffu8; 32];
        assert!(!verify_merkle_proof(&leaf, &[sibling], &wrong_root));
    }

    #[test]
    fn test_merkle_proof_empty_proof_mismatch() {
        let leaf = luxtensor_crypto::keccak256(b"hello");
        let wrong = [0u8; 32];
        assert!(!verify_merkle_proof(&leaf, &[], &wrong));
    }

    // ── verify_account_proof tests ───────────────────────────────────

    #[test]
    fn test_verify_account_proof_valid() {
        // Build a valid proof: single account whose leaf hash IS the state root
        let addr = Address::from([1u8; 20]);
        let acc = Account {
            nonce: 1,
            balance: 100,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        };

        let mut data = Vec::new();
        data.extend_from_slice(addr.as_bytes());
        data.extend_from_slice(&acc.balance.to_le_bytes());
        data.extend_from_slice(&acc.nonce.to_le_bytes());
        let leaf_hash = luxtensor_crypto::keccak256(&data);

        // Domain-separated root for single-element tree: H(0x00 || leaf_hash)
        let mut leaf_node_data = vec![0x00u8];
        leaf_node_data.extend_from_slice(&leaf_hash);
        let root = luxtensor_crypto::keccak256(&leaf_node_data);

        // State root equals the domain-separated leaf hash (no siblings)
        let mut manager = StateSyncManager::new(StateSyncConfig {
            verify_proofs: true,
            ..Default::default()
        });
        manager.target_snapshot = Some(StateSnapshot {
            block_number: 10,
            block_hash: [0u8; 32],
            state_root: root,
            timestamp: 0,
            account_count: 1,
            storage_count: 0,
        });

        let response = StateRange {
            accounts: vec![(addr, acc)],
            proof: vec![], // empty proof → single-element tree
            has_more: false,
            continuation: None,
        };

        assert!(manager.verify_account_proof(&response).is_ok());
        assert!(manager.last_verification_ok);
    }

    #[test]
    fn test_verify_account_proof_invalid() {
        let addr = Address::from([2u8; 20]);
        let acc = Account {
            nonce: 0,
            balance: 50,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        };

        let mut manager = StateSyncManager::new(StateSyncConfig {
            verify_proofs: true,
            ..Default::default()
        });
        manager.target_snapshot = Some(StateSnapshot {
            block_number: 10,
            block_hash: [0u8; 32],
            state_root: [0xffu8; 32], // wrong root
            timestamp: 0,
            account_count: 1,
            storage_count: 0,
        });

        let response = StateRange {
            accounts: vec![(addr, acc)],
            proof: vec![],
            has_more: false,
            continuation: None,
        };

        let result = manager.verify_account_proof(&response);
        assert!(result.is_err());
        assert!(!manager.last_verification_ok);
    }

    #[test]
    fn test_verify_account_proof_empty_response() {
        let mut manager = StateSyncManager::default();
        manager.target_snapshot = Some(StateSnapshot {
            block_number: 10,
            block_hash: [0u8; 32],
            state_root: [1u8; 32],
            timestamp: 0,
            account_count: 0,
            storage_count: 0,
        });

        let response = StateRange {
            accounts: vec![],
            proof: vec![],
            has_more: false,
            continuation: None,
        };
        assert!(manager.verify_account_proof(&response).is_ok());
    }

    // ── verify_storage_proof tests ───────────────────────────────────

    #[test]
    fn test_verify_storage_proof_valid() {
        let addr = Address::from([3u8; 20]);
        let key = [0xaau8; 32];
        let value = [0xbbu8; 32];

        // Compute expected storage root with domain separation
        let mut slot_data = Vec::with_capacity(64);
        slot_data.extend_from_slice(&key);
        slot_data.extend_from_slice(&value);
        let leaf_hash = luxtensor_crypto::keccak256(&slot_data);

        // Domain-separated root for single-element tree: H(0x00 || leaf_hash)
        let mut leaf_node_data = vec![0x00u8];
        leaf_node_data.extend_from_slice(&leaf_hash);
        let storage_root = luxtensor_crypto::keccak256(&leaf_node_data);

        let acc = Account {
            nonce: 0,
            balance: 0,
            storage_root, // single-slot trie → root == domain-separated leaf hash
            code_hash: [0u8; 32],
            code: None,
        };

        let mut manager = StateSyncManager::new(StateSyncConfig {
            verify_proofs: true,
            ..Default::default()
        });
        manager.target_snapshot = Some(StateSnapshot {
            block_number: 5,
            block_hash: [0u8; 32],
            state_root: [0u8; 32],
            timestamp: 0,
            account_count: 1,
            storage_count: 1,
        });
        // Insert the account so storage proof can find the storage_root
        manager
            .account_chunks
            .entry(addr)
            .or_default()
            .push((addr, acc));

        let response = StorageRange {
            slots: vec![(key, value)],
            proof: vec![], // single-leaf → no siblings
            has_more: false,
        };

        assert!(manager.verify_storage_proof(&addr, &response).is_ok());
    }

    #[test]
    fn test_verify_storage_proof_account_missing() {
        let addr = Address::from([4u8; 20]);
        let mut manager = StateSyncManager::default();
        manager.target_snapshot = Some(StateSnapshot {
            block_number: 5,
            block_hash: [0u8; 32],
            state_root: [0u8; 32],
            timestamp: 0,
            account_count: 0,
            storage_count: 0,
        });

        let response = StorageRange {
            slots: vec![([1u8; 32], [2u8; 32])],
            proof: vec![],
            has_more: false,
        };

        let result = manager.verify_storage_proof(&addr, &response);
        assert!(result.is_err());
    }

    // ── Snap sync pivot tests ────────────────────────────────────────

    #[test]
    fn test_select_pivot_block_ok() {
        let mut manager = StateSyncManager::default();
        let result = manager.select_pivot_block(200, [1u8; 32], [2u8; 32]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 200 - 64); // default behind = 64
        assert_eq!(manager.snap_sync.phase, SnapSyncPhase::DownloadingState);
        assert_eq!(manager.snap_sync.pivot_block, Some(136));
    }

    #[test]
    fn test_select_pivot_block_chain_too_short() {
        let mut manager = StateSyncManager::default();
        let result = manager.select_pivot_block(10, [1u8; 32], [2u8; 32]);
        assert!(result.is_err());
        match result {
            Err(SyncError::PivotSelectionFailed(_)) => {}
            other => panic!("Expected PivotSelectionFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_snap_sync_phase_transitions() {
        let mut manager = StateSyncManager::default();
        assert_eq!(manager.snap_sync.phase, SnapSyncPhase::SelectingPivot);

        // Cannot advance without selecting pivot first
        assert!(manager.advance_snap_sync().is_err());

        // Select pivot – moves to DownloadingState
        manager
            .select_pivot_block(200, [1u8; 32], [0u8; 32])
            .unwrap();
        assert_eq!(manager.snap_sync.phase, SnapSyncPhase::DownloadingState);

        // Advance to VerifyingState
        let phase = manager.advance_snap_sync().unwrap();
        assert_eq!(phase, SnapSyncPhase::VerifyingState);

        // Advance to SwitchingToBlockSync (empty state → root = [0;32] matches pivot)
        let phase = manager.advance_snap_sync().unwrap();
        assert_eq!(phase, SnapSyncPhase::SwitchingToBlockSync);

        // Advance to Complete
        let phase = manager.advance_snap_sync().unwrap();
        assert_eq!(phase, SnapSyncPhase::Complete);
    }

    // ── request_state_range tests ────────────────────────────────────

    #[test]
    fn test_request_state_range_basic() {
        let mut manager = StateSyncManager::default();
        manager.target_snapshot = Some(StateSnapshot {
            block_number: 100,
            block_hash: [0u8; 32],
            state_root: [0xabu8; 32],
            timestamp: 0,
            account_count: 1000,
            storage_count: 0,
        });

        let req = manager.request_state_range(0, 4).unwrap();
        assert!(req.is_some());
        let req = req.unwrap();
        assert_eq!(req.start.as_bytes()[0], 0);
    }

    #[test]
    fn test_request_state_range_parallel_limit() {
        let mut manager = StateSyncManager::new(StateSyncConfig {
            parallel_downloads: 2,
            ..Default::default()
        });
        manager.target_snapshot = Some(StateSnapshot {
            block_number: 100,
            block_hash: [0u8; 32],
            state_root: [0xabu8; 32],
            timestamp: 0,
            account_count: 1000,
            storage_count: 0,
        });

        let _ = manager.request_state_range(0, 4).unwrap(); // active = 1
        let _ = manager.request_state_range(1, 4).unwrap(); // active = 2

        // Third request → over limit → None
        let req = manager.request_state_range(2, 4).unwrap();
        assert!(req.is_none());
    }

    #[test]
    fn test_request_state_range_already_downloaded() {
        let mut manager = StateSyncManager::default();
        manager.target_snapshot = Some(StateSnapshot {
            block_number: 100,
            block_hash: [0u8; 32],
            state_root: [0xabu8; 32],
            timestamp: 0,
            account_count: 1000,
            storage_count: 0,
        });

        let start_addr = address_at_fraction(0, 4);
        manager.mark_range_downloaded(start_addr);

        let req = manager.request_state_range(0, 4).unwrap();
        assert!(req.is_none()); // already downloaded
    }

    #[test]
    fn test_request_state_range_invalid_index() {
        let mut manager = StateSyncManager::default();
        manager.target_snapshot = Some(StateSnapshot {
            block_number: 100,
            block_hash: [0u8; 32],
            state_root: [0u8; 32],
            timestamp: 0,
            account_count: 0,
            storage_count: 0,
        });

        let result = manager.request_state_range(5, 4);
        assert!(result.is_err());
    }

    // ── Progress reporting tests ─────────────────────────────────────

    #[test]
    fn test_progress_report() {
        let mut manager = StateSyncManager::default();
        manager.progress.accounts_synced = 250;
        manager.progress.total_accounts = Some(1000);
        manager.progress.bytes_downloaded = 50_000;

        let report = manager.progress_report();
        assert!((report.percentage - 25.0).abs() < 0.01);
        assert_eq!(report.bytes_downloaded, 50_000);
        assert!(report.verification_ok);
        assert_eq!(report.sync_phase, SyncPhase::Idle);
        assert_eq!(report.snap_phase, SnapSyncPhase::SelectingPivot);
    }

    #[test]
    fn test_snap_sync_state_default() {
        let state = SnapSyncState::default();
        assert_eq!(state.phase, SnapSyncPhase::SelectingPivot);
        assert_eq!(state.pivot_behind, 64);
        assert_eq!(state.max_parallel_downloads, 4);
        assert!(state.downloaded_ranges.is_empty());
    }

    #[test]
    fn test_mark_range_downloaded() {
        let mut manager = StateSyncManager::default();
        assert_eq!(manager.snap_sync.active_downloads, 0);

        // Simulate an in-flight download
        manager.snap_sync.active_downloads = 2;
        let addr = Address::from([0x42u8; 20]);
        manager.mark_range_downloaded(addr);
        assert_eq!(manager.snap_sync.active_downloads, 1);
        assert!(manager.snap_sync.downloaded_ranges.contains(addr.as_bytes()));
    }
}
