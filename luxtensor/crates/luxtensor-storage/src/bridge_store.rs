// ─── RocksDB Bridge Store ────────────────────────────────────────────
//
// Persistent implementation of `BridgeStore` backed by RocksDB column
// families, sharing the same DB instance as `BlockchainDB`.
//
// Column families:
//   CF_BRIDGE_MESSAGES — key: Hash (32 bytes), value: bincode(BridgeMessage)
//   CF_BRIDGE_NONCES   — key: nonce name (utf-8), value: u64 (8 bytes LE)

use luxtensor_core::bridge::{
    BridgeError, BridgeMessage, BridgeMessageStatus, BridgeStore, Result as BridgeResult,
};
use luxtensor_core::types::Hash;
use rocksdb::DB;
use std::sync::Arc;
use tracing::debug;

/// Column family name for bridge messages.
pub const CF_BRIDGE_MESSAGES: &str = "bridge_messages";
/// Column family name for bridge nonces.
pub const CF_BRIDGE_NONCES: &str = "bridge_nonces";

/// RocksDB-backed bridge store.
///
/// Shares the same `Arc<DB>` as `BlockchainDB` via `inner_db()`, so no
/// extra RocksDB instance is created.  All reads/writes go to dedicated
/// column families (`bridge_messages`, `bridge_nonces`).
pub struct RocksDBBridgeStore {
    db: Arc<DB>,
}

impl RocksDBBridgeStore {
    /// Create a new `RocksDBBridgeStore` using an existing RocksDB handle.
    ///
    /// The database MUST have been opened with the `bridge_messages` and
    /// `bridge_nonces` column families already defined (see `BlockchainDB::open`).
    pub fn new(db: Arc<DB>) -> Self {
        debug!("RocksDBBridgeStore: using shared RocksDB instance");
        Self { db }
    }

    #[inline]
    fn cf_messages(&self) -> std::result::Result<&rocksdb::ColumnFamily, BridgeError> {
        self.db
            .cf_handle(CF_BRIDGE_MESSAGES)
            .ok_or_else(|| BridgeError::StoreError("CF bridge_messages not found".into()))
    }

    #[inline]
    fn cf_nonces(&self) -> std::result::Result<&rocksdb::ColumnFamily, BridgeError> {
        self.db
            .cf_handle(CF_BRIDGE_NONCES)
            .ok_or_else(|| BridgeError::StoreError("CF bridge_nonces not found".into()))
    }
}

impl BridgeStore for RocksDBBridgeStore {
    fn get_message(&self, hash: &Hash) -> BridgeResult<Option<BridgeMessage>> {
        let cf = self.cf_messages()?;
        match self.db.get_cf(cf, hash) {
            Ok(Some(bytes)) => {
                let msg: BridgeMessage = bincode::deserialize(&bytes)
                    .map_err(|e| BridgeError::SerializationError(e.to_string()))?;
                Ok(Some(msg))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(BridgeError::StoreError(e.to_string())),
        }
    }

    fn put_message(&self, hash: &Hash, msg: &BridgeMessage) -> BridgeResult<()> {
        let cf = self.cf_messages()?;
        let bytes = bincode::serialize(msg)
            .map_err(|e| BridgeError::SerializationError(e.to_string()))?;
        self.db
            .put_cf(cf, hash, bytes)
            .map_err(|e| BridgeError::StoreError(e.to_string()))
    }

    fn get_nonce(&self, key: &str) -> BridgeResult<u64> {
        let cf = self.cf_nonces()?;
        match self.db.get_cf(cf, key.as_bytes()) {
            Ok(Some(bytes)) if bytes.len() >= 8 => {
                let val = u64::from_le_bytes(
                    bytes[..8]
                        .try_into()
                        .map_err(|_| BridgeError::StoreError("invalid nonce bytes".into()))?,
                );
                Ok(val)
            }
            Ok(_) => Ok(1), // Default nonce = 1 (matches InMemoryBridgeStore)
            Err(e) => Err(BridgeError::StoreError(e.to_string())),
        }
    }

    fn put_nonce(&self, key: &str, val: u64) -> BridgeResult<()> {
        let cf = self.cf_nonces()?;
        self.db
            .put_cf(cf, key.as_bytes(), val.to_le_bytes())
            .map_err(|e| BridgeError::StoreError(e.to_string()))
    }

    fn list_by_status(&self, status: BridgeMessageStatus) -> BridgeResult<Vec<BridgeMessage>> {
        let cf = self.cf_messages()?;
        let mut result = Vec::new();
        let iter = self
            .db
            .iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_key, value) = item.map_err(|e| BridgeError::StoreError(e.to_string()))?;
            let msg: BridgeMessage = bincode::deserialize(&value)
                .map_err(|e| BridgeError::SerializationError(e.to_string()))?;
            if msg.status == status {
                result.push(msg);
            }
        }
        Ok(result)
    }

    fn list_all(&self) -> BridgeResult<Vec<BridgeMessage>> {
        let cf = self.cf_messages()?;
        let mut result = Vec::new();
        let iter = self
            .db
            .iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_key, value) = item.map_err(|e| BridgeError::StoreError(e.to_string()))?;
            let msg: BridgeMessage = bincode::deserialize(&value)
                .map_err(|e| BridgeError::SerializationError(e.to_string()))?;
            result.push(msg);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::bridge::{BridgeDirection, ChainId};
    use tempfile::TempDir;

    fn open_test_db() -> (Arc<DB>, TempDir) {
        let dir = TempDir::new().unwrap();
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let cfs = vec![
            rocksdb::ColumnFamilyDescriptor::new(CF_BRIDGE_MESSAGES, rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new(CF_BRIDGE_NONCES, rocksdb::Options::default()),
        ];
        let db = DB::open_cf_descriptors(&opts, dir.path(), cfs).unwrap();
        (Arc::new(db), dir)
    }

    fn sample_msg(hash_byte: u8, status: BridgeMessageStatus) -> BridgeMessage {
        let mut h = [0u8; 32];
        h[0] = hash_byte;
        BridgeMessage {
            message_hash: h,
            nonce: hash_byte as u64,
            direction: BridgeDirection::Outbound,
            source_chain: ChainId::LuxTensorMainnet,
            target_chain: ChainId::Ethereum,
            sender: [1u8; 20].into(),
            recipient: [2u8; 20].into(),
            amount: 1_000_000,
            data: vec![],
            source_block: 100,
            source_timestamp: 1700000000,
            source_state_root: [0u8; 32],
            status,
        }
    }

    #[test]
    fn test_put_get_message() {
        let (db, _dir) = open_test_db();
        let store = RocksDBBridgeStore::new(db);
        let msg = sample_msg(1, BridgeMessageStatus::Pending);
        store.put_message(&msg.message_hash, &msg).unwrap();
        let got = store.get_message(&msg.message_hash).unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap().nonce, 1);
    }

    #[test]
    fn test_get_missing() {
        let (db, _dir) = open_test_db();
        let store = RocksDBBridgeStore::new(db);
        let hash = [99u8; 32];
        assert!(store.get_message(&hash).unwrap().is_none());
    }

    #[test]
    fn test_nonce_roundtrip() {
        let (db, _dir) = open_test_db();
        let store = RocksDBBridgeStore::new(db);
        // default
        assert_eq!(store.get_nonce("outbound").unwrap(), 1);
        store.put_nonce("outbound", 42).unwrap();
        assert_eq!(store.get_nonce("outbound").unwrap(), 42);
    }

    #[test]
    fn test_list_all_and_by_status() {
        let (db, _dir) = open_test_db();
        let store = RocksDBBridgeStore::new(db);
        let m1 = sample_msg(1, BridgeMessageStatus::Pending);
        let m2 = sample_msg(2, BridgeMessageStatus::Executed);
        let m3 = sample_msg(3, BridgeMessageStatus::Pending);
        store.put_message(&m1.message_hash, &m1).unwrap();
        store.put_message(&m2.message_hash, &m2).unwrap();
        store.put_message(&m3.message_hash, &m3).unwrap();

        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 3);

        let pending = store.list_by_status(BridgeMessageStatus::Pending).unwrap();
        assert_eq!(pending.len(), 2);

        let executed = store.list_by_status(BridgeMessageStatus::Executed).unwrap();
        assert_eq!(executed.len(), 1);
    }
}
