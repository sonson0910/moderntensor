use crate::{Hash, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// A simple, pure-Rust implementation of VectorStore (Brute-force)
/// Used for initial prototype and fallback.
/// In production, this will be replaced by HNSW or DiskANN.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimpleVectorStore {
    pub vectors: HashMap<u64, Vec<f32>>,
    pub dimension: usize,
}

impl Default for SimpleVectorStore {
    fn default() -> Self {
        Self::new(768)
    }
}

impl SimpleVectorStore {
    pub fn new(dimension: usize) -> Self {
        Self {
            vectors: HashMap::new(),
            dimension,
        }
    }

    /// Calculate Euclidean distance squared (avoid sqrt for speed)
    fn dist_sq(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum()
    }
}

impl VectorStore for SimpleVectorStore {
    fn insert(&mut self, id: u64, vector: Vec<f32>) -> Result<()> {
        if vector.len() != self.dimension {
            return Err(crate::CoreError::InvalidVectorDimension(
                self.dimension,
                vector.len(),
            ));
        }
        self.vectors.insert(id, vector);
        Ok(())
    }

    fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>> {
        if query.len() != self.dimension {
            return Err(crate::CoreError::InvalidVectorDimension(
                self.dimension,
                query.len(),
            ));
        }

        let mut distances: Vec<(u64, f32)> = self.vectors
            .iter()
            .map(|(id, vec)| (*id, Self::dist_sq(query, vec)))
            .collect();

        // Sort by distance (ascending)
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        Ok(distances.into_iter().take(k).collect())
    }

    fn root_hash(&self) -> Result<Hash> {
        if self.vectors.is_empty() {
            return Ok([0u8; 32]);
        }

        // Sort keys for deterministic hash
        let mut keys: Vec<_> = self.vectors.keys().collect();
        keys.sort();

        // Simple hash accumulation for prototype
        // In prod, use a proper Merkle Tree over vector commitments
        let mut data = Vec::new();
        for key in keys {
            data.extend_from_slice(&key.to_le_bytes());
            let vec = &self.vectors[key];
            for val in vec {
                data.extend_from_slice(&val.to_le_bytes());
            }
        }

        Ok(luxtensor_crypto::keccak256(&data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_insert_search() {
        let mut store = SimpleVectorStore::new(2);

        // Insert vectors: (0,0), (1,1), (5,5)
        store.insert(1, vec![0.0, 0.0]).unwrap();
        store.insert(2, vec![1.0, 1.0]).unwrap();
        store.insert(3, vec![5.0, 5.0]).unwrap();

        // Search for (0.1, 0.1) -> Should match (0,0) then (1,1)
        let results = store.search(&[0.1, 0.1], 2).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 1); // ID 1 is closest directly
        assert_eq!(results[1].0, 2); // ID 2 is next
    }
}
