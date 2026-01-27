// LuxTensor Tests Library
// This crate contains integration tests and benchmarks for LuxTensor

pub mod test_utils;
pub mod rpc_client;
pub mod node_manager;
pub mod network_security; // Peer banning, bootstrap failover
pub mod fuzz_targets; // Fuzzing targets for cargo-fuzz

// Unit test modules
#[cfg(test)]
mod core_tests;
#[cfg(test)]
mod crypto_tests;
#[cfg(test)]
mod stake_reward_tests;
#[cfg(test)]
mod edge_case_tests;
#[cfg(test)]
mod crypto_verification; // ECDSA, Keccak256, Merkle verification
#[cfg(test)]
mod consensus_security; // Validator selection, slashing, jail/unjail
