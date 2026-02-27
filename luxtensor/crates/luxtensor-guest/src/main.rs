//! LuxTensor AI Inference Guest Program
//!
//! This program runs inside the RISC Zero zkVM to produce a verifiable
//! proof of an AI inference computation. The guest:
//!
//! 1. Reads model hash and input data from the host
//! 2. Validates input sizes against [`MAX_MODEL_HASH_LEN`] and [`MAX_INPUT_DATA_LEN`]
//! 3. Computes a deterministic commitment: SHA-256(model_hash || input_data)
//! 4. Commits the result to the journal (public output)
//!
//! The journal is publicly visible and can be verified on-chain,
//! while the input data remains private.
//!
//! # Security Considerations
//!
//! - All inputs are length-checked to prevent OOM inside the zkVM.
//! - SHA-256 is used via the zkVM's hardware accelerator for performance.
//! - The commitment is deterministic: same inputs → same proof.
//!
//! # Protocol
//!
//! **Input** (read from host via `env::read()`):
//! - `model_hash_len: u32` — length of model hash (max 256 bytes)
//! - `model_hash: [u8; model_hash_len]` — hash of the AI model
//! - `input_data_len: u32` — length of input data (max 10 MB)
//! - `input_data: [u8; input_data_len]` — AI inference input
//!
//! **Output** (committed to journal):
//! - `commitment: [u8; 32]` — SHA-256(model_hash || input_data)

#![no_main]
#![no_std]

risc0_zkvm::guest::entry!(main);

use risc0_zkvm::guest::env;

/// Maximum allowed model hash length (256 bytes).
///
/// Model hashes are typically 32 bytes (SHA-256) or 64 bytes (SHA-512).
/// The generous 256-byte limit accommodates future multi-hash schemes.
const MAX_MODEL_HASH_LEN: u32 = 256;

/// Maximum allowed input data length (10 MB).
///
/// This bounds the guest memory usage and ensures the zkVM proof
/// generation completes within reasonable time/memory constraints.
const MAX_INPUT_DATA_LEN: u32 = 10 * 1024 * 1024;

fn main() {
    // Read and validate model hash
    let model_hash_len: u32 = env::read();
    assert!(
        model_hash_len <= MAX_MODEL_HASH_LEN,
        "model_hash_len exceeds maximum ({} > {})",
        model_hash_len,
        MAX_MODEL_HASH_LEN,
    );
    let model_hash: alloc::vec::Vec<u8> = {
        let mut buf = alloc::vec![0u8; model_hash_len as usize];
        env::read_slice(&mut buf);
        buf
    };

    // Read and validate input data
    let input_data_len: u32 = env::read();
    assert!(
        input_data_len <= MAX_INPUT_DATA_LEN,
        "input_data_len exceeds maximum ({} > {})",
        input_data_len,
        MAX_INPUT_DATA_LEN,
    );
    let input_data: alloc::vec::Vec<u8> = {
        let mut buf = alloc::vec![0u8; input_data_len as usize];
        env::read_slice(&mut buf);
        buf
    };

    // Compute commitment: SHA-256(model_hash || input_data)
    // Uses risc0's built-in SHA-256 (hardware accelerated in zkVM)
    let mut combined = alloc::vec::Vec::with_capacity(model_hash.len() + input_data.len());
    combined.extend_from_slice(&model_hash);
    combined.extend_from_slice(&input_data);

    let commitment = risc0_zkvm::sha::Impl::hash_bytes(&combined);

    // Commit the hash to the journal (public output)
    env::commit_slice(commitment.as_bytes());
}

extern crate alloc;
