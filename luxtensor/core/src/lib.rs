// Core blockchain primitives for LuxTensor
// 
// This module provides the fundamental building blocks for the blockchain:
// - Block structures and headers
// - Transaction types and validation
// - State management and accounts
// - Cryptographic primitives
// - Validation rules

pub mod block;
pub mod transaction;
pub mod state;
pub mod crypto;
pub mod validation;
pub mod types;
pub mod errors;

pub use block::{Block, BlockHeader};
pub use transaction::{Transaction, TransactionReceipt};
pub use state::{Account, StateDB};
pub use crypto::{KeyPair, MerkleTree};
pub use validation::BlockValidator;
