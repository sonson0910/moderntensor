//! LuxTensor RISC Zero Methods
//!
//! This crate provides the compiled guest program ELF binary and image ID
//! for use by the host-side prover and verifier.
//!
//! # Generated Constants
//!
//! - `LUXTENSOR_GUEST_ELF` — The compiled RISC-V ELF binary of the guest program
//! - `LUXTENSOR_GUEST_ID` — The image ID (hash of the ELF) used for verification

include!(concat!(env!("OUT_DIR"), "/methods.rs"));
