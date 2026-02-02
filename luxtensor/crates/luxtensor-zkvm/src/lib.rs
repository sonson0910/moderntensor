//! LuxTensor Zero-Knowledge Virtual Machine Integration
//!
//! This crate provides zero-knowledge proof capabilities for the LuxTensor blockchain,
//! enabling verifiable computation, privacy-preserving AI inference, and scalable validation.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │                   ZKML Layer                     │
//! │  (Neural networks, Model verification)          │
//! ├─────────────────────────────────────────────────┤
//! │               RISC Zero zkVM                     │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
//! │  │  Guest   │  │  Prover  │  │ Verifier │       │
//! │  └──────────┘  └──────────┘  └──────────┘       │
//! ├─────────────────────────────────────────────────┤
//! │              LuxTensor L1                        │
//! └─────────────────────────────────────────────────┘
//! ```
//!
//! # Features
//!
//! - `prove` - Enable proof generation (default)
//! - `cuda` - GPU acceleration via CUDA
//! - `metal` - GPU acceleration via Metal (macOS)
//! - `groth16` - SNARK wrapping for smaller proofs

pub mod error;
pub mod types;
pub mod prover;
pub mod verifier;
pub mod guest;
pub mod pot_verifier;

pub use error::{ZkVmError, Result};
pub use types::{
    Proof, ProofReceipt, GuestInput, GuestOutput,
    ProverConfig, VerificationResult, ImageId,
};
pub use prover::{ZkProver, ProverStats};
pub use verifier::{ZkVerifier, VerifierConfig};
