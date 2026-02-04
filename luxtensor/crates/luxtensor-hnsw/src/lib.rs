//! # LuxTensor HNSW
//!
//! Deterministic Hierarchical Navigable Small World (HNSW) implementation
//! designed for consensus-safe operation in blockchain environments.
//!
//! ## Key Features
//!
//! This implementation enforces the **Determinism Trilemma** to prevent consensus forks:
//!
//! 1. **Cryptographic Seeding**: PRNG seeded by `Keccak256(TxHash ⊕ BlockHash)`
//! 2. **Fixed-Point Arithmetic**: All distance calculations use `I64F32` (no floating-point)
//! 3. **Canonical Insertion Order**: Vectors inserted serially by block transaction index
//!
//! ## SIMD Acceleration
//!
//! Enable the `simd` feature for ~30% faster distance calculations (requires nightly):
//!
//! ```toml
//! luxtensor-hnsw = { version = "0.1", features = ["simd"] }
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use luxtensor_hnsw::{HnswGraph, FixedPointVector, DeterministicRng};
//!
//! // Create a deterministic RNG from consensus artifacts
//! let mut rng = DeterministicRng::new(tx_hash, block_hash);
//!
//! // Create a 768-dimensional HNSW graph
//! let mut graph: HnswGraph<768> = HnswGraph::new();
//!
//! // Insert vectors deterministically
//! let vector = FixedPointVector::from_f32_slice(&embedding);
//! graph.insert(vector, &mut rng);
//!
//! // Search for nearest neighbors
//! let results = graph.search(&query_vector, 10);
//! ```

// Enable portable_simd when simd feature is active (requires nightly)
#![cfg_attr(feature = "simd", feature(portable_simd))]

pub mod fixed_point;
pub mod deterministic_rng;
pub mod graph;
pub mod error;

#[cfg(feature = "simd")]
pub mod simd;

pub use fixed_point::FixedPointVector;
pub use deterministic_rng::DeterministicRng;
pub use graph::HnswGraph;
pub use error::{HnswError, Result};

/// Maximum number of connections per node at layer 0
pub const M0: usize = 32;

/// Maximum number of connections per node at higher layers
pub const M: usize = 16;

/// Calculate level multiplier for probability distribution
/// This is 1/ln(M) but cannot be const due to ln() not being const fn
#[inline]
pub fn ml() -> f64 {
    1.0 / (M as f64).ln()
}

/// Size of the dynamic candidate list during search
pub const EF_CONSTRUCTION: usize = 200;

/// Default dimension for embeddings (transformer standard)
pub const DEFAULT_DIM: usize = 768;
