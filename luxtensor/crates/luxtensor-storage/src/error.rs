use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Storage error: {0}")]
    General(String),
    
    #[error("Database error")]
    DatabaseError,
}

pub type Result<T> = std::result::Result<T, StorageError>;
