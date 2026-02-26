//! HNSW (Hierarchical Navigable Small World) Index
//!
//! Re-exports from the consensus-safe `luxtensor-hnsw` crate, which uses
//! fixed-point arithmetic (I64F32) for deterministic distance calculations
//! across all validator hardware (x86, ARM, etc).
//!
//! This module provides:
//! - `HnswVectorStore` — Production vector store with capacity limits, TTL, AI primitives
//! - `HnswError` — Error types for HNSW operations
//!
//! ## Consensus Safety
//!
//! The underlying implementation enforces the **Determinism Trilemma**:
//! 1. **Fixed-Point Arithmetic**: All distances use `I64F32` (no f32 rounding divergence)
//! 2. **Cryptographic Seeding**: `DeterministicRng` seeded by consensus artifacts
//! 3. **Canonical Insertion Order**: Vectors inserted serially by transaction index

pub use luxtensor_hnsw::vector_store::HnswVectorStore;
pub use luxtensor_hnsw::error::HnswError;
