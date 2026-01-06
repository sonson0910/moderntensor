use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("RPC error: {0}")]
    General(String),
}

pub type Result<T> = std::result::Result<T, RpcError>;
