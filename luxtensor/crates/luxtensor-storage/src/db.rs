use crate::{Result, StorageError};
use luxtensor_core::{Block, BlockHeader, Transaction};
use luxtensor_crypto::Hash;
use rocksdb::{ColumnFamilyDescriptor, Options, WriteBatch, DB};
use std::path::Path;
use std::sync::Arc;

/// Column family names
const CF_BLOCKS: &str = "blocks";
const CF_HEADERS: &str = "headers";
const CF_TRANSACTIONS: &str = "transactions";
const CF_HEIGHT_TO_HASH: &str = "height_to_hash";
const CF_TX_TO_BLOCK: &str = "tx_to_block";
const CF_RECEIPTS: &str = "receipts";
const CF_CONTRACTS: &str = "contracts";
// Metagraph column families
const CF_SUBNETS: &str = "subnets";
const CF_NEURONS: &str = "neurons";
const CF_VALIDATORS: &str = "validators";
const CF_STAKES: &str = "stakes";
const CF_WEIGHTS: &str = "weights";

/// Blockchain database using RocksDB
pub struct BlockchainDB {
    db: Arc<DB>,
}

impl BlockchainDB {
    /// Open a blockchain database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_max_open_files(10000);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        // Memory optimizations for 16GB RAM
        opts.set_write_buffer_size(128 * 1024 * 1024);  // 128MB write buffer
        opts.set_max_write_buffer_number(4);             // 4 buffers before flush
        opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB SST files

        // Define column families (core + metagraph)
        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_BLOCKS, Options::default()),
            ColumnFamilyDescriptor::new(CF_HEADERS, Options::default()),
            ColumnFamilyDescriptor::new(CF_TRANSACTIONS, Options::default()),
            ColumnFamilyDescriptor::new(CF_HEIGHT_TO_HASH, Options::default()),
            ColumnFamilyDescriptor::new(CF_TX_TO_BLOCK, Options::default()),
            ColumnFamilyDescriptor::new(CF_RECEIPTS, Options::default()),
            ColumnFamilyDescriptor::new(CF_CONTRACTS, Options::default()),
            // Metagraph
            ColumnFamilyDescriptor::new(CF_SUBNETS, Options::default()),
            ColumnFamilyDescriptor::new(CF_NEURONS, Options::default()),
            ColumnFamilyDescriptor::new(CF_VALIDATORS, Options::default()),
            ColumnFamilyDescriptor::new(CF_STAKES, Options::default()),
            ColumnFamilyDescriptor::new(CF_WEIGHTS, Options::default()),
        ];

        let db = DB::open_cf_descriptors(&opts, path, cfs)?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Get the inner DB reference for state database
    pub fn inner_db(&self) -> Arc<DB> {
        self.db.clone()
    }

    /// Store a block and index its transactions
    pub fn store_block(&self, block: &Block) -> Result<()> {
        let block_hash = block.hash();
        let height = block.header.height;

        let mut batch = WriteBatch::default();

        // Store block
        let block_bytes = bincode::serialize(block)?;
        let cf_blocks = self
            .db
            .cf_handle(CF_BLOCKS)
            .ok_or_else(|| StorageError::DatabaseError("CF_BLOCKS not found".to_string()))?;
        batch.put_cf(cf_blocks, block_hash, block_bytes);

        // Store header separately for faster lookups
        let header_bytes = bincode::serialize(&block.header)?;
        let cf_headers = self
            .db
            .cf_handle(CF_HEADERS)
            .ok_or_else(|| StorageError::DatabaseError("CF_HEADERS not found".to_string()))?;
        batch.put_cf(cf_headers, block_hash, header_bytes);

        // Index height -> hash
        let cf_height = self.db.cf_handle(CF_HEIGHT_TO_HASH).ok_or_else(|| {
            StorageError::DatabaseError("CF_HEIGHT_TO_HASH not found".to_string())
        })?;
        batch.put_cf(cf_height, height.to_be_bytes(), block_hash);

        // Index transactions
        let cf_txs = self.db.cf_handle(CF_TRANSACTIONS).ok_or_else(|| {
            StorageError::DatabaseError("CF_TRANSACTIONS not found".to_string())
        })?;
        let cf_tx_to_block = self.db.cf_handle(CF_TX_TO_BLOCK).ok_or_else(|| {
            StorageError::DatabaseError("CF_TX_TO_BLOCK not found".to_string())
        })?;

        for tx in &block.transactions {
            let tx_hash = tx.hash();
            let tx_bytes = bincode::serialize(tx)?;
            batch.put_cf(cf_txs, tx_hash, tx_bytes);
            batch.put_cf(cf_tx_to_block, tx_hash, block_hash);
        }

        // Write batch atomically
        self.db.write(batch)?;

        Ok(())
    }

    /// Get a block by its hash
    pub fn get_block(&self, hash: &Hash) -> Result<Option<Block>> {
        let cf_blocks = self
            .db
            .cf_handle(CF_BLOCKS)
            .ok_or_else(|| StorageError::DatabaseError("CF_BLOCKS not found".to_string()))?;

        match self.db.get_cf(cf_blocks, hash)? {
            Some(bytes) => {
                let block = bincode::deserialize(&bytes)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    /// Get a block by its height
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>> {
        let cf_height = self.db.cf_handle(CF_HEIGHT_TO_HASH).ok_or_else(|| {
            StorageError::DatabaseError("CF_HEIGHT_TO_HASH not found".to_string())
        })?;

        match self.db.get_cf(cf_height, height.to_be_bytes())? {
            Some(hash_bytes) => {
                let hash: Hash = hash_bytes
                    .try_into()
                    .map_err(|_| StorageError::DatabaseError("Invalid hash size".to_string()))?;
                self.get_block(&hash)
            }
            None => Ok(None),
        }
    }

    /// Get a block header by hash
    pub fn get_header(&self, hash: &Hash) -> Result<Option<BlockHeader>> {
        let cf_headers = self
            .db
            .cf_handle(CF_HEADERS)
            .ok_or_else(|| StorageError::DatabaseError("CF_HEADERS not found".to_string()))?;

        match self.db.get_cf(cf_headers, hash)? {
            Some(bytes) => {
                let header = bincode::deserialize(&bytes)?;
                Ok(Some(header))
            }
            None => Ok(None),
        }
    }

    /// Get a transaction by its hash
    pub fn get_transaction(&self, hash: &Hash) -> Result<Option<Transaction>> {
        let cf_txs = self.db.cf_handle(CF_TRANSACTIONS).ok_or_else(|| {
            StorageError::DatabaseError("CF_TRANSACTIONS not found".to_string())
        })?;

        match self.db.get_cf(cf_txs, hash)? {
            Some(bytes) => {
                let tx = bincode::deserialize(&bytes)?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    /// Get the block hash that contains a transaction
    pub fn get_block_hash_by_tx(&self, tx_hash: &Hash) -> Result<Option<Hash>> {
        let cf_tx_to_block = self.db.cf_handle(CF_TX_TO_BLOCK).ok_or_else(|| {
            StorageError::DatabaseError("CF_TX_TO_BLOCK not found".to_string())
        })?;

        match self.db.get_cf(cf_tx_to_block, tx_hash)? {
            Some(hash_bytes) => {
                let hash: Hash = hash_bytes
                    .try_into()
                    .map_err(|_| StorageError::DatabaseError("Invalid hash size".to_string()))?;
                Ok(Some(hash))
            }
            None => Ok(None),
        }
    }

    /// Get the current best block height
    pub fn get_best_height(&self) -> Result<Option<u64>> {
        let cf_height = self.db.cf_handle(CF_HEIGHT_TO_HASH).ok_or_else(|| {
            StorageError::DatabaseError("CF_HEIGHT_TO_HASH not found".to_string())
        })?;

        // Iterate backwards from u64::MAX to find the highest height
        let mut iter = self.db.iterator_cf(cf_height, rocksdb::IteratorMode::End);
        if let Some(Ok((key, _))) = iter.next() {
            let height = u64::from_be_bytes(
                key.as_ref()
                    .try_into()
                    .map_err(|_| StorageError::DatabaseError("Invalid height key".to_string()))?,
            );
            Ok(Some(height))
        } else {
            Ok(None)
        }
    }

    // ==================== METAGRAPH OPERATIONS ====================

    /// Store a subnet
    pub fn store_subnet(&self, subnet_id: u64, data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_SUBNETS).ok_or_else(|| {
            StorageError::DatabaseError("CF_SUBNETS not found".to_string())
        })?;
        self.db.put_cf(cf, subnet_id.to_be_bytes(), data)?;
        Ok(())
    }

    /// Get all subnets as (id, data) pairs
    pub fn get_all_subnets(&self) -> Result<Vec<(u64, Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_SUBNETS).ok_or_else(|| {
            StorageError::DatabaseError("CF_SUBNETS not found".to_string())
        })?;
        let mut subnets = Vec::new();
        for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (key, value) = item?;
            if key.len() >= 8 {
                let id = u64::from_be_bytes(key[..8].try_into().unwrap());
                subnets.push((id, value.to_vec()));
            }
        }
        Ok(subnets)
    }

    /// Store a neuron
    pub fn store_neuron(&self, subnet_id: u64, uid: u64, data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_NEURONS).ok_or_else(|| {
            StorageError::DatabaseError("CF_NEURONS not found".to_string())
        })?;
        let mut key = Vec::with_capacity(16);
        key.extend_from_slice(&subnet_id.to_be_bytes());
        key.extend_from_slice(&uid.to_be_bytes());
        self.db.put_cf(cf, key, data)?;
        Ok(())
    }

    /// Get all neurons as ((subnet_id, uid), data) pairs
    pub fn get_all_neurons(&self) -> Result<Vec<((u64, u64), Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_NEURONS).ok_or_else(|| {
            StorageError::DatabaseError("CF_NEURONS not found".to_string())
        })?;
        let mut neurons = Vec::new();
        for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (key, value) = item?;
            if key.len() >= 16 {
                let subnet_id = u64::from_be_bytes(key[..8].try_into().unwrap());
                let uid = u64::from_be_bytes(key[8..16].try_into().unwrap());
                neurons.push(((subnet_id, uid), value.to_vec()));
            }
        }
        Ok(neurons)
    }

    /// Store a validator
    pub fn store_validator(&self, address: &[u8], data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_VALIDATORS).ok_or_else(|| {
            StorageError::DatabaseError("CF_VALIDATORS not found".to_string())
        })?;
        self.db.put_cf(cf, address, data)?;
        Ok(())
    }

    /// Get all validators
    pub fn get_all_validators(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_VALIDATORS).ok_or_else(|| {
            StorageError::DatabaseError("CF_VALIDATORS not found".to_string())
        })?;
        let mut validators = Vec::new();
        for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (key, value) = item?;
            validators.push((key.to_vec(), value.to_vec()));
        }
        Ok(validators)
    }

    /// Store stake data
    pub fn store_stake(&self, address: &[u8], data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_STAKES).ok_or_else(|| {
            StorageError::DatabaseError("CF_STAKES not found".to_string())
        })?;
        self.db.put_cf(cf, address, data)?;
        Ok(())
    }

    /// Remove stake data
    pub fn remove_stake(&self, address: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_STAKES).ok_or_else(|| {
            StorageError::DatabaseError("CF_STAKES not found".to_string())
        })?;
        self.db.delete_cf(cf, address)?;
        Ok(())
    }

    /// Get all stakes
    pub fn get_all_stakes(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_STAKES).ok_or_else(|| {
            StorageError::DatabaseError("CF_STAKES not found".to_string())
        })?;
        let mut stakes = Vec::new();
        for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (key, value) = item?;
            stakes.push((key.to_vec(), value.to_vec()));
        }
        Ok(stakes)
    }

    /// Store weights
    pub fn store_weights(&self, subnet_id: u64, uid: u64, data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_WEIGHTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_WEIGHTS not found".to_string())
        })?;
        let mut key = Vec::with_capacity(16);
        key.extend_from_slice(&subnet_id.to_be_bytes());
        key.extend_from_slice(&uid.to_be_bytes());
        self.db.put_cf(cf, key, data)?;
        Ok(())
    }

    /// Get all weights
    pub fn get_all_weights(&self) -> Result<Vec<((u64, u64), Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_WEIGHTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_WEIGHTS not found".to_string())
        })?;
        let mut weights = Vec::new();
        for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (key, value) = item?;
            if key.len() >= 16 {
                let subnet_id = u64::from_be_bytes(key[..8].try_into().unwrap());
                let uid = u64::from_be_bytes(key[8..16].try_into().unwrap());
                weights.push(((subnet_id, uid), value.to_vec()));
            }
        }
        Ok(weights)
    }

    // ==================== RECEIPT OPERATIONS ====================

    /// Store a transaction receipt
    pub fn store_receipt(&self, tx_hash: &Hash, data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_RECEIPTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_RECEIPTS not found".to_string())
        })?;
        self.db.put_cf(cf, tx_hash, data)?;
        Ok(())
    }

    /// Get a transaction receipt
    pub fn get_receipt(&self, tx_hash: &Hash) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_RECEIPTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_RECEIPTS not found".to_string())
        })?;
        match self.db.get_cf(cf, tx_hash)? {
            Some(bytes) => Ok(Some(bytes.to_vec())),
            None => Ok(None),
        }
    }

    // ==================== CONTRACT OPERATIONS ====================

    /// Store contract code
    pub fn store_contract(&self, address: &[u8], code: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_CONTRACTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_CONTRACTS not found".to_string())
        })?;
        self.db.put_cf(cf, address, code)?;
        Ok(())
    }

    /// Get contract code
    pub fn get_contract(&self, address: &[u8]) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_CONTRACTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_CONTRACTS not found".to_string())
        })?;
        match self.db.get_cf(cf, address)? {
            Some(bytes) => Ok(Some(bytes.to_vec())),
            None => Ok(None),
        }
    }

    // ==================== PRUNING OPERATIONS ====================

    /// Prune old receipts before a given block height
    /// Returns the number of receipts pruned
    /// Note: This is a simplified implementation - in production, receipts
    /// would be indexed by block height for efficient pruning
    pub fn prune_receipts_before_height(&self, _before_height: u64) -> Result<usize> {
        // For now, return 0 as we don't have height-indexed receipts yet
        // In production, receipts would be stored with height prefix for efficient pruning
        // This satisfies the API contract without actual pruning
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::{Address, Block, BlockHeader};
    use tempfile::TempDir;

    fn create_test_block(height: u64) -> Block {
        Block {
            header: BlockHeader {
                version: 1,
                height,
                timestamp: 1000 + height,
                previous_hash: [0u8; 32],
                state_root: [0u8; 32],
                txs_root: [0u8; 32],
                receipts_root: [0u8; 32],
                validator: [0u8; 32],
                signature: vec![0u8; 64],
                gas_used: 0,
                gas_limit: 1000000,
                extra_data: vec![],
            },
            transactions: vec![],
        }
    }

    fn create_test_block_with_tx(height: u64) -> Block {
        let tx = Transaction::new(
            0,
            Address::zero(),
            Some(Address::zero()),
            1000,
            1,
            21000,
            vec![],
        );

        Block {
            header: BlockHeader {
                version: 1,
                height,
                timestamp: 1000 + height,
                previous_hash: [0u8; 32],
                state_root: [0u8; 32],
                txs_root: [0u8; 32],
                receipts_root: [0u8; 32],
                validator: [0u8; 32],
                signature: vec![0u8; 64],
                gas_used: 21000,
                gas_limit: 1000000,
                extra_data: vec![],
            },
            transactions: vec![tx],
        }
    }

    #[test]
    fn test_db_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db = BlockchainDB::open(temp_dir.path()).unwrap();
        assert!(db.get_best_height().unwrap().is_none());
    }

    #[test]
    fn test_store_and_get_block() {
        let temp_dir = TempDir::new().unwrap();
        let db = BlockchainDB::open(temp_dir.path()).unwrap();

        let block = create_test_block(1);
        let hash = block.hash();

        db.store_block(&block).unwrap();

        let retrieved = db.get_block(&hash).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().header.height, 1);
    }

    #[test]
    fn test_get_block_by_height() {
        let temp_dir = TempDir::new().unwrap();
        let db = BlockchainDB::open(temp_dir.path()).unwrap();

        let block = create_test_block(5);
        db.store_block(&block).unwrap();

        let retrieved = db.get_block_by_height(5).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().header.height, 5);
    }

    #[test]
    fn test_get_header() {
        let temp_dir = TempDir::new().unwrap();
        let db = BlockchainDB::open(temp_dir.path()).unwrap();

        let block = create_test_block(3);
        let hash = block.hash();
        db.store_block(&block).unwrap();

        let header = db.get_header(&hash).unwrap();
        assert!(header.is_some());
        assert_eq!(header.unwrap().height, 3);
    }

    #[test]
    fn test_store_and_get_transaction() {
        let temp_dir = TempDir::new().unwrap();
        let db = BlockchainDB::open(temp_dir.path()).unwrap();

        let block = create_test_block_with_tx(1);
        let tx_hash = block.transactions[0].hash();

        db.store_block(&block).unwrap();

        let retrieved_tx = db.get_transaction(&tx_hash).unwrap();
        assert!(retrieved_tx.is_some());
    }

    #[test]
    fn test_get_block_hash_by_tx() {
        let temp_dir = TempDir::new().unwrap();
        let db = BlockchainDB::open(temp_dir.path()).unwrap();

        let block = create_test_block_with_tx(1);
        let block_hash = block.hash();
        let tx_hash = block.transactions[0].hash();

        db.store_block(&block).unwrap();

        let retrieved_block_hash = db.get_block_hash_by_tx(&tx_hash).unwrap();
        assert!(retrieved_block_hash.is_some());
        assert_eq!(retrieved_block_hash.unwrap(), block_hash);
    }

    #[test]
    fn test_get_best_height() {
        let temp_dir = TempDir::new().unwrap();
        let db = BlockchainDB::open(temp_dir.path()).unwrap();

        db.store_block(&create_test_block(1)).unwrap();
        db.store_block(&create_test_block(5)).unwrap();
        db.store_block(&create_test_block(3)).unwrap();

        let best_height = db.get_best_height().unwrap();
        assert_eq!(best_height, Some(5));
    }

    #[test]
    fn test_block_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let db = BlockchainDB::open(temp_dir.path()).unwrap();

        let result = db.get_block(&[0u8; 32]).unwrap();
        assert!(result.is_none());
    }
}
