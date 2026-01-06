use luxtensor_core::types::{Address, Hash};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("Invalid validator")]
    InvalidValidator,
    
    #[error("Invalid block producer: expected {expected}, got {actual}")]
    InvalidProducer { expected: Address, actual: Address },
    
    #[error("Validator selection failed: {0}")]
    ValidatorSelection(String),
    
    #[error("Validator management error: {0}")]
    ValidatorManagement(String),
    
    #[error("Insufficient stake: provided {provided}, required {required}")]
    InsufficientStake { provided: u128, required: u128 },
    
    #[error("Reward distribution failed: {0}")]
    RewardDistribution(String),
    
    #[error("Block not found: {0:?}")]
    BlockNotFound(Hash),
    
    #[error("Duplicate block: {0:?}")]
    DuplicateBlock(Hash),
    
    #[error("Orphan block {block:?} with missing parent {parent:?}")]
    OrphanBlock { block: Hash, parent: Hash },
    
    #[error("Fork choice error: {0}")]
    ForkChoice(String),
    
    #[error("AI validation error: {0}")]
    AIValidation(String),
    
    #[error("Consensus error: {0}")]
    General(String),

    #[error("Validator not found: {0:?}")]
    ValidatorNotFound(Address),

    #[error("Validator already exists: {0:?}")]
    ValidatorAlreadyExists(Address),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("No validators available")]
    NoValidatorsAvailable,

    #[error("Epoch transition error: {0}")]
    EpochTransition(String),
}

pub type Result<T> = std::result::Result<T, ConsensusError>;
