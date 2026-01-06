use luxtensor_core::{Transaction, Hash};
use std::collections::HashMap;
use parking_lot::RwLock;
use std::sync::Arc;

/// Transaction mempool
pub struct Mempool {
    transactions: Arc<RwLock<HashMap<Hash, Transaction>>>,
    max_size: usize,
}

impl Mempool {
    /// Create a new mempool with maximum size
    pub fn new(max_size: usize) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            max_size,
        }
    }

    /// Add a transaction to the mempool
    pub fn add_transaction(&self, tx: Transaction) -> Result<(), MempoolError> {
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
        
        txs.insert(hash, tx);
        Ok(())
    }

    /// Get all pending transactions
    pub fn get_pending_transactions(&self) -> Vec<Transaction> {
        let txs = self.transactions.read();
        txs.values().cloned().collect()
    }

    /// Get transactions for block production (up to limit)
    pub fn get_transactions_for_block(&self, limit: usize) -> Vec<Transaction> {
        let txs = self.transactions.read();
        txs.values().take(limit).cloned().collect()
    }

    /// Remove transactions from mempool
    pub fn remove_transactions(&self, tx_hashes: &[Hash]) {
        let mut txs = self.transactions.write();
        for hash in tx_hashes {
            txs.remove(hash);
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
        self.transactions.read().get(hash).cloned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("Mempool is full")]
    Full,
    
    #[error("Duplicate transaction")]
    DuplicateTransaction,
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
        let mempool = Mempool::new(100);
        assert_eq!(mempool.len(), 0);
        assert!(mempool.is_empty());
    }

    #[test]
    fn test_add_transaction() {
        let mempool = Mempool::new(100);
        let tx = create_test_transaction(0);
        
        assert!(mempool.add_transaction(tx).is_ok());
        assert_eq!(mempool.len(), 1);
    }

    #[test]
    fn test_get_pending_transactions() {
        let mempool = Mempool::new(100);
        
        mempool.add_transaction(create_test_transaction(0)).unwrap();
        mempool.add_transaction(create_test_transaction(1)).unwrap();
        
        let pending = mempool.get_pending_transactions();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_remove_transactions() {
        let mempool = Mempool::new(100);
        let tx = create_test_transaction(0);
        let hash = tx.hash();
        
        mempool.add_transaction(tx).unwrap();
        assert_eq!(mempool.len(), 1);
        
        mempool.remove_transactions(&[hash]);
        assert_eq!(mempool.len(), 0);
    }

    #[test]
    fn test_mempool_full() {
        let mempool = Mempool::new(2);
        
        mempool.add_transaction(create_test_transaction(0)).unwrap();
        mempool.add_transaction(create_test_transaction(1)).unwrap();
        
        let result = mempool.add_transaction(create_test_transaction(2));
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_transaction() {
        let mempool = Mempool::new(100);
        let tx = create_test_transaction(0);
        
        mempool.add_transaction(tx.clone()).unwrap();
        let result = mempool.add_transaction(tx);
        assert!(result.is_err());
    }
}
