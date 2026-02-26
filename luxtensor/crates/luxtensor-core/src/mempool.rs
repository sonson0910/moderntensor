//! # Unified Mempool Module
//!
//! Single source of truth for all pending transactions across the system.
//! Replaces the dual mempool architecture (eth_rpc::Mempool + node::Mempool)
//! with a unified design that supports both consensus and RPC queries.
//!
//! ## Design
//!
//! - **Transactions**: `HashMap<Hash, TimedTransaction>` — core validated pool
//! - **Pending metadata**: `HashMap<Hash, PendingTxMetadata>` — RPC-specific data
//! - **Separate locks**: Minimizes contention between block production and RPC queries

use crate::{Address, Hash, Transaction};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

// ============================================================================
// Constants
// ============================================================================

/// Default maximum pending transactions per sender (DoS protection)
const DEFAULT_MAX_PER_SENDER: usize = 16;
/// Default minimum gas price in wei (1 Gwei)
const DEFAULT_MIN_GAS_PRICE: u64 = 1_000_000_000;
/// Default maximum transaction size in bytes (128 KB)
const DEFAULT_MAX_TX_SIZE: usize = 128 * 1024;
/// Default transaction expiration duration in seconds (30 minutes)
const DEFAULT_TX_EXPIRATION_SECS: u64 = 30 * 60;

// ============================================================================
// Types
// ============================================================================

/// Transaction with timestamp for expiration tracking
struct TimedTransaction {
    tx: Transaction,
    added_at: Instant,
}

/// Lightweight metadata for RPC pending queries.
///
/// Replaces the heavyweight `PendingTransaction` struct from `eth_rpc`.
/// Stored separately from core transactions to maintain separation of concerns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTxMetadata {
    /// Whether the transaction has been executed (included in a block)
    pub executed: bool,
    /// Contract address if this was a deployment transaction
    pub contract_address: Option<Address>,
    /// Execution status (true = success)
    pub status: bool,
    /// Gas actually used during execution
    pub gas_used: u64,
}

impl Default for PendingTxMetadata {
    fn default() -> Self {
        Self {
            executed: false,
            contract_address: None,
            status: false,
            gas_used: 0,
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("Mempool is full")]
    Full,

    #[error("Duplicate transaction")]
    DuplicateTransaction,

    #[error("Invalid transaction signature")]
    InvalidSignature,

    #[error("Transaction too large: {size} bytes (max: {max})")]
    TransactionTooLarge { size: usize, max: usize },

    #[error("Gas price too low: {price} (min: {min})")]
    GasPriceTooLow { price: u64, min: u64 },

    #[error("Sender {sender:?} has reached limit of {limit} pending transactions")]
    SenderLimitReached { sender: Address, limit: usize },

    #[error("Wrong chain_id: expected {expected}, got {got}")]
    WrongChainId { expected: u64, got: u64 },
}

// ============================================================================
// UnifiedMempool
// ============================================================================

/// Single mempool for all pending transactions.
///
/// Combines the validated transaction pool (previously `node::Mempool`)
/// with RPC pending metadata (previously `eth_rpc::Mempool`).
///
/// Thread-safe with separate locks for transactions and metadata
/// to minimize contention between block production and RPC queries.
pub struct UnifiedMempool {
    // === Core transaction pool (from node::Mempool) ===
    transactions: Arc<RwLock<HashMap<Hash, TimedTransaction>>>,
    /// Per-sender transaction count for DoS protection
    sender_tx_count: Arc<RwLock<HashMap<Address, usize>>>,
    max_size: usize,
    max_per_sender: usize,
    min_gas_price: u64,
    max_tx_size: usize,
    validate_signatures: bool,
    tx_expiration: Duration,
    chain_id: u64,

    // === RPC pending metadata (replaces eth_rpc::Mempool::pending_txs) ===
    /// Separate lock to avoid contention with block production reads
    pending_metadata: Arc<RwLock<HashMap<Hash, PendingTxMetadata>>>,
}

impl UnifiedMempool {
    /// Create a production mempool with signature validation enabled
    pub fn new(max_size: usize, chain_id: u64) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            sender_tx_count: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_per_sender: DEFAULT_MAX_PER_SENDER,
            min_gas_price: DEFAULT_MIN_GAS_PRICE,
            max_tx_size: DEFAULT_MAX_TX_SIZE,
            validate_signatures: true,
            tx_expiration: Duration::from_secs(DEFAULT_TX_EXPIRATION_SECS),
            chain_id,
            pending_metadata: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create mempool with custom config
    pub fn with_config(
        max_size: usize,
        max_per_sender: usize,
        min_gas_price: u64,
        max_tx_size: usize,
        chain_id: u64,
    ) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            sender_tx_count: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_per_sender,
            min_gas_price,
            max_tx_size,
            validate_signatures: true,
            tx_expiration: Duration::from_secs(DEFAULT_TX_EXPIRATION_SECS),
            chain_id,
            pending_metadata: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create mempool for development (no signature validation, relaxed limits)
    ///
    /// # WARNING
    /// **DO NOT USE IN PRODUCTION.** Signature validation is disabled and all
    /// transactions are accepted without authentication.
    ///
    /// SECURITY (L-8): Runtime log emitted at WARN level to prevent silent misuse.
    pub fn new_dev_mode(max_size: usize, chain_id: u64) -> Self {
        warn!(
            "⚠️ DEV MODE MEMPOOL initialized — signature validation is DISABLED. \
             Do NOT use in production!"
        );
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            sender_tx_count: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_per_sender: 1000,             // Relaxed for dev
            min_gas_price: 0,                 // No minimum for dev
            max_tx_size: 1024 * 1024,         // 1MB for dev
            validate_signatures: false,
            tx_expiration: Duration::from_secs(30 * 60),
            chain_id,
            pending_metadata: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ========================================================================
    // Transaction Management (from node::Mempool)
    // ========================================================================

    /// Add a transaction to the mempool with full DoS protection.
    ///
    /// Used by P2P handler and internal paths where no RPC metadata is needed.
    pub fn add_transaction(&self, tx: Transaction) -> Result<(), MempoolError> {
        self.validate_and_insert(tx)
    }

    /// Add a **system/inherent** transaction to the mempool — signature validation skipped.
    ///
    /// Used for MetagraphTx submitted by the RPC server itself (not the client).
    /// The signature has already been verified at the RPC layer (timestamp + ECDSA).
    /// chain_id, size, and sender-limit checks still apply.
    ///
    /// Analogous to Substrate "inherent extrinsics" (e.g. set_timestamp, register_validator).
    pub fn add_system_transaction(&self, tx: Transaction) -> Result<(), MempoolError> {
        self.validate_and_insert_system(tx)
    }

    /// Get the chain_id this mempool was configured with.
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Add a transaction with associated RPC pending metadata.
    ///
    /// Used by RPC handlers (`eth_sendRawTransaction`, `eth_sendTransaction`)
    /// to track pending state for queries like `eth_getTransactionByHash`.
    pub fn add_transaction_with_metadata(
        &self,
        tx: Transaction,
        metadata: PendingTxMetadata,
    ) -> Result<(), MempoolError> {
        let hash = tx.hash();
        self.validate_and_insert(tx)?;

        // Store metadata under separate lock (no contention with tx lock)
        let mut meta = self.pending_metadata.write();
        meta.insert(hash, metadata);
        Ok(())
    }

    /// Core validation and insertion logic — shared by both add paths.
    fn validate_and_insert(&self, tx: Transaction) -> Result<(), MempoolError> {
        // SECURITY: Validate chain_id — reject cross-chain transactions early
        if tx.chain_id != self.chain_id {
            warn!(
                "🛡️ Rejected transaction: chain_id {} != expected {}",
                tx.chain_id, self.chain_id
            );
            return Err(MempoolError::WrongChainId {
                expected: self.chain_id,
                got: tx.chain_id,
            });
        }

        // DoS Protection 1: Check transaction size
        // SECURITY (H-1): Reject transaction if size cannot be determined instead
        // of using u64::MAX which bypasses the size check on some architectures.
        let tx_size = bincode::serialized_size(&tx).map_err(|e| {
            warn!("Failed to compute transaction size: {}", e);
            MempoolError::TransactionTooLarge { size: usize::MAX, max: self.max_tx_size }
        })? as usize;
        if tx_size > self.max_tx_size {
            warn!(
                "🛡️ Rejected transaction: size {} > max {}",
                tx_size, self.max_tx_size
            );
            return Err(MempoolError::TransactionTooLarge {
                size: tx_size,
                max: self.max_tx_size,
            });
        }

        // DoS Protection 2: Check minimum gas price
        // SECURITY (H-2): No bypass for faucet mint or any special address.
        // All transactions must pass gas price and signature checks.
        if tx.gas_price < self.min_gas_price {
            debug!(
                "🛡️ Rejected transaction: gas_price {} < min {}",
                tx.gas_price, self.min_gas_price
            );
            return Err(MempoolError::GasPriceTooLow {
                price: tx.gas_price,
                min: self.min_gas_price,
            });
        }

        // SECURITY (H-2): Validate signature for ALL transactions, no exceptions.
        if self.validate_signatures {
            if let Err(e) = tx.verify_signature() {
                warn!("Rejected transaction with invalid signature: {:?}", e);
                return Err(MempoolError::InvalidSignature);
            }
        }

        let sender = tx.from;

        // DoS Protection 3: Check per-sender transaction limit
        {
            let sender_counts = self.sender_tx_count.read();
            if let Some(&count) = sender_counts.get(&sender) {
                if count >= self.max_per_sender {
                    warn!(
                        "🛡️ Rejected transaction from {:?}: sender limit {} reached",
                        sender, self.max_per_sender
                    );
                    return Err(MempoolError::SenderLimitReached {
                        sender,
                        limit: self.max_per_sender,
                    });
                }
            }
        }

        // Cleanup expired transactions first
        self.cleanup_expired();

        let mut txs = self.transactions.write();

        // Check if mempool is full
        if txs.len() >= self.max_size {
            return Err(MempoolError::Full);
        }

        let hash = tx.hash();

        // Check if transaction already exists
        if txs.contains_key(&hash) {
            return Err(MempoolError::DuplicateTransaction);
        }

        // Update sender count
        {
            let mut sender_counts = self.sender_tx_count.write();
            *sender_counts.entry(sender).or_insert(0) += 1;
        }

        txs.insert(hash, TimedTransaction {
            tx,
            added_at: Instant::now(),
        });
        Ok(())
    }

    /// System transaction insertion — skips signature validation.
    /// All other checks (chain_id, size, sender limit, mempool full, dedup) still apply.
    fn validate_and_insert_system(&self, tx: Transaction) -> Result<(), MempoolError> {
        // chain_id must still match
        if tx.chain_id != self.chain_id {
            warn!("🛡️ Rejected system tx: chain_id {} != expected {}", tx.chain_id, self.chain_id);
            return Err(MempoolError::WrongChainId { expected: self.chain_id, got: tx.chain_id });
        }

        let tx_size = bincode::serialized_size(&tx).map_err(|e| {
            warn!("Failed to compute system tx size: {}", e);
            MempoolError::TransactionTooLarge { size: usize::MAX, max: self.max_tx_size }
        })? as usize;
        if tx_size > self.max_tx_size {
            return Err(MempoolError::TransactionTooLarge { size: tx_size, max: self.max_tx_size });
        }

        // No gas_price check for system txs (gas_price=1 is acceptable)
        // No signature check — server-originated tx

        let sender = tx.from;
        {
            let sender_counts = self.sender_tx_count.read();
            if let Some(&count) = sender_counts.get(&sender) {
                if count >= self.max_per_sender {
                    return Err(MempoolError::SenderLimitReached { sender, limit: self.max_per_sender });
                }
            }
        }

        self.cleanup_expired();
        let mut txs = self.transactions.write();
        if txs.len() >= self.max_size {
            return Err(MempoolError::Full);
        }
        let hash = tx.hash();
        if txs.contains_key(&hash) {
            // Idempotent: same payload = same hash = already queued, not an error
            return Ok(());
        }
        {
            let mut sender_counts = self.sender_tx_count.write();
            *sender_counts.entry(sender).or_insert(0) += 1;
        }
        txs.insert(hash, TimedTransaction { tx, added_at: Instant::now() });
        Ok(())
    }

    /// Remove expired transactions from the mempool
    pub fn cleanup_expired(&self) -> usize {
        let mut txs = self.transactions.write();
        let now = Instant::now();

        // Collect expired hashes for metadata cleanup
        let expired_hashes: Vec<Hash> = txs
            .iter()
            .filter(|(_, timed_tx)| now.duration_since(timed_tx.added_at) >= self.tx_expiration)
            .map(|(hash, _)| *hash)
            .collect();

        for hash in &expired_hashes {
            txs.remove(hash);
        }
        drop(txs);

        // Also cleanup corresponding metadata
        if !expired_hashes.is_empty() {
            let mut meta = self.pending_metadata.write();
            for hash in &expired_hashes {
                meta.remove(hash);
            }
        }

        let removed = expired_hashes.len();
        if removed > 0 {
            info!("🧹 Cleaned up {} expired transactions from mempool", removed);
        }
        removed
    }

    // ========================================================================
    // Query Methods
    // ========================================================================

    /// Get all pending transactions
    pub fn get_pending_transactions(&self) -> Vec<Transaction> {
        let txs = self.transactions.read();
        txs.values().map(|t| t.tx.clone()).collect()
    }

    /// Get transactions for block production (up to limit).
    ///
    /// Returns transactions sorted by (gas_price desc, sender asc, nonce asc).
    ///
    /// SECURITY (L-4): Uses fully deterministic ordering by including tx hash as
    /// a final tie-breaker — ensures every node produces the same TX ordering
    /// for identical gas_price/sender/nonce, which is required for consensus.
    pub fn get_transactions_for_block(&self, limit: usize) -> Vec<Transaction> {
        let txs = self.transactions.read();
        debug!(
            "get_transactions_for_block: mempool has {} transactions, limit={}",
            txs.len(),
            limit
        );
        let mut sorted: Vec<Transaction> = txs.values().map(|t| t.tx.clone()).collect();

        // Stable, fully-deterministic sort:
        //   1. gas_price DESC  (higher priority first)
        //   2. sender ASC      (group TXs from same sender)
        //   3. nonce ASC       (correct execution order within sender)
        //   4. tx hash ASC     (canonical tie-breaker — same on every node)
        sorted.sort_by(|a, b| {
            b.gas_price
                .cmp(&a.gas_price)
                .then_with(|| a.from.cmp(&b.from))
                .then_with(|| a.nonce.cmp(&b.nonce))
                .then_with(|| a.hash().cmp(&b.hash())) // L-4: deterministic tie-break
        });

        sorted.truncate(limit);
        debug!(
            "get_transactions_for_block: returning {} transactions",
            sorted.len()
        );
        sorted
    }

    /// Remove transactions from mempool (also updates sender counts and metadata)
    pub fn remove_transactions(&self, tx_hashes: &[Hash]) {
        let mut txs = self.transactions.write();
        let mut sender_counts = self.sender_tx_count.write();

        for hash in tx_hashes {
            if let Some(timed_tx) = txs.remove(hash) {
                let sender = timed_tx.tx.from;
                if let Some(count) = sender_counts.get_mut(&sender) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        sender_counts.remove(&sender);
                    }
                }
            }
        }
        drop(txs);
        drop(sender_counts);

        // Cleanup metadata under separate lock
        let mut meta = self.pending_metadata.write();
        for hash in tx_hashes {
            meta.remove(hash);
        }
    }

    /// Get number of transactions in mempool
    pub fn len(&self) -> usize {
        self.transactions.read().len()
    }

    /// Check if mempool is empty
    pub fn is_empty(&self) -> bool {
        self.transactions.read().is_empty()
    }

    /// Get a specific transaction by hash
    pub fn get_transaction(&self, hash: &Hash) -> Option<Transaction> {
        self.transactions.read().get(hash).map(|t| t.tx.clone())
    }

    /// Check if a transaction exists in the mempool
    pub fn contains(&self, hash: &Hash) -> bool {
        self.transactions.read().contains_key(hash)
    }

    // ========================================================================
    // RPC Pending Metadata (replaces eth_rpc::Mempool::pending_txs)
    // ========================================================================

    /// Get pending metadata for a transaction
    pub fn get_pending_metadata(&self, hash: &Hash) -> Option<PendingTxMetadata> {
        self.pending_metadata.read().get(hash).cloned()
    }

    /// Update pending metadata (e.g., after execution)
    pub fn update_pending_metadata(&self, hash: &Hash, metadata: PendingTxMetadata) {
        let mut meta = self.pending_metadata.write();
        meta.insert(*hash, metadata);
    }

    /// Check if a transaction hash has pending metadata
    pub fn has_pending_metadata(&self, hash: &Hash) -> bool {
        self.pending_metadata.read().contains_key(hash)
    }

    /// Get number of pending metadata entries
    pub fn pending_metadata_count(&self) -> usize {
        self.pending_metadata.read().len()
    }

    // ========================================================================
    // Persistence (from node::Mempool)
    // ========================================================================

    /// Save mempool to file for graceful shutdown.
    ///
    /// SECURITY (L-2): Uses atomic write — data is written to a temp file first,
    /// then renamed to the target path. This prevents a corrupt/truncated backup
    /// file if the node crashes mid-write.
    ///
    /// Returns number of transactions saved.
    pub fn save_to_file(&self, path: &str) -> std::io::Result<usize> {
        let txs = self.transactions.read();
        let transactions: Vec<Transaction> = txs.values().map(|t| t.tx.clone()).collect();
        let count = transactions.len();

        if count == 0 {
            info!("💾 Mempool empty, nothing to save");
            return Ok(0);
        }

        let data = bincode::serialize(&transactions)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // SECURITY (L-2): Atomic write via temp file + rename.
        // Prevents a partially-written backup file if the process is killed
        // during the write, which would corrupt the mempool on next startup.
        let tmp_path = format!("{}.tmp", path);
        std::fs::write(&tmp_path, &data)?;
        std::fs::rename(&tmp_path, path)?;

        info!("💾 Saved {} transactions to {}", count, path);
        Ok(count)
    }

    /// Load mempool from file after restart.
    /// Returns number of transactions loaded.
    pub fn load_from_file(&self, path: &str) -> std::io::Result<usize> {
        // SECURITY (M-2): Check file size before deserializing to prevent DoS via
        // a crafted oversized backup file that could exhaust memory on startup.
        const MAX_BACKUP_SIZE: u64 = 64 * 1024 * 1024; // 64 MiB
        let data = match std::fs::read(path) {
            Ok(d) => {
                if d.len() as u64 > MAX_BACKUP_SIZE {
                    warn!(
                        "💾 Mempool backup at {} is too large ({} bytes > {} limit), ignoring",
                        path, d.len(), MAX_BACKUP_SIZE
                    );
                    // Remove the potentially malicious backup file
                    let _ = std::fs::remove_file(path);
                    return Ok(0);
                }
                d
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!("💾 No mempool backup found at {}", path);
                return Ok(0);
            }
            Err(e) => return Err(e),
        };

        let transactions: Vec<Transaction> = bincode::deserialize(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        let count = transactions.len();
        let mut txs = self.transactions.write();

        for tx in transactions {
            let hash = tx.hash();
            if !txs.contains_key(&hash) {
                txs.insert(hash, TimedTransaction {
                    tx,
                    added_at: Instant::now(),
                });
            }
        }

        // Remove the backup file after successful load
        let _ = std::fs::remove_file(path);

        info!("💾 Loaded {} transactions from {}", count, path);
        Ok(count)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test chain_id — matches Transaction::new() default (devnet)
    const TEST_CHAIN_ID: u64 = 8898;

    fn create_test_transaction(nonce: u64) -> Transaction {
        Transaction::new(
            nonce,
            Address::zero(),
            Some(Address::zero()),
            1000,
            1,
            21000,
            vec![],
        )
    }

    #[test]
    fn test_mempool_creation() {
        let mempool = UnifiedMempool::new_dev_mode(100, TEST_CHAIN_ID);
        assert_eq!(mempool.len(), 0);
        assert!(mempool.is_empty());
    }

    #[test]
    fn test_add_transaction() {
        let mempool = UnifiedMempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(0);
        assert!(mempool.add_transaction(tx).is_ok());
        assert_eq!(mempool.len(), 1);
    }

    #[test]
    fn test_add_transaction_with_metadata() {
        let mempool = UnifiedMempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(0);
        let hash = tx.hash();

        let meta = PendingTxMetadata::default();
        assert!(mempool.add_transaction_with_metadata(tx, meta).is_ok());
        assert_eq!(mempool.len(), 1);

        // Verify metadata was stored
        let stored_meta = mempool.get_pending_metadata(&hash);
        assert!(stored_meta.is_some());
        assert!(!stored_meta.unwrap().executed);
    }

    #[test]
    fn test_get_pending_transactions() {
        let mempool = UnifiedMempool::new_dev_mode(100, TEST_CHAIN_ID);
        mempool.add_transaction(create_test_transaction(0)).unwrap();
        mempool.add_transaction(create_test_transaction(1)).unwrap();

        let pending = mempool.get_pending_transactions();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_remove_transactions_cleans_metadata() {
        let mempool = UnifiedMempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(0);
        let hash = tx.hash();

        let meta = PendingTxMetadata::default();
        mempool.add_transaction_with_metadata(tx, meta).unwrap();
        assert_eq!(mempool.len(), 1);
        assert!(mempool.has_pending_metadata(&hash));

        mempool.remove_transactions(&[hash]);
        assert_eq!(mempool.len(), 0);
        assert!(!mempool.has_pending_metadata(&hash));
    }

    #[test]
    fn test_mempool_full() {
        let mempool = UnifiedMempool::new_dev_mode(2, TEST_CHAIN_ID);
        mempool.add_transaction(create_test_transaction(0)).unwrap();
        mempool.add_transaction(create_test_transaction(1)).unwrap();

        let result = mempool.add_transaction(create_test_transaction(2));
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_transaction() {
        let mempool = UnifiedMempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(0);

        mempool.add_transaction(tx.clone()).unwrap();
        let result = mempool.add_transaction(tx);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_chain_id() {
        let mempool = UnifiedMempool::new_dev_mode(100, 9999);
        let tx = create_test_transaction(0); // chain_id = 8898

        let result = mempool.add_transaction(tx);
        assert!(matches!(result, Err(MempoolError::WrongChainId { .. })));
    }

    #[test]
    fn test_get_transaction_by_hash() {
        let mempool = UnifiedMempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(42);
        let hash = tx.hash();

        mempool.add_transaction(tx).unwrap();
        let found = mempool.get_transaction(&hash);
        assert!(found.is_some());
        assert_eq!(found.unwrap().nonce, 42);
    }

    #[test]
    fn test_contains() {
        let mempool = UnifiedMempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(0);
        let hash = tx.hash();

        assert!(!mempool.contains(&hash));
        mempool.add_transaction(tx).unwrap();
        assert!(mempool.contains(&hash));
    }

    #[test]
    fn test_update_pending_metadata() {
        let mempool = UnifiedMempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(0);
        let hash = tx.hash();

        let meta = PendingTxMetadata::default();
        mempool.add_transaction_with_metadata(tx, meta).unwrap();

        // Simulate execution completion
        mempool.update_pending_metadata(&hash, PendingTxMetadata {
            executed: true,
            contract_address: None,
            status: true,
            gas_used: 21000,
        });

        let updated = mempool.get_pending_metadata(&hash).unwrap();
        assert!(updated.executed);
        assert!(updated.status);
        assert_eq!(updated.gas_used, 21000);
    }
}
