//! LuxTensor RISC Zero Methods
//!
//! This crate provides the compiled guest program ELF binary and image ID
//! for use by the host-side prover and verifier.
//!
//! # Generated Constants
//!
//! The `include!` macro below pulls in two constants generated at build time
//! by the `risc0-build` crate:
//!
//! - `LUXTENSOR_GUEST_ELF: &[u8]` — The compiled RISC-V ELF binary of the
//!   guest program (`luxtensor-guest`).
//! - `LUXTENSOR_GUEST_ID: [u32; 8]` — The image ID (Merkle root of the ELF
//!   memory image) used for on-chain verification of zkVM proofs.
//!
//! # Usage
//!
//! ```rust,ignore
//! use luxtensor_methods::{LUXTENSOR_GUEST_ELF, LUXTENSOR_GUEST_ID};
//!
//! // Host-side: generate a proof
//! let receipt = prover.prove(LUXTENSOR_GUEST_ELF, input)?;
//!
//! // Verifier-side: verify against the known image ID
//! receipt.verify(LUXTENSOR_GUEST_ID)?;
//! ```
//!
//! # Build Requirements
//!
//! This crate requires:
//! - `risc0-build` in `[build-dependencies]`
//! - A `build.rs` that calls `risc0_build::embed_methods()`
//! - The `luxtensor-guest` crate as the guest program source

include!(concat!(env!("OUT_DIR"), "/methods.rs"));
