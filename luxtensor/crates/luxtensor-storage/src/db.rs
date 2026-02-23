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
// EVM state column families (persistent contract storage)
const CF_EVM_ACCOUNTS: &str = "evm_accounts";
const CF_EVM_STORAGE: &str = "evm_storage";
// Fork choice persistence
const CF_FORK_CHOICE: &str = "fork_choice";
// Metadata column family for schema versioning
const CF_METADATA: &str = "metadata";

/// Key used to store schema version in the metadata column family
const SCHEMA_VERSION_KEY: &[u8] = b"schema_version";
/// Current schema version — increment when making incompatible DB changes
const CURRENT_SCHEMA_VERSION: u32 = 1;

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

        // Define column families (core + metagraph + EVM state)
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
            // EVM persistent state (survives node restart)
            ColumnFamilyDescriptor::new(CF_EVM_ACCOUNTS, Options::default()),
            ColumnFamilyDescriptor::new(CF_EVM_STORAGE, Options::default()),
            // Fork choice metadata
            ColumnFamilyDescriptor::new(CF_FORK_CHOICE, Options::default()),
            // Schema versioning metadata
            ColumnFamilyDescriptor::new(CF_METADATA, Options::default()),
        ];

        let db = DB::open_cf_descriptors(&opts, path, cfs)?;

        // --- Schema version check / initialization ---
        {
            let cf_meta = db
                .cf_handle(CF_METADATA)
                .ok_or_else(|| StorageError::DatabaseError("CF_METADATA not found".to_string()))?;

            match db.get_cf(cf_meta, SCHEMA_VERSION_KEY)? {
                Some(bytes) => {
                    let raw = <[u8; 4]>::try_from(bytes.as_ref())
                        .map_err(|_| {
                            StorageError::DatabaseError(
                                "Invalid schema_version value in DB".to_string(),
                            )
                        })?;
                    let found = u32::from_le_bytes(raw);
                    if found != CURRENT_SCHEMA_VERSION {
                        return Err(StorageError::SchemaMismatch {
                            found,
                            expected: CURRENT_SCHEMA_VERSION,
                        });
                    }
                }
                None => {
                    // Fresh database — write current schema version
                    db.put_cf(cf_meta, SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION.to_le_bytes())?;
                }
            }
        }

        Ok(Self { db: Arc::new(db) })
    }

    /// Get the inner DB reference for state database
    pub fn inner_db(&self) -> Arc<DB> {
        self.db.clone()
    }

    /// Returns the schema version stored in this database.
    pub fn schema_version(&self) -> Result<u32> {
        let cf_meta = self
            .db
            .cf_handle(CF_METADATA)
            .ok_or_else(|| StorageError::DatabaseError("CF_METADATA not found".to_string()))?;

        match self.db.get_cf(cf_meta, SCHEMA_VERSION_KEY)? {
            Some(bytes) => {
                let raw = <[u8; 4]>::try_from(bytes.as_ref())
                    .map_err(|_| {
                        StorageError::DatabaseError(
                            "Invalid schema_version value in DB".to_string(),
                        )
                    })?;
                let version = u32::from_le_bytes(raw);
                Ok(version)
            }
            None => Err(StorageError::DatabaseError(
                "schema_version key missing from metadata".to_string(),
            )),
        }
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

        // Store best_height metadata for O(1) lookup — only update if new height is greater
        // AND the block properly connects to the existing chain
        let cf_meta = self.db.cf_handle(CF_METADATA).ok_or_else(|| {
            StorageError::DatabaseError("CF_METADATA not found".to_string())
        })?;
        let should_update = match self.db.get_cf(cf_meta, b"best_height")? {
            Some(bytes) if bytes.len() >= 8 => {
                let current_best = u64::from_be_bytes(
                    bytes[..8]
                        .try_into()
                        .map_err(|_| StorageError::DatabaseError("Invalid best_height".to_string()))?,
                );
                if height > current_best {
                    // Verify chain connectivity: if a block at height-1 exists,
                    // the new block's previous_hash must match it
                    if height > 0 {
                        match self.db.get_cf(cf_height, (height - 1).to_be_bytes())? {
                            Some(prev_hash_bytes) => {
                                // Previous block exists — verify the link
                                block.header.previous_hash == prev_hash_bytes.as_ref()
                            }
                            None => {
                                // No block at height-1 (out-of-order import), allow update
                                true
                            }
                        }
                    } else {
                        true // Genesis block
                    }
                } else {
                    false
                }
            }
            _ => true, // No existing best_height, always set
        };
        if should_update {
            batch.put_cf(cf_meta, b"best_height", height.to_be_bytes());
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

    /// Get the current best block height.
    /// Uses O(1) metadata lookup with fallback to reverse iterator for backward compatibility.
    pub fn get_best_height(&self) -> Result<Option<u64>> {
        // Fast path: read from metadata key (O(1))
        if let Some(cf_meta) = self.db.cf_handle(CF_METADATA) {
            if let Some(bytes) = self.db.get_cf(cf_meta, b"best_height")? {
                if bytes.len() >= 8 {
                    let height = u64::from_be_bytes(
                        bytes[..8]
                            .try_into()
                            .map_err(|_| StorageError::DatabaseError("Invalid best_height".to_string()))?,
                    );
                    return Ok(Some(height));
                }
            }
        }

        // Fallback: iterate backwards (for DBs created before metadata key was added)
        let cf_height = self.db.cf_handle(CF_HEIGHT_TO_HASH).ok_or_else(|| {
            StorageError::DatabaseError("CF_HEIGHT_TO_HASH not found".to_string())
        })?;

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

    /// Prune old receipts for blocks before a given height.
    ///
    /// Since receipts are indexed by tx_hash (not block height), this method
    /// iterates blocks [0, before_height) and deletes receipts for each
    /// transaction in those blocks.
    ///
    /// Returns the number of receipts deleted.
    pub fn prune_receipts_before_height(&self, before_height: u64) -> Result<usize> {
        let cf = self.db.cf_handle(CF_RECEIPTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_RECEIPTS not found".to_string())
        })?;

        let mut batch = WriteBatch::default();
        let mut pruned = 0usize;

        for height in 0..before_height {
            let block = match self.get_block_by_height(height)? {
                Some(b) => b,
                None => continue,
            };

            for tx in &block.transactions {
                let tx_hash = tx.hash();
                // Only count if the receipt actually existed
                if self.db.get_cf(cf, &tx_hash)?.is_some() {
                    batch.delete_cf(cf, &tx_hash);
                    pruned += 1;
                }
            }
        }

        // Atomic write: all deletions succeed or none do
        if pruned > 0 {
            self.db.write(batch).map_err(|e| {
                StorageError::DatabaseError(format!("Failed to write prune batch: {}", e))
            })?;
            tracing::info!("Pruned {} receipts for blocks [0, {})", pruned, before_height);
        }

        Ok(pruned)
    }

    // ==================== EVM STATE PERSISTENCE ====================
    // These methods allow the EvmExecutor to persist contract storage
    // and account state to RocksDB, surviving node restarts.

    /// Store an EVM account (balance, nonce, code_hash, optional code).
    /// Key: 20-byte address. Value: bincode-serialized account record.
    pub fn store_evm_account(&self, address: &[u8; 20], data: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle(CF_EVM_ACCOUNTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_EVM_ACCOUNTS not found".to_string())
        })?;
        self.db.put_cf(cf, address, data)?;
        Ok(())
    }

    /// Get an EVM account by address
    pub fn get_evm_account(&self, address: &[u8; 20]) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle(CF_EVM_ACCOUNTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_EVM_ACCOUNTS not found".to_string())
        })?;
        match self.db.get_cf(cf, address)? {
            Some(bytes) => Ok(Some(bytes.to_vec())),
            None => Ok(None),
        }
    }

    /// Get all EVM accounts (for state loading on restart)
    pub fn get_all_evm_accounts(&self) -> Result<Vec<([u8; 20], Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_EVM_ACCOUNTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_EVM_ACCOUNTS not found".to_string())
        })?;
        let mut accounts = Vec::new();
        for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (key, value) = item?;
            if key.len() == 20 {
                let mut addr = [0u8; 20];
                addr.copy_from_slice(&key);
                accounts.push((addr, value.to_vec()));
            }
        }
        Ok(accounts)
    }

    /// Store an EVM storage slot.
    /// Key format: address(20) || slot(32) — 52 bytes total.
    /// Value: 32-byte storage value.
    pub fn store_evm_storage(&self, address: &[u8; 20], slot: &[u8; 32], value: &[u8; 32]) -> Result<()> {
        let cf = self.db.cf_handle(CF_EVM_STORAGE).ok_or_else(|| {
            StorageError::DatabaseError("CF_EVM_STORAGE not found".to_string())
        })?;
        let mut key = Vec::with_capacity(52);
        key.extend_from_slice(address);
        key.extend_from_slice(slot);
        self.db.put_cf(cf, key, value)?;
        Ok(())
    }

    /// Get an EVM storage slot value
    pub fn get_evm_storage(&self, address: &[u8; 20], slot: &[u8; 32]) -> Result<Option<[u8; 32]>> {
        let cf = self.db.cf_handle(CF_EVM_STORAGE).ok_or_else(|| {
            StorageError::DatabaseError("CF_EVM_STORAGE not found".to_string())
        })?;
        let mut key = Vec::with_capacity(52);
        key.extend_from_slice(address);
        key.extend_from_slice(slot);
        match self.db.get_cf(cf, key)? {
            Some(bytes) if bytes.len() == 32 => {
                let mut val = [0u8; 32];
                val.copy_from_slice(&bytes);
                Ok(Some(val))
            }
            _ => Ok(None),
        }
    }

    /// Get all storage slots for a given EVM address
    pub fn get_all_evm_storage_for_address(&self, address: &[u8; 20]) -> Result<Vec<([u8; 32], [u8; 32])>> {
        let cf = self.db.cf_handle(CF_EVM_STORAGE).ok_or_else(|| {
            StorageError::DatabaseError("CF_EVM_STORAGE not found".to_string())
        })?;
        let prefix = address.to_vec();
        let mut slots = Vec::new();
        for item in self.db.prefix_iterator_cf(cf, &prefix) {
            let (key, value) = item?;
            if key.starts_with(&prefix) && key.len() == 52 && value.len() == 32 {
                let mut slot = [0u8; 32];
                slot.copy_from_slice(&key[20..52]);
                let mut val = [0u8; 32];
                val.copy_from_slice(&value);
                slots.push((slot, val));
            } else if !key.starts_with(&prefix) {
                break;
            }
        }
        Ok(slots)
    }

    /// Batch-write EVM state changes (accounts + storage) atomically.
    /// Called after each block execution to persist all EVM state changes.
    pub fn flush_evm_state(
        &self,
        accounts: &[([u8; 20], Vec<u8>)],
        storage: &[([u8; 20], [u8; 32], [u8; 32])],
        deleted_accounts: &[[u8; 20]],
    ) -> Result<()> {
        let cf_accounts = self.db.cf_handle(CF_EVM_ACCOUNTS).ok_or_else(|| {
            StorageError::DatabaseError("CF_EVM_ACCOUNTS not found".to_string())
        })?;
        let cf_storage = self.db.cf_handle(CF_EVM_STORAGE).ok_or_else(|| {
            StorageError::DatabaseError("CF_EVM_STORAGE not found".to_string())
        })?;

        let mut batch = WriteBatch::default();

        // Delete self-destructed accounts
        for addr in deleted_accounts {
            batch.delete_cf(cf_accounts, addr);
        }

        // Write accounts
        for (addr, data) in accounts {
            batch.put_cf(cf_accounts, addr, data);
        }

        // Write storage slots
        for (addr, slot, value) in storage {
            let mut key = Vec::with_capacity(52);
            key.extend_from_slice(addr);
            key.extend_from_slice(slot);

            if value == &[0u8; 32] {
                batch.delete_cf(cf_storage, key);
            } else {
                batch.put_cf(cf_storage, key, value);
            }
        }

        self.db.write(batch)?;
        Ok(())
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

// ============================================================================
// Implement RocksDbLike trait for BlockchainDB so core::StateDB can persist
// ============================================================================
impl luxtensor_core::RocksDbLike for BlockchainDB {
    fn put(&self, key: &[u8], value: &[u8]) -> std::result::Result<(), String> {
        self.db.put(key, value).map_err(|e| e.to_string())
    }

    fn get(&self, key: &[u8]) -> std::result::Result<Option<Vec<u8>>, String> {
        self.db.get(key).map_err(|e| e.to_string())
    }

    fn prefix_scan(&self, prefix: &[u8]) -> std::result::Result<Vec<(Vec<u8>, Vec<u8>)>, String> {
        let mut result = Vec::new();
        let iter = self.db.prefix_iterator(prefix);
        for item in iter {
            match item {
                Ok((key, value)) => {
                    if key.starts_with(prefix) {
                        result.push((key.to_vec(), value.to_vec()));
                    } else {
                        break;
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(result)
    }
}

// ============================================================================
// Fork Choice Persistence — stores head hash, scores, attestation stakes
// ============================================================================
impl BlockchainDB {
    /// Key constant for fork choice head
    const FC_HEAD_KEY: &'static [u8] = b"fc_head";

    /// Store the current fork choice head hash
    pub fn store_fork_choice_head(&self, head_hash: &Hash) -> Result<()> {
        let cf = self.db.cf_handle(CF_FORK_CHOICE)
            .ok_or_else(|| StorageError::DatabaseError("CF_FORK_CHOICE not found".to_string()))?;
        self.db.put_cf(cf, Self::FC_HEAD_KEY, head_hash)?;
        Ok(())
    }

    /// Load the stored fork choice head hash
    pub fn load_fork_choice_head(&self) -> Result<Option<Hash>> {
        let cf = self.db.cf_handle(CF_FORK_CHOICE)
            .ok_or_else(|| StorageError::DatabaseError("CF_FORK_CHOICE not found".to_string()))?;
        match self.db.get_cf(cf, Self::FC_HEAD_KEY)? {
            Some(bytes) if bytes.len() == 32 => {
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&bytes);
                Ok(Some(hash))
            }
            _ => Ok(None),
        }
    }

    /// Store a block score for fork choice (key = "score:" + block_hash)
    pub fn store_block_score(&self, block_hash: &Hash, score: u64) -> Result<()> {
        let cf = self.db.cf_handle(CF_FORK_CHOICE)
            .ok_or_else(|| StorageError::DatabaseError("CF_FORK_CHOICE not found".to_string()))?;
        let mut key = Vec::with_capacity(38);
        key.extend_from_slice(b"score:");
        key.extend_from_slice(block_hash);
        self.db.put_cf(cf, &key, &score.to_be_bytes())?;
        Ok(())
    }

    /// Load a block score
    pub fn load_block_score(&self, block_hash: &Hash) -> Result<Option<u64>> {
        let cf = self.db.cf_handle(CF_FORK_CHOICE)
            .ok_or_else(|| StorageError::DatabaseError("CF_FORK_CHOICE not found".to_string()))?;
        let mut key = Vec::with_capacity(38);
        key.extend_from_slice(b"score:");
        key.extend_from_slice(block_hash);
        match self.db.get_cf(cf, &key)? {
            Some(bytes) if bytes.len() == 8 => {
                let score = u64::from_be_bytes(bytes[..8].try_into().unwrap());
                Ok(Some(score))
            }
            _ => Ok(None),
        }
    }

    /// Store attestation stake for a block (key = "attn:" + block_hash)
    pub fn store_attestation_stake(&self, block_hash: &Hash, stake: u128) -> Result<()> {
        let cf = self.db.cf_handle(CF_FORK_CHOICE)
            .ok_or_else(|| StorageError::DatabaseError("CF_FORK_CHOICE not found".to_string()))?;
        let mut key = Vec::with_capacity(37);
        key.extend_from_slice(b"attn:");
        key.extend_from_slice(block_hash);
        self.db.put_cf(cf, &key, &stake.to_be_bytes())?;
        Ok(())
    }

    /// Flush all fork choice state atomically:
    /// head hash + all block scores + attestation stakes
    pub fn flush_fork_choice_state(
        &self,
        head_hash: &Hash,
        scores: &[(Hash, u64)],
        attestation_stakes: &[(Hash, u128)],
    ) -> Result<()> {
        let cf = self.db.cf_handle(CF_FORK_CHOICE)
            .ok_or_else(|| StorageError::DatabaseError("CF_FORK_CHOICE not found".to_string()))?;

        let mut batch = WriteBatch::default();

        // Head
        batch.put_cf(cf, Self::FC_HEAD_KEY, head_hash);

        // Scores
        for (hash, score) in scores {
            let mut key = Vec::with_capacity(38);
            key.extend_from_slice(b"score:");
            key.extend_from_slice(hash);
            batch.put_cf(cf, &key, &score.to_be_bytes());
        }

        // Attestation stakes
        for (hash, stake) in attestation_stakes {
            let mut key = Vec::with_capacity(37);
            key.extend_from_slice(b"attn:");
            key.extend_from_slice(hash);
            batch.put_cf(cf, &key, &stake.to_be_bytes());
        }

        self.db.write(batch)?;
        Ok(())
    }

    /// Load all block scores from the fork choice CF
    pub fn load_all_block_scores(&self) -> Result<Vec<(Hash, u64)>> {
        let cf = self.db.cf_handle(CF_FORK_CHOICE)
            .ok_or_else(|| StorageError::DatabaseError("CF_FORK_CHOICE not found".to_string()))?;
        let prefix = b"score:";
        let mut result = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::From(prefix, rocksdb::Direction::Forward));
        for item in iter {
            match item {
                Ok((key, value)) => {
                    if !key.starts_with(prefix) { break; }
                    if key.len() == 38 && value.len() == 8 {
                        let mut hash = [0u8; 32];
                        hash.copy_from_slice(&key[6..38]);
                        let score = u64::from_be_bytes(value[..8].try_into().unwrap());
                        result.push((hash, score));
                    }
                }
                Err(e) => return Err(StorageError::DatabaseError(e.to_string())),
            }
        }
        Ok(result)
    }

    /// Load all attestation stakes from the fork choice CF
    pub fn load_all_attestation_stakes(&self) -> Result<Vec<(Hash, u128)>> {
        let cf = self.db.cf_handle(CF_FORK_CHOICE)
            .ok_or_else(|| StorageError::DatabaseError("CF_FORK_CHOICE not found".to_string()))?;
        let prefix = b"attn:";
        let mut result = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::From(prefix, rocksdb::Direction::Forward));
        for item in iter {
            match item {
                Ok((key, value)) => {
                    if !key.starts_with(prefix) { break; }
                    if key.len() == 37 && value.len() == 16 {
                        let mut hash = [0u8; 32];
                        hash.copy_from_slice(&key[5..37]);
                        let stake = u128::from_be_bytes(value[..16].try_into().unwrap());
                        result.push((hash, stake));
                    }
                }
                Err(e) => return Err(StorageError::DatabaseError(e.to_string())),
            }
        }
        Ok(result)
    }
}

// ============================================================================
// Implement EvmStateStore trait so BlockchainDB can persist EVM state
// ============================================================================
impl crate::evm_store::EvmStateStore for BlockchainDB {
    fn load_all_evm_accounts(&self) -> std::result::Result<Vec<([u8; 20], crate::evm_store::EvmAccountRecord)>, String> {
        let raw = self.get_all_evm_accounts().map_err(|e| e.to_string())?;
        let mut result = Vec::with_capacity(raw.len());
        for (addr, data) in raw {
            let record: crate::evm_store::EvmAccountRecord =
                bincode::deserialize(&data).map_err(|e| e.to_string())?;
            result.push((addr, record));
        }
        Ok(result)
    }

    fn load_all_evm_storage(&self) -> std::result::Result<Vec<([u8; 20], [u8; 32], [u8; 32])>, String> {
        // Iterate CF_EVM_STORAGE directly
        let cf = self.db.cf_handle(CF_EVM_STORAGE)
            .ok_or_else(|| "CF_EVM_STORAGE not found".to_string())?;
        let mut entries = Vec::new();
        for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (key, value) = item.map_err(|e| e.to_string())?;
            if key.len() == 52 && value.len() == 32 {
                let mut addr = [0u8; 20];
                addr.copy_from_slice(&key[..20]);
                let mut slot = [0u8; 32];
                slot.copy_from_slice(&key[20..52]);
                let mut val = [0u8; 32];
                val.copy_from_slice(&value);
                entries.push((addr, slot, val));
            }
        }
        Ok(entries)
    }

    fn flush_evm_state(
        &self,
        accounts: &[([u8; 20], Vec<u8>)],
        storage: &[([u8; 20], [u8; 32], [u8; 32])],
        deleted: &[[u8; 20]],
    ) -> std::result::Result<(), String> {
        BlockchainDB::flush_evm_state(self, accounts, storage, deleted)
            .map_err(|e| e.to_string())
    }
}
