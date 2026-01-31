use crate::{Result, StorageError};
use luxtensor_core::{Block, BlockHeader, Transaction};
use luxtensor_crypto::Hash;
use rocksdb::{ColumnFamilyDescriptor, Options, WriteBatch, DB};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Column family names
const CF_BLOCKS: &str = "blocks";
const CF_HEADERS: &str = "headers";
const CF_TRANSACTIONS: &str = "transactions";
const CF_HEIGHT_TO_HASH: &str = "height_to_hash";
const CF_TX_TO_BLOCK: &str = "tx_to_block";
const CF_META: &str = "metadata";
const CF_RECEIPTS: &str = "receipts";
const CF_CONTRACTS: &str = "contracts";
const CF_STAKES: &str = "stakes";
const CF_SUBNETS: &str = "subnets";
const CF_NEURONS: &str = "neurons";
const CF_WEIGHTS: &str = "weights";
const CF_VALIDATORS: &str = "validators";

/// Metadata keys
const META_BEST_HEIGHT: &[u8] = b"best_height";

/// Blockchain database using RocksDB
pub struct BlockchainDB {
    db: Arc<DB>,
    /// In-memory cache of best block height for fast access
    best_height: Arc<AtomicU64>,
}

impl BlockchainDB {
    /// Open a blockchain database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_max_open_files(10000);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        // Define column families
        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_BLOCKS, Options::default()),
            ColumnFamilyDescriptor::new(CF_HEADERS, Options::default()),
            ColumnFamilyDescriptor::new(CF_TRANSACTIONS, Options::default()),
            ColumnFamilyDescriptor::new(CF_HEIGHT_TO_HASH, Options::default()),
            ColumnFamilyDescriptor::new(CF_TX_TO_BLOCK, Options::default()),
            ColumnFamilyDescriptor::new(CF_META, Options::default()),
            ColumnFamilyDescriptor::new(CF_RECEIPTS, Options::default()),
            ColumnFamilyDescriptor::new(CF_CONTRACTS, Options::default()),
            ColumnFamilyDescriptor::new(CF_STAKES, Options::default()),
            ColumnFamilyDescriptor::new(CF_SUBNETS, Options::default()),
            ColumnFamilyDescriptor::new(CF_NEURONS, Options::default()),
            ColumnFamilyDescriptor::new(CF_WEIGHTS, Options::default()),
            ColumnFamilyDescriptor::new(CF_VALIDATORS, Options::default()),
        ];

        let db = DB::open_cf_descriptors(&opts, path, cfs)?;

        // Load best_height from storage into memory
        let initial_height = if let Some(cf_meta) = db.cf_handle(CF_META) {
            db.get_cf(cf_meta, META_BEST_HEIGHT)
                .ok()
                .flatten()
                .and_then(|bytes| {
                    if bytes.len() == 8 {
                        Some(u64::from_be_bytes(bytes.as_slice().try_into().ok()?))
                    } else {
                        None
                    }
                })
                .unwrap_or(0)
        } else {
            0
        };

        Ok(Self {
            db: Arc::new(db),
            best_height: Arc::new(AtomicU64::new(initial_height)),
        })
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

        // Update best height if this block is higher (or first block)
        let cf_meta = self.db.cf_handle(CF_META).ok_or_else(|| {
            StorageError::DatabaseError("CF_META not found".to_string())
        })?;
        let current_best = self.db.get_cf(cf_meta, META_BEST_HEIGHT)?
            .and_then(|bytes| {
                if bytes.len() == 8 {
                    Some(u64::from_be_bytes(bytes.as_slice().try_into().ok()?))
                } else {
                    None
                }
            });

        // Update if no best height yet OR this block is higher
        let should_update = match current_best {
            None => true,  // First block ever
            Some(current) => height > current,
        };

        if should_update {
            batch.put_cf(cf_meta, META_BEST_HEIGHT, height.to_be_bytes());
        }

        // Write batch atomically
        self.db.write(batch)?;

        // Update in-memory best_height atomically
        // Use fetch_max to ensure we only increase, never decrease
        self.best_height.fetch_max(height, Ordering::SeqCst);

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
    /// Uses binary search with get_block_by_height for guaranteed accuracy
    pub fn get_best_height(&self) -> Result<Option<u64>> {
        // Quick cache check - if cache is ahead of 0, use as starting point
        let cached = self.best_height.load(Ordering::SeqCst);

        // Linear scan from cached value - most efficient for normal operation
        // since new blocks are added incrementally
        let mut height = cached;

        // First check if we have any blocks
        if self.get_block_by_height(0)?.is_none() {
            return Ok(None);
        }

        // Scan forward from cached position to find actual best
        while self.get_block_by_height(height + 1)?.is_some() {
            height += 1;
        }

        // Update cache if we found higher blocks
        if height > cached {
            self.best_height.store(height, Ordering::SeqCst);
        }

        Ok(Some(height))
    }

    /// Prune old blocks below the specified height to free up disk space
    /// Keeps blocks from keep_from_height onwards
    /// Returns the number of blocks pruned
    pub fn prune_old_blocks(&self, keep_from_height: u64) -> Result<u64> {
        if keep_from_height == 0 {
            return Ok(0); // Don't prune genesis
        }

        let cf_blocks = self.db.cf_handle(CF_BLOCKS)
            .ok_or_else(|| StorageError::DatabaseError("CF_BLOCKS not found".into()))?;
        let cf_height = self.db.cf_handle(CF_HEIGHT_TO_HASH)
            .ok_or_else(|| StorageError::DatabaseError("CF_HEIGHT_TO_HASH not found".into()))?;

        let mut pruned_count = 0u64;
        let mut batch = WriteBatch::default();

        // Prune blocks from 1 to keep_from_height - 1 (keep genesis at 0)
        for height in 1..keep_from_height {
            let height_key = height.to_be_bytes();

            // Get block hash at this height
            if let Some(hash_bytes) = self.db.get_cf(&cf_height, &height_key)? {
                // Delete block data
                batch.delete_cf(&cf_blocks, &hash_bytes);
                // Delete height->hash mapping
                batch.delete_cf(&cf_height, &height_key);
                pruned_count += 1;
            }
        }

        // Write all deletions atomically
        if pruned_count > 0 {
            self.db.write(batch)?;
        }

        Ok(pruned_count)
    }

    /// Store a transaction receipt
    pub fn store_receipt(&self, tx_hash: &Hash, receipt_bytes: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_RECEIPTS)
            .ok_or_else(|| StorageError::DatabaseError("CF_RECEIPTS not found".into()))?;
        self.db.put_cf(&cf, tx_hash, receipt_bytes)?;
        Ok(())
    }

    /// Get a transaction receipt by tx hash
    pub fn get_receipt(&self, tx_hash: &Hash) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_RECEIPTS)
            .ok_or_else(|| StorageError::DatabaseError("CF_RECEIPTS not found".into()))?;
        Ok(self.db.get_cf(&cf, tx_hash)?)
    }

    /// Store contract code
    pub fn store_contract(&self, address: &[u8; 20], code: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_CONTRACTS)
            .ok_or_else(|| StorageError::DatabaseError("CF_CONTRACTS not found".into()))?;
        self.db.put_cf(&cf, address, code)?;
        Ok(())
    }

    /// Get contract code by address
    pub fn get_contract(&self, address: &[u8; 20]) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_CONTRACTS)
            .ok_or_else(|| StorageError::DatabaseError("CF_CONTRACTS not found".into()))?;
        Ok(self.db.get_cf(&cf, address)?)
    }

    // ============ STAKE STORAGE METHODS ============

    /// Store stake lock data (address -> (amount, unlock_timestamp, bonus_rate))
    pub fn store_stake(&self, address: &[u8; 20], stake_data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_STAKES)
            .ok_or_else(|| StorageError::DatabaseError("CF_STAKES not found".into()))?;
        self.db.put_cf(&cf, address, stake_data)?;
        Ok(())
    }

    /// Get stake lock data by address
    pub fn get_stake(&self, address: &[u8; 20]) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_STAKES)
            .ok_or_else(|| StorageError::DatabaseError("CF_STAKES not found".into()))?;
        Ok(self.db.get_cf(&cf, address)?)
    }

    /// Remove stake lock data (after unlock)
    pub fn remove_stake(&self, address: &[u8; 20]) -> Result<()> {
        let cf = self.db.cf_handle(CF_STAKES)
            .ok_or_else(|| StorageError::DatabaseError("CF_STAKES not found".into()))?;
        self.db.delete_cf(&cf, address)?;
        Ok(())
    }

    /// Get all stakes (for loading on startup)
    pub fn get_all_stakes(&self) -> Result<Vec<([u8; 20], Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_STAKES)
            .ok_or_else(|| StorageError::DatabaseError("CF_STAKES not found".into()))?;

        let mut stakes = Vec::new();
        let iter = self.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (key, value) = item?;
            if key.len() == 20 {
                let mut addr = [0u8; 20];
                addr.copy_from_slice(&key);
                stakes.push((addr, value.to_vec()));
            }
        }

        Ok(stakes)
    }

    // ============ SUBNET STORAGE METHODS ============

    /// Store subnet data (subnet_id -> serialized SubnetInfo)
    pub fn store_subnet(&self, subnet_id: u64, data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_SUBNETS)
            .ok_or_else(|| StorageError::DatabaseError("CF_SUBNETS not found".into()))?;
        self.db.put_cf(&cf, subnet_id.to_be_bytes(), data)?;
        Ok(())
    }

    /// Get subnet data by ID
    pub fn get_subnet(&self, subnet_id: u64) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_SUBNETS)
            .ok_or_else(|| StorageError::DatabaseError("CF_SUBNETS not found".into()))?;
        Ok(self.db.get_cf(&cf, subnet_id.to_be_bytes())?)
    }

    /// Get all subnets
    pub fn get_all_subnets(&self) -> Result<Vec<(u64, Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_SUBNETS)
            .ok_or_else(|| StorageError::DatabaseError("CF_SUBNETS not found".into()))?;
        let mut subnets = Vec::new();
        let iter = self.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            if key.len() == 8 {
                if let Ok(bytes) = key[..8].try_into() {
                    let id = u64::from_be_bytes(bytes);
                    subnets.push((id, value.to_vec()));
                }
            }
        }
        Ok(subnets)
    }

    // ============ NEURON STORAGE METHODS ============

    /// Store neuron data (subnet_id, uid) -> serialized NeuronInfo
    pub fn store_neuron(&self, subnet_id: u64, uid: u64, data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_NEURONS)
            .ok_or_else(|| StorageError::DatabaseError("CF_NEURONS not found".into()))?;
        let mut key = [0u8; 16];
        key[..8].copy_from_slice(&subnet_id.to_be_bytes());
        key[8..].copy_from_slice(&uid.to_be_bytes());
        self.db.put_cf(&cf, key, data)?;
        Ok(())
    }

    /// Get neuron data
    pub fn get_neuron(&self, subnet_id: u64, uid: u64) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_NEURONS)
            .ok_or_else(|| StorageError::DatabaseError("CF_NEURONS not found".into()))?;
        let mut key = [0u8; 16];
        key[..8].copy_from_slice(&subnet_id.to_be_bytes());
        key[8..].copy_from_slice(&uid.to_be_bytes());
        Ok(self.db.get_cf(&cf, key)?)
    }

    /// Get all neurons
    pub fn get_all_neurons(&self) -> Result<Vec<((u64, u64), Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_NEURONS)
            .ok_or_else(|| StorageError::DatabaseError("CF_NEURONS not found".into()))?;
        let mut neurons = Vec::new();
        let iter = self.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            if key.len() == 16 {
                if let (Ok(subnet_bytes), Ok(uid_bytes)) = (key[..8].try_into(), key[8..16].try_into()) {
                    let subnet_id = u64::from_be_bytes(subnet_bytes);
                    let uid = u64::from_be_bytes(uid_bytes);
                    neurons.push(((subnet_id, uid), value.to_vec()));
                }
            }
        }
        Ok(neurons)
    }

    // ============ WEIGHT STORAGE METHODS ============

    /// Store weight data (subnet_id, uid) -> serialized Vec<WeightInfo>
    pub fn store_weights(&self, subnet_id: u64, uid: u64, data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_WEIGHTS)
            .ok_or_else(|| StorageError::DatabaseError("CF_WEIGHTS not found".into()))?;
        let mut key = [0u8; 16];
        key[..8].copy_from_slice(&subnet_id.to_be_bytes());
        key[8..].copy_from_slice(&uid.to_be_bytes());
        self.db.put_cf(&cf, key, data)?;
        Ok(())
    }

    /// Get weight data
    pub fn get_weights(&self, subnet_id: u64, uid: u64) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_WEIGHTS)
            .ok_or_else(|| StorageError::DatabaseError("CF_WEIGHTS not found".into()))?;
        let mut key = [0u8; 16];
        key[..8].copy_from_slice(&subnet_id.to_be_bytes());
        key[8..].copy_from_slice(&uid.to_be_bytes());
        Ok(self.db.get_cf(&cf, key)?)
    }

    /// Get all weights
    pub fn get_all_weights(&self) -> Result<Vec<((u64, u64), Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_WEIGHTS)
            .ok_or_else(|| StorageError::DatabaseError("CF_WEIGHTS not found".into()))?;
        let mut weights = Vec::new();
        let iter = self.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            if key.len() == 16 {
                if let (Ok(subnet_bytes), Ok(uid_bytes)) = (key[..8].try_into(), key[8..16].try_into()) {
                    let subnet_id = u64::from_be_bytes(subnet_bytes);
                    let uid = u64::from_be_bytes(uid_bytes);
                    weights.push(((subnet_id, uid), value.to_vec()));
                }
            }
        }
        Ok(weights)
    }

    // ============ VALIDATOR STORAGE METHODS ============

    /// Store validator data (address -> serialized Validator)
    pub fn store_validator(&self, address: &[u8; 20], data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_VALIDATORS)
            .ok_or_else(|| StorageError::DatabaseError("CF_VALIDATORS not found".into()))?;
        self.db.put_cf(&cf, address, data)?;
        Ok(())
    }

    /// Get validator data by address
    pub fn get_validator(&self, address: &[u8; 20]) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_VALIDATORS)
            .ok_or_else(|| StorageError::DatabaseError("CF_VALIDATORS not found".into()))?;
        Ok(self.db.get_cf(&cf, address)?)
    }

    /// Remove validator
    pub fn remove_validator(&self, address: &[u8; 20]) -> Result<()> {
        let cf = self.db.cf_handle(CF_VALIDATORS)
            .ok_or_else(|| StorageError::DatabaseError("CF_VALIDATORS not found".into()))?;
        self.db.delete_cf(&cf, address)?;
        Ok(())
    }

    /// Get all validators
    pub fn get_all_validators(&self) -> Result<Vec<([u8; 20], Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_VALIDATORS)
            .ok_or_else(|| StorageError::DatabaseError("CF_VALIDATORS not found".into()))?;
        let mut validators = Vec::new();
        let iter = self.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            if key.len() == 20 {
                let mut addr = [0u8; 20];
                addr.copy_from_slice(&key);
                validators.push((addr, value.to_vec()));
            }
        }
        Ok(validators)
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

        // Must store block 0 first (genesis) for get_best_height to work
        db.store_block(&create_test_block(0)).unwrap();
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
