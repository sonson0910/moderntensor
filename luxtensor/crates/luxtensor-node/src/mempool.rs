use luxtensor_core::{Transaction, Hash, Address};
use std::collections::HashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{warn, info, debug};

/// Default maximum pending transactions per sender (DoS protection)
const DEFAULT_MAX_PER_SENDER: usize = 16;
/// Default minimum gas price in wei (1 Gwei)
const DEFAULT_MIN_GAS_PRICE: u64 = 1_000_000_000;
/// Default maximum transaction size in bytes (128 KB)
const DEFAULT_MAX_TX_SIZE: usize = 128 * 1024;
/// Default transaction expiration duration in seconds (30 minutes)
const DEFAULT_TX_EXPIRATION_SECS: u64 = 30 * 60;

/// Transaction with timestamp for expiration tracking
struct TimedTransaction {
    tx: Transaction,
    added_at: Instant,
}

/// Transaction mempool with signature validation, expiration, and DoS protection
pub struct Mempool {
    transactions: Arc<RwLock<HashMap<Hash, TimedTransaction>>>,
    /// Transactions per sender for DoS protection
    sender_tx_count: Arc<RwLock<HashMap<Address, usize>>>,
    max_size: usize,
    /// Maximum transactions per sender (DoS protection)
    max_per_sender: usize,
    /// Minimum gas price to accept (DoS protection)
    min_gas_price: u64,
    /// Maximum transaction size in bytes (DoS protection)
    max_tx_size: usize,
    /// Whether to validate signatures before adding (should be true in production)
    validate_signatures: bool,
    /// Transaction expiration time (default: 30 minutes)
    tx_expiration: Duration,
    /// Expected chain_id — transactions with a different chain_id are rejected
    chain_id: u64,
    /// L2 FIX: Last cleanup timestamp to avoid O(n) scan on every insert
    last_cleanup: Arc<RwLock<Instant>>,
}

impl Mempool {
    /// Create a new mempool with maximum size and signature validation enabled
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
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Create mempool with custom config
    #[allow(dead_code)]
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
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Create mempool for development (no signature validation, relaxed limits)
    /// WARNING: Only use for local development/testing!
    #[allow(dead_code)]
    pub fn new_dev_mode(max_size: usize, chain_id: u64) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            sender_tx_count: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_per_sender: 1000,                  // Relaxed for dev
            min_gas_price: 0,                      // No minimum for dev
            max_tx_size: 1024 * 1024,              // 1MB for dev
            validate_signatures: false,
            tx_expiration: Duration::from_secs(30 * 60),
            chain_id,
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Remove expired transactions from the mempool
    pub fn cleanup_expired(&self) -> usize {
        let mut txs = self.transactions.write();
        let now = Instant::now();
        let before = txs.len();

        txs.retain(|_, timed_tx| {
            now.duration_since(timed_tx.added_at) < self.tx_expiration
        });

        let removed = before - txs.len();
        if removed > 0 {
            info!("🧹 Cleaned up {} expired transactions from mempool", removed);
        }
        removed
    }

    /// Add a transaction to the mempool with DoS protection
    pub fn add_transaction(&self, tx: Transaction) -> Result<(), MempoolError> {
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
        let tx_size = bincode::serialized_size(&tx).unwrap_or(u64::MAX) as usize;
        if tx_size > self.max_tx_size {
            warn!("🛡️ Rejected transaction: size {} > max {}", tx_size, self.max_tx_size);
            return Err(MempoolError::TransactionTooLarge { size: tx_size, max: self.max_tx_size });
        }

        // 🔧 FIX: Faucet mint TXs (from Address::zero()) skip gas price
        // and signature checks — they are special "mint" transactions.
        let is_faucet_mint = tx.from == Address::zero();

        // DoS Protection 2: Check minimum gas price (skip for faucet mints)
        if !is_faucet_mint && tx.gas_price < self.min_gas_price {
            debug!("🛡️ Rejected transaction: gas_price {} < min {}", tx.gas_price, self.min_gas_price);
            return Err(MempoolError::GasPriceTooLow { price: tx.gas_price, min: self.min_gas_price });
        }

        // SECURITY: Validate signature before accepting into mempool
        // (skip for faucet mints — they don't have real signatures)
        if !is_faucet_mint && self.validate_signatures {
            if let Err(e) = tx.verify_signature() {
                warn!("Rejected transaction with invalid signature: {:?}", e);
                return Err(MempoolError::InvalidSignature);
            }
        }

        // Get sender for per-sender limit check
        let sender = tx.from;

        // H3 FIX: Per-sender limit check moved below to atomic write-lock scope
        // to eliminate TOCTOU race between read-check and write-increment.

        // L2 FIX: Only cleanup expired TXs periodically (every 60s) instead of
        // on every insert. With 10K TXs, this avoids O(n) scan per insertion.
        {
            let last = *self.last_cleanup.read();
            if last.elapsed() > Duration::from_secs(60) {
                self.cleanup_expired();
                *self.last_cleanup.write() = Instant::now();
            }
        }

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

        // H3 FIX: Atomic check-and-increment to prevent TOCTOU race.
        // Previously, a read lock checked the count and released, then a write lock
        // incremented — two threads could both pass the check simultaneously.
        {
            let mut sender_counts = self.sender_tx_count.write();
            let count = sender_counts.entry(sender).or_insert(0);
            if *count >= self.max_per_sender {
                warn!("🛡️ Rejected transaction from {:?}: sender limit {} reached", sender, self.max_per_sender);
                return Err(MempoolError::SenderLimitReached { sender, limit: self.max_per_sender });
            }
            *count += 1;
        }

        // Wrap with timestamp and sender
        let timed_tx = TimedTransaction {
            tx,
            added_at: Instant::now(),
        };
        txs.insert(hash, timed_tx);
        Ok(())
    }



    /// Get all pending transactions
    #[allow(dead_code)]
    pub fn get_pending_transactions(&self) -> Vec<Transaction> {
        let txs = self.transactions.read();
        txs.values().map(|t| t.tx.clone()).collect()
    }

    /// Get transactions for block production (up to limit)
    ///
    /// Returns transactions sorted by (sender, nonce) for correct execution order.
    /// Within a block, transactions from the same sender MUST be ordered by nonce
    /// to prevent execution failures (nonce gaps cause all subsequent txs to fail).
    /// Transactions are also prioritized by gas_price (higher gas = earlier inclusion).
    pub fn get_transactions_for_block(&self, limit: usize) -> Vec<Transaction> {
        let txs = self.transactions.read();
        debug!("get_transactions_for_block: mempool has {} transactions, limit={}", txs.len(), limit);
        let mut sorted: Vec<Transaction> = txs.values().map(|t| t.tx.clone()).collect();

        // Primary sort: gas_price descending (priority), secondary: sender + nonce ascending
        sorted.sort_by(|a, b| {
            // First: higher gas_price transactions get priority
            b.gas_price.cmp(&a.gas_price)
                // Then: group by sender and order by nonce within sender
                .then_with(|| a.from.cmp(&b.from))
                .then_with(|| a.nonce.cmp(&b.nonce))
        });

        sorted.truncate(limit);
        debug!("get_transactions_for_block: returning {} transactions", sorted.len());
        sorted
    }

    /// Remove transactions from mempool (also updates sender counts)
    pub fn remove_transactions(&self, tx_hashes: &[Hash]) {
        let mut txs = self.transactions.write();
        let mut sender_counts = self.sender_tx_count.write();

        for hash in tx_hashes {
            if let Some(timed_tx) = txs.remove(hash) {
                // Decrement sender count
                let sender = timed_tx.tx.from;
                if let Some(count) = sender_counts.get_mut(&sender) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        sender_counts.remove(&sender);
                    }
                }
            }
        }
    }

    /// Get number of transactions in mempool
    pub fn len(&self) -> usize {
        self.transactions.read().len()
    }

    /// Check if mempool is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.transactions.read().is_empty()
    }

    /// Get a specific transaction by hash
    #[allow(dead_code)]
    pub fn get_transaction(&self, hash: &Hash) -> Option<Transaction> {
        self.transactions.read().get(hash).map(|t| t.tx.clone())
    }

    /// Save mempool to file for graceful shutdown
    /// Returns number of transactions saved
    pub fn save_to_file(&self, path: &str) -> std::io::Result<usize> {
        let txs = self.transactions.read();
        let transactions: Vec<Transaction> = txs.values().map(|t| t.tx.clone()).collect();
        let count = transactions.len();

        if count == 0 {
            info!("💾 Mempool empty, nothing to save");
            return Ok(0);
        }

        let payload = bincode::serialize(&transactions)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // L5 FIX: Prepend a 4-byte version header to detect format mismatches on load.
        const MEMPOOL_FILE_VERSION: u32 = 1;
        let mut data = Vec::with_capacity(4 + payload.len());
        data.extend_from_slice(&MEMPOOL_FILE_VERSION.to_le_bytes());
        data.extend_from_slice(&payload);

        std::fs::write(path, data)?;
        info!("💾 Saved {} transactions to {}", count, path);
        Ok(count)
    }

    /// Load mempool from file after restart
    /// Returns number of transactions loaded
    pub fn load_from_file(&self, path: &str) -> std::io::Result<usize> {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!("💾 No mempool backup found at {}", path);
                return Ok(0);
            }
            Err(e) => return Err(e),
        };

        // L5 FIX: Check version header to detect format mismatches.
        // Files without header (pre-v1) are rejected gracefully.
        const MEMPOOL_FILE_VERSION: u32 = 1;
        if data.len() < 4 {
            warn!("💾 Mempool backup too small ({} bytes), skipping", data.len());
            let _ = std::fs::remove_file(path);
            return Ok(0);
        }
        let file_version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if file_version != MEMPOOL_FILE_VERSION {
            warn!(
                "💾 Mempool backup version mismatch (file={}, expected={}), discarding",
                file_version, MEMPOOL_FILE_VERSION
            );
            let _ = std::fs::remove_file(path);
            return Ok(0);
        }

        let transactions: Vec<Transaction> = bincode::deserialize(&data[4..])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        let count = transactions.len();
        let mut txs = self.transactions.write();

        for tx in transactions {
            let hash = tx.hash();
            if !txs.contains_key(&hash) {
                txs.insert(hash, TimedTransaction {
                    tx,
                    added_at: std::time::Instant::now(),
                });
            }
        }

        // Remove the backup file after successful load
        let _ = std::fs::remove_file(path);

        info!("💾 Loaded {} transactions from {}", count, path);
        Ok(count)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::Address;

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
        let mempool = Mempool::new_dev_mode(100, TEST_CHAIN_ID);
        assert_eq!(mempool.len(), 0);
        assert!(mempool.is_empty());
    }

    #[test]
    fn test_add_transaction() {
        // Use dev mode for unsigned test transactions
        let mempool = Mempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(0);

        assert!(mempool.add_transaction(tx).is_ok());
        assert_eq!(mempool.len(), 1);
    }

    #[test]
    fn test_get_pending_transactions() {
        let mempool = Mempool::new_dev_mode(100, TEST_CHAIN_ID);

        mempool.add_transaction(create_test_transaction(0)).unwrap();
        mempool.add_transaction(create_test_transaction(1)).unwrap();

        let pending = mempool.get_pending_transactions();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_remove_transactions() {
        let mempool = Mempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(0);
        let hash = tx.hash();

        mempool.add_transaction(tx).unwrap();
        assert_eq!(mempool.len(), 1);

        mempool.remove_transactions(&[hash]);
        assert_eq!(mempool.len(), 0);
    }

    #[test]
    fn test_mempool_full() {
        let mempool = Mempool::new_dev_mode(2, TEST_CHAIN_ID);

        mempool.add_transaction(create_test_transaction(0)).unwrap();
        mempool.add_transaction(create_test_transaction(1)).unwrap();

        let result = mempool.add_transaction(create_test_transaction(2));
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_transaction() {
        let mempool = Mempool::new_dev_mode(100, TEST_CHAIN_ID);
        let tx = create_test_transaction(0);

        mempool.add_transaction(tx.clone()).unwrap();
        let result = mempool.add_transaction(tx);
        assert!(result.is_err());
    }
}
