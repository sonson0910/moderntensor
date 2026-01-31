use luxtensor_core::{Transaction, Hash, Address};
use std::collections::HashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{warn, info, debug};

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
}

impl Mempool {
    /// Create a new mempool with maximum size and signature validation enabled
    pub fn new(max_size: usize) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            sender_tx_count: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_per_sender: 16,                    // DoS: Max 16 pending tx per sender
            min_gas_price: 1_000_000_000,          // DoS: Min 1 gwei gas price
            max_tx_size: 128 * 1024,               // DoS: Max 128KB per transaction
            validate_signatures: true,              // PRODUCTION: always validate
            tx_expiration: Duration::from_secs(30 * 60), // 30 minutes
        }
    }

    /// Create mempool with custom config
    pub fn with_config(
        max_size: usize,
        max_per_sender: usize,
        min_gas_price: u64,
        max_tx_size: usize,
    ) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            sender_tx_count: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_per_sender,
            min_gas_price,
            max_tx_size,
            validate_signatures: true,
            tx_expiration: Duration::from_secs(30 * 60),
        }
    }

    /// Create mempool for development (no signature validation, relaxed limits)
    /// WARNING: Only use for local development/testing!
    pub fn new_dev_mode(max_size: usize) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            sender_tx_count: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_per_sender: 1000,                  // Relaxed for dev
            min_gas_price: 0,                      // No minimum for dev
            max_tx_size: 1024 * 1024,              // 1MB for dev
            validate_signatures: false,
            tx_expiration: Duration::from_secs(30 * 60),
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
        // DoS Protection 1: Check transaction size
        let tx_size = bincode::serialized_size(&tx).unwrap_or(u64::MAX) as usize;
        if tx_size > self.max_tx_size {
            warn!("🛡️ Rejected transaction: size {} > max {}", tx_size, self.max_tx_size);
            return Err(MempoolError::TransactionTooLarge { size: tx_size, max: self.max_tx_size });
        }

        // DoS Protection 2: Check minimum gas price
        if tx.gas_price < self.min_gas_price {
            debug!("🛡️ Rejected transaction: gas_price {} < min {}", tx.gas_price, self.min_gas_price);
            return Err(MempoolError::GasPriceTooLow { price: tx.gas_price, min: self.min_gas_price });
        }

        // SECURITY: Validate signature before accepting into mempool
        if self.validate_signatures {
            if let Err(e) = tx.verify_signature() {
                warn!("Rejected transaction with invalid signature: {:?}", e);
                return Err(MempoolError::InvalidSignature);
            }
        }

        // Get sender for per-sender limit check
        let sender = tx.from;

        // DoS Protection 3: Check per-sender transaction limit
        {
            let sender_counts = self.sender_tx_count.read();
            if let Some(&count) = sender_counts.get(&sender) {
                if count >= self.max_per_sender {
                    warn!("🛡️ Rejected transaction from {:?}: sender limit {} reached", sender, self.max_per_sender);
                    return Err(MempoolError::SenderLimitReached { sender, limit: self.max_per_sender });
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

        // Wrap with timestamp and sender
        let timed_tx = TimedTransaction {
            tx,
            added_at: Instant::now(),
        };
        txs.insert(hash, timed_tx);
        Ok(())
    }

    /// Get all pending transactions
    pub fn get_pending_transactions(&self) -> Vec<Transaction> {
        let txs = self.transactions.read();
        txs.values().map(|t| t.tx.clone()).collect()
    }

    /// Get transactions for block production (up to limit)
    pub fn get_transactions_for_block(&self, limit: usize) -> Vec<Transaction> {
        let txs = self.transactions.read();
        txs.values().take(limit).map(|t| t.tx.clone()).collect()
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
    pub fn is_empty(&self) -> bool {
        self.transactions.read().is_empty()
    }

    /// Clear all transactions
    pub fn clear(&self) {
        self.transactions.write().clear();
    }

    /// Get a specific transaction by hash
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

        let data = bincode::serialize(&transactions)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

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

        let transactions: Vec<Transaction> = bincode::deserialize(&data)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::Address;

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
        let mempool = Mempool::new_dev_mode(100);
        assert_eq!(mempool.len(), 0);
        assert!(mempool.is_empty());
    }

    #[test]
    fn test_add_transaction() {
        // Use dev mode for unsigned test transactions
        let mempool = Mempool::new_dev_mode(100);
        let tx = create_test_transaction(0);

        assert!(mempool.add_transaction(tx).is_ok());
        assert_eq!(mempool.len(), 1);
    }

    #[test]
    fn test_get_pending_transactions() {
        let mempool = Mempool::new_dev_mode(100);

        mempool.add_transaction(create_test_transaction(0)).unwrap();
        mempool.add_transaction(create_test_transaction(1)).unwrap();

        let pending = mempool.get_pending_transactions();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_remove_transactions() {
        let mempool = Mempool::new_dev_mode(100);
        let tx = create_test_transaction(0);
        let hash = tx.hash();

        mempool.add_transaction(tx).unwrap();
        assert_eq!(mempool.len(), 1);

        mempool.remove_transactions(&[hash]);
        assert_eq!(mempool.len(), 0);
    }

    #[test]
    fn test_mempool_full() {
        let mempool = Mempool::new_dev_mode(2);

        mempool.add_transaction(create_test_transaction(0)).unwrap();
        mempool.add_transaction(create_test_transaction(1)).unwrap();

        let result = mempool.add_transaction(create_test_transaction(2));
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_transaction() {
        let mempool = Mempool::new_dev_mode(100);
        let tx = create_test_transaction(0);

        mempool.add_transaction(tx.clone()).unwrap();
        let result = mempool.add_transaction(tx);
        assert!(result.is_err());
    }
}
