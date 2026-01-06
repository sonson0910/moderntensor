// LuxTensor consensus module
// Phase 2: Consensus Layer Implementation

pub mod error;
pub mod validator;
pub mod pos;
pub mod fork_choice;
pub mod rotation;
pub mod fork_resolution;
pub mod fast_finality;

pub use error::*;
pub use validator::{Validator, ValidatorSet};
pub use pos::{ProofOfStake, ConsensusConfig};
pub use fork_choice::ForkChoice;
pub use rotation::{ValidatorRotation, RotationConfig, RotationStats, EpochTransitionResult};
pub use fork_resolution::{ForkResolver, ReorgInfo, FinalityStatus, FinalityStats};
pub use fast_finality::{FastFinality, FastFinalityStats};
