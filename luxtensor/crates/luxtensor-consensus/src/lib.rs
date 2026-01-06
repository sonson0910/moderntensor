// LuxTensor consensus module
// Phase 2: Consensus Layer Implementation

pub mod error;
pub mod validator;
pub mod pos;
pub mod fork_choice;

pub use error::*;
pub use validator::{Validator, ValidatorSet};
pub use pos::{ProofOfStake, ConsensusConfig};
pub use fork_choice::ForkChoice;
