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

    #[error("Validator not found: {0}")]
    ValidatorNotFound(String),

    #[error("Validator already exists: {0:?}")]
    ValidatorAlreadyExists(Address),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("No validators available")]
    NoValidatorsAvailable,

    #[error("Epoch transition error: {0}")]
    EpochTransition(String),

    #[error("Slashing failed: {0}")]
    SlashingFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    // ── Production VRF errors (active when `production-vrf` feature is enabled) ──

    /// VRF proof generation failed (e.g. bad key material).
    #[error("VRF proof generation failed: {0}")]
    VrfProofFailed(String),

    /// A received or locally-generated VRF proof is cryptographically invalid.
    /// Any peer that receives a block with an invalid VRF proof MUST reject it.
    #[error("VRF proof is invalid — block must be rejected")]
    VrfProofInvalid,

    /// The node tried to produce a block but no VRF secret key has been configured.
    /// Call `ProofOfStake::set_vrf_key()` during node startup.
    #[error("VRF secret key not configured — call set_vrf_key() before using production-vrf")]
    VrfKeyMissing,

    /// The supplied key bytes are not a valid Ed25519 scalar.
    #[error("Invalid VRF key bytes: {0}")]
    VrfKeyInvalid(String),
}

pub type Result<T> = std::result::Result<T, ConsensusError>;
