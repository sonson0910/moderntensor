use crate::{Hash, Result};

/// Vector Store Interface
/// Defines how the blockchain interacts with high-dimensional data
pub trait VectorStore {
    /// Insert a vector with a unique ID
    fn insert(&mut self, id: u64, vector: Vec<f32>) -> Result<()>;

    /// Search for k-nearest neighbors
    /// Returns list of (id, distance) tuples
    fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>>;

    /// Calculate Merkle Root of the vector store for consensus verification
    fn root_hash(&self) -> Result<Hash>;
}

// SimpleVectorStore has been deprecated in favor of HnswVectorStore.
// Use crate::hnsw::HnswVectorStore for O(log N) approximate nearest neighbor search.
// See: luxtensor-core/src/hnsw.rs
