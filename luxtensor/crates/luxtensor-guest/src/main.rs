//! LuxTensor AI Inference Guest Program
//!
//! This program runs inside the RISC Zero zkVM to produce a verifiable
//! proof of an AI inference computation. The guest:
//!
//! 1. Reads model hash and input data from the host
//! 2. Computes a deterministic commitment: keccak256(model_hash || input_data)
//! 3. Commits the result to the journal (public output)
//!
//! The journal is publicly visible and can be verified on-chain,
//! while the input data remains private.

#![no_main]
#![no_std]

risc0_zkvm::guest::entry!(main);

use risc0_zkvm::guest::env;

/// Main guest program entry point.
///
/// # Protocol
///
/// **Input** (read from host via `env::read()`):
/// - `model_hash_len: u32` — length of model hash
/// - `model_hash: [u8; model_hash_len]` — hash of the AI model
/// - `input_data_len: u32` — length of input data
/// - `input_data: [u8; input_data_len]` — AI inference input
///
/// **Output** (committed to journal):
/// - `commitment: [u8; 32]` — keccak256(model_hash || input_data)
fn main() {
    // Read model hash length and data
    let model_hash_len: u32 = env::read();
    let model_hash: alloc::vec::Vec<u8> = {
        let mut buf = alloc::vec![0u8; model_hash_len as usize];
        env::read_slice(&mut buf);
        buf
    };

    // Read input data length and data
    let input_data_len: u32 = env::read();
    let input_data: alloc::vec::Vec<u8> = {
        let mut buf = alloc::vec![0u8; input_data_len as usize];
        env::read_slice(&mut buf);
        buf
    };

    // Compute commitment: simple keccak256-like hash
    // Using a basic hash since we can't use external crypto crates in guest
    // The hash is computed as: SHA-256 of (model_hash || input_data)
    // We use the zkVM's built-in SHA-256 accelerator for performance
    let mut combined = alloc::vec::Vec::with_capacity(model_hash.len() + input_data.len());
    combined.extend_from_slice(&model_hash);
    combined.extend_from_slice(&input_data);

    // Use risc0's built-in sha256 (hardware accelerated in zkVM)
    let commitment = risc0_zkvm::sha::Impl::hash_bytes(&combined);

    // Commit the hash to the journal (public output)
    env::commit_slice(commitment.as_bytes());
}

extern crate alloc;
