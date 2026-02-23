use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Block not found: {0:?}")]
    BlockNotFound([u8; 32]),

    #[error("Transaction not found: {0:?}")]
    TransactionNotFound([u8; 32]),

    #[error("Account not found: {0:?}")]
    AccountNotFound([u8; 20]),

    #[error("Invalid trie node")]
    InvalidTrieNode,

    #[error("Invalid proof")]
    InvalidProof,

    #[error("Schema version mismatch: database has version {found}, but expected version {expected}. A migration is required.")]
    SchemaMismatch { found: u32, expected: u32 },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<rocksdb::Error> for StorageError {
    fn from(err: rocksdb::Error) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

impl From<bincode::Error> for StorageError {
    fn from(err: bincode::Error) -> Self {
        StorageError::SerializationError(err.to_string())
    }
}

impl From<luxtensor_core::CoreError> for StorageError {
    fn from(err: luxtensor_core::CoreError) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, StorageError>;
