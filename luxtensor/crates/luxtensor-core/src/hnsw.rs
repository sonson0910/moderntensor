//! HNSW (Hierarchical Navigable Small World) Index
//!
//! Re-exports from the consensus-safe `luxtensor-hnsw` crate, which uses
//! fixed-point arithmetic (I64F32) for deterministic distance calculations
//! across all validator hardware (x86, ARM, etc).
//!
//! # Why re-export through `luxtensor-core`?
//!
//! `luxtensor-core` is the shared type/API crate that every other LuxTensor
//! crate depends on.  Re-exporting the key HNSW types here means:
//!
//! - **Single import path**: consumers write `use luxtensor_core::hnsw::*`
//!   instead of taking a direct dependency on `luxtensor-hnsw`.
//! - **Version alignment**: the core crate pins the exact `luxtensor-hnsw`
//!   version, preventing diamond-dependency conflicts across the workspace.
//! - **Semantic coupling**: `UnifiedStateDB` embeds an `HnswVectorStore`
//!   field, so the type *must* be visible at the core level.
//!
//! The actual implementation lives entirely in `luxtensor-hnsw`.  This file
//! is intentionally thin — it is a **façade**, not a re-implementation.
//!
//! # Provided Types
//!
//! - [`HnswVectorStore`] — Production vector store with capacity limits,
//!   TTL, and AI primitives (insert, search, delete).
//! - [`HnswError`]       — Error types for HNSW operations.
//!
//! # Consensus Safety
//!
//! The underlying implementation enforces the **Determinism Trilemma**:
//! 1. **Fixed-Point Arithmetic**: All distances use `I64F32` (no f32 rounding divergence)
//! 2. **Cryptographic Seeding**: `DeterministicRng` seeded by consensus artifacts
//! 3. **Canonical Insertion Order**: Vectors inserted serially by transaction index

pub use luxtensor_hnsw::vector_store::HnswVectorStore;
pub use luxtensor_hnsw::error::HnswError;
