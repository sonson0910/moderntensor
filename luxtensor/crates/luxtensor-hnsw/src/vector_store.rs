//! Production-ready HNSW Vector Store
//!
//! Provides `HnswVectorStore` — a drop-in replacement for `core::hnsw::HnswVectorStore`
//! that uses consensus-safe fixed-point arithmetic internally while maintaining
//! the same `Vec<f32>` public API.
//!
//! ## Architecture
//!
//! ```text
//! Vec<f32> input → FixedPointVector<768> → HnswGraph<768> (I64F32 distances)
//!                                          ↓
//! Vec<(u64, f32)> output ← I64F32::to_num::<f32>() conversion
//! ```
//!
//! All distance calculations happen in fixed-point, ensuring consensus safety.

use std::collections::HashMap;

use crate::deterministic_rng::DeterministicRng;
use crate::error::HnswError;
use crate::fixed_point::FixedPointVector;
use crate::graph::HnswGraph;

/// Standard embedding dimension (BERT/LLM transformers)
const STANDARD_DIM: usize = 768;

/// HNSW-backed vector store for precompile integration.
///
/// Wraps `HnswGraph<768>` with production features:
/// - `Vec<f32>` public API (converts to fixed-point internally)
/// - Capacity limits and TTL-based pruning
/// - AI primitives (classify, anomaly detection, similarity)
/// - Merkle root hash for consensus verification
/// - Deterministic serialization via bincode
pub struct HnswVectorStore {
    /// The underlying deterministic HNSW graph
    graph: HnswGraph<STANDARD_DIM>,
    /// RNG for deterministic node level assignment
    rng: DeterministicRng,
    /// Mapping from user-facing u64 IDs to internal graph node indices
    id_to_index: HashMap<u64, usize>,
    /// Mapping from internal graph node indices to user-facing u64 IDs
    index_to_id: HashMap<usize, u64>,
    /// Original f32 vectors (kept for get_vector() and root_hash())
    vectors_f32: HashMap<u64, Vec<f32>>,
    /// Maximum number of vectors allowed in this store
    max_capacity: usize,
    /// Tracks insertion block for each vector ID (for TTL-based pruning)
    timestamps: HashMap<u64, u64>,
    /// Vector dimension (always 768 for production)
    dimension: usize,
}

impl HnswVectorStore {
    /// Default capacity limit per store (prevents unbounded growth).
    /// 10K vectors × 768 dims × 4 bytes ≈ 30MB — manageable for blockchain state.
    pub const DEFAULT_MAX_CAPACITY: usize = 10_000;

    /// Create a new vector store with the given dimension.
    ///
    /// # Panics
    /// Currently only supports dimension 768. Other dimensions will be
    /// accepted but vectors must still be 768-dimensional.
    pub fn new(dimension: usize) -> Self {
        Self {
            graph: HnswGraph::new(),
            rng: DeterministicRng::from_seed([0u8; 32]),
            id_to_index: HashMap::new(),
            index_to_id: HashMap::new(),
            vectors_f32: HashMap::new(),
            max_capacity: Self::DEFAULT_MAX_CAPACITY,
            timestamps: HashMap::new(),
            dimension,
        }
    }

    /// Create a store with a custom capacity limit.
    pub fn with_capacity(dimension: usize, max_capacity: usize) -> Self {
        Self {
            graph: HnswGraph::new(),
            rng: DeterministicRng::from_seed([0u8; 32]),
            id_to_index: HashMap::new(),
            index_to_id: HashMap::new(),
            vectors_f32: HashMap::new(),
            max_capacity,
            timestamps: HashMap::new(),
            dimension,
        }
    }

    /// Insert a vector into the store.
    pub fn insert(&mut self, id: u64, vector: Vec<f32>) -> Result<(), HnswError> {
        if vector.len() != self.dimension {
            return Err(HnswError::DimensionMismatch {
                expected: self.dimension,
                actual: vector.len(),
            });
        }

        // Reject NaN / Infinity vectors
        if vector.iter().any(|v| !v.is_finite()) {
            return Err(HnswError::InvalidData);
        }

        if self.id_to_index.len() >= self.max_capacity {
            return Err(HnswError::CapacityExceeded);
        }

        if self.id_to_index.contains_key(&id) {
            return Err(HnswError::DuplicateId(id));
        }

        // Convert f32 to fixed-point for deterministic graph operations
        // Pad or truncate to STANDARD_DIM if dimension differs
        let fixed_vec = self.f32_to_fixed(&vector)?;

        // Insert into the deterministic graph
        let node_index = self.graph.insert(fixed_vec, &mut self.rng)?;

        // Track mappings
        self.id_to_index.insert(id, node_index);
        self.index_to_id.insert(node_index, id);
        self.vectors_f32.insert(id, vector);

        Ok(())
    }

    /// Insert a vector with block-height tracking for TTL pruning.
    pub fn insert_with_block(
        &mut self,
        id: u64,
        vector: Vec<f32>,
        block: u64,
    ) -> Result<(), HnswError> {
        self.insert(id, vector)?;
        self.timestamps.insert(id, block);
        Ok(())
    }

    /// Remove a single vector by ID.
    ///
    /// Note: The underlying HnswGraph uses append-only storage, so removal
    /// only clears the mapping and f32 data. The fixed-point node remains
    /// in the graph but becomes unreachable from search results.
    pub fn remove(&mut self, id: u64) -> bool {
        if let Some(_index) = self.id_to_index.remove(&id) {
            self.index_to_id.remove(&_index);
            self.vectors_f32.remove(&id);
            self.timestamps.remove(&id);
            true
        } else {
            false
        }
    }

    /// Prune all vectors inserted before `cutoff_block`.
    ///
    /// Returns the number of vectors removed.
    pub fn prune_before(&mut self, cutoff_block: u64) -> usize {
        let expired_ids: Vec<u64> = self
            .timestamps
            .iter()
            .filter(|&(_, &block)| block < cutoff_block)
            .map(|(&id, _)| id)
            .collect();

        let count = expired_ids.len();
        for id in expired_ids {
            self.remove(id);
        }
        count
    }

    /// Search for the k nearest neighbors of a query vector.
    ///
    /// Returns `Vec<(id, distance)>` sorted by distance (ascending).
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>, HnswError> {
        if query.len() != self.dimension {
            return Err(HnswError::DimensionMismatch {
                expected: self.dimension,
                actual: query.len(),
            });
        }

        if self.graph.is_empty() {
            return Ok(Vec::new());
        }

        // Convert query to fixed-point
        let fixed_query = self.f32_to_fixed(query)?;

        // Search with ef = max(k, 64) for good recall
        let ef = k.max(64);
        let results = self.graph.search(&fixed_query, k, ef)?;

        // Convert results: map internal indices back to user IDs, distances to f32
        let mut mapped_results: Vec<(u64, f32)> = results
            .into_iter()
            .filter_map(|(index, dist)| {
                self.index_to_id.get(&index).map(|&id| {
                    // Only return results that haven't been "removed"
                    if self.id_to_index.contains_key(&id) {
                        Some((id, dist.to_num::<f32>()))
                    } else {
                        None
                    }
                })
            })
            .flatten()
            .collect();

        mapped_results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        mapped_results.truncate(k);

        Ok(mapped_results)
    }

    /// Get the number of vectors in the store.
    pub fn len(&self) -> usize {
        self.id_to_index.len()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.id_to_index.is_empty()
    }

    /// Get current capacity limit.
    pub fn capacity(&self) -> usize {
        self.max_capacity
    }

    /// Get the dimension of vectors in this store.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Get a vector by ID for cross-contract composability.
    pub fn get_vector(&self, id: u64) -> Option<Vec<f32>> {
        self.vectors_f32.get(&id).cloned()
    }

    // ==================== AI Primitives ====================

    /// Classify a vector against labeled reference vectors.
    /// Returns the label of the closest match and confidence score.
    ///
    /// # Arguments
    /// * `query` - The vector to classify
    /// * `labels` - List of (vector_id, label) pairs representing categories
    ///
    /// # Returns
    /// * `(label, confidence)` where confidence is 1.0 - normalized_distance
    pub fn classify(&self, query: &[f32], labels: &[(u64, u32)]) -> Result<(u32, f32), HnswError> {
        if labels.is_empty() {
            return Err(HnswError::InvalidData);
        }

        let results = self.search(query, 1)?;

        if results.is_empty() {
            return Err(HnswError::InvalidData);
        }

        let (nearest_id, distance) = results[0];

        let label = labels
            .iter()
            .find(|(id, _)| *id == nearest_id)
            .map(|(_, l)| *l)
            .unwrap_or(0);

        // Convert distance to confidence (1.0 = exact match, 0.0 = very far)
        let confidence = (-distance.sqrt()).exp().clamp(0.0, 1.0);

        Ok((label, confidence))
    }

    /// Calculate anomaly score for a vector relative to stored vectors.
    /// Higher score means more anomalous (further from all stored vectors).
    ///
    /// # Returns
    /// * Score in range [0.0, 1.0] where 1.0 = highly anomalous
    pub fn anomaly_score(&self, query: &[f32]) -> Result<f32, HnswError> {
        if self.is_empty() {
            return Ok(1.0);
        }

        let k = 5.min(self.len());
        let results = self.search(query, k)?;

        if results.is_empty() {
            return Ok(1.0);
        }

        let avg_distance: f32 = results.iter().map(|(_, d)| d).sum::<f32>() / results.len() as f32;

        let threshold = 2.0;
        let score = 1.0 / (1.0 + (-((avg_distance / threshold) - 1.0)).exp());

        Ok(score.clamp(0.0, 1.0))
    }

    /// Check if two vectors are semantically similar above a threshold.
    ///
    /// # Returns
    /// * `(is_similar, similarity_score)`
    pub fn similarity_check(
        &self,
        vector_a: &[f32],
        vector_b: &[f32],
        threshold: f32,
    ) -> Result<(bool, f32), HnswError> {
        if vector_a.len() != self.dimension || vector_b.len() != self.dimension {
            return Err(HnswError::DimensionMismatch {
                expected: self.dimension,
                actual: vector_a.len().min(vector_b.len()),
            });
        }

        // Use fixed-point for deterministic distance
        let fixed_a = self.f32_to_fixed(vector_a)?;
        let fixed_b = self.f32_to_fixed(vector_b)?;
        let distance: f32 = fixed_a.squared_distance(&fixed_b).to_num::<f32>();

        // Convert distance to similarity (1.0 = identical, 0.0 = very different)
        let similarity = (-distance.sqrt() / 2.0).exp();
        let is_similar = similarity >= threshold;

        Ok((is_similar, similarity))
    }

    // ==================== Consensus ====================

    /// Calculate Merkle root hash for consensus verification.
    /// Produces a deterministic 32-byte hash from all stored vectors.
    pub fn root_hash(&self) -> [u8; 32] {
        if self.is_empty() {
            return [0u8; 32];
        }

        // Collect and sort IDs for deterministic ordering
        let mut ids: Vec<u64> = self.vectors_f32.keys().copied().collect();
        ids.sort();

        // Hash all vector data in sorted order
        let mut data = Vec::new();
        for id in ids {
            data.extend_from_slice(&id.to_le_bytes());
            if let Some(vector) = self.vectors_f32.get(&id) {
                for val in vector {
                    data.extend_from_slice(&val.to_le_bytes());
                }
            }
        }

        luxtensor_crypto::keccak256(&data)
    }

    // ==================== Serialization ====================

    /// Serialize the store to bytes for persistence.
    pub fn to_bytes(&self) -> Vec<u8> {
        // Serialize the f32 vectors + metadata for backward compatibility
        let mut bytes = Vec::new();

        // Header
        bytes.extend_from_slice(&(self.dimension as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.id_to_index.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.max_capacity as u64).to_le_bytes());

        // Sort by ID for deterministic serialization
        let mut entries: Vec<_> = self.vectors_f32.iter().collect();
        entries.sort_by_key(|(id, _)| *id);

        for (&id, vector) in &entries {
            bytes.extend_from_slice(&id.to_le_bytes());

            // Write vector
            for &v in vector.iter() {
                bytes.extend_from_slice(&v.to_le_bytes());
            }

            // Write timestamp if present
            let ts = self.timestamps.get(&id).copied().unwrap_or(0);
            bytes.extend_from_slice(&ts.to_le_bytes());
        }

        bytes
    }

    /// Deserialize a store from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HnswError> {
        // Header: dimension(4) + count(8) + max_capacity(8) = 20 bytes
        if bytes.len() < 20 {
            return Err(HnswError::InvalidData);
        }

        let mut pos = 0;

        let dimension = u32::from_le_bytes(
            bytes[pos..pos + 4]
                .try_into()
                .map_err(|_| HnswError::InvalidData)?,
        ) as usize;
        pos += 4;

        let count = u64::from_le_bytes(
            bytes[pos..pos + 8]
                .try_into()
                .map_err(|_| HnswError::InvalidData)?,
        ) as usize;
        pos += 8;

        let max_capacity = u64::from_le_bytes(
            bytes[pos..pos + 8]
                .try_into()
                .map_err(|_| HnswError::InvalidData)?,
        ) as usize;
        pos += 8;

        // Validation
        const MAX_DIMENSION: usize = 65_536;
        const MAX_COUNT: usize = 10_000_000;
        if dimension > MAX_DIMENSION || count > MAX_COUNT || dimension == 0 {
            return Err(HnswError::InvalidData);
        }

        // Each entry: id(8) + dimension*4 (vector) + timestamp(8)
        let entry_size = 8 + dimension * 4 + 8;
        let expected_size = 20 + count * entry_size;
        if bytes.len() < expected_size {
            return Err(HnswError::InvalidData);
        }

        let mut store = Self::with_capacity(dimension, max_capacity.max(Self::DEFAULT_MAX_CAPACITY));

        for _ in 0..count {
            if pos + 8 > bytes.len() {
                return Err(HnswError::InvalidData);
            }

            let id = u64::from_le_bytes(
                bytes[pos..pos + 8]
                    .try_into()
                    .map_err(|_| HnswError::InvalidData)?,
            );
            pos += 8;

            let mut vector = Vec::with_capacity(dimension);
            for _ in 0..dimension {
                if pos + 4 > bytes.len() {
                    return Err(HnswError::InvalidData);
                }
                let v = f32::from_le_bytes(
                    bytes[pos..pos + 4]
                        .try_into()
                        .map_err(|_| HnswError::InvalidData)?,
                );
                pos += 4;
                vector.push(v);
            }

            let timestamp = u64::from_le_bytes(
                bytes[pos..pos + 8]
                    .try_into()
                    .map_err(|_| HnswError::InvalidData)?,
            );
            pos += 8;

            // Re-insert into graph
            if let Err(e) = store.insert(id, vector) {
                tracing::warn!("Skipping vector {id} during restore: {e}");
                continue;
            }

            if timestamp > 0 {
                store.timestamps.insert(id, timestamp);
            }
        }

        Ok(store)
    }

    // ==================== Internal Helpers ====================

    /// Convert a f32 slice to FixedPointVector<768>.
    ///
    /// If the input dimension is less than 768, pads with zeros.
    /// If greater, truncates (shouldn't happen in production).
    fn f32_to_fixed(&self, input: &[f32]) -> Result<FixedPointVector<STANDARD_DIM>, HnswError> {
        let mut padded = [0.0f32; STANDARD_DIM];
        let copy_len = input.len().min(STANDARD_DIM);
        padded[..copy_len].copy_from_slice(&input[..copy_len]);

        FixedPointVector::from_f32_slice(&padded)
    }
}

impl Default for HnswVectorStore {
    fn default() -> Self {
        Self::new(768)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert_and_search() {
        let mut store = HnswVectorStore::new(4);

        // Insert with padded-to-768 internally
        store.insert(1, vec![1.0; 4]).unwrap();
        store.insert(2, vec![0.0; 4]).unwrap();
        store.insert(3, vec![0.5; 4]).unwrap();

        assert_eq!(store.len(), 3);

        let results = store.search(&[1.0; 4], 2).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].0, 1); // Exact match should be first
    }

    #[test]
    fn test_capacity_limit() {
        let mut store = HnswVectorStore::with_capacity(4, 2);

        store.insert(1, vec![1.0; 4]).unwrap();
        store.insert(2, vec![0.0; 4]).unwrap();

        let result = store.insert(3, vec![0.5; 4]);
        assert!(matches!(result, Err(HnswError::CapacityExceeded)));
    }

    #[test]
    fn test_duplicate_rejection() {
        let mut store = HnswVectorStore::new(4);

        store.insert(1, vec![1.0; 4]).unwrap();
        let result = store.insert(1, vec![0.0; 4]);
        assert!(matches!(result, Err(HnswError::DuplicateId(1))));
    }

    #[test]
    fn test_dimension_mismatch() {
        let mut store = HnswVectorStore::new(4);

        let result = store.insert(1, vec![1.0; 3]);
        assert!(matches!(result, Err(HnswError::DimensionMismatch { .. })));
    }

    #[test]
    fn test_remove() {
        let mut store = HnswVectorStore::new(4);

        store.insert(1, vec![1.0; 4]).unwrap();
        assert_eq!(store.len(), 1);

        assert!(store.remove(1));
        assert_eq!(store.len(), 0);

        assert!(!store.remove(1)); // Already removed
    }

    #[test]
    fn test_get_vector() {
        let mut store = HnswVectorStore::new(4);

        let vec_data = vec![1.0, 2.0, 3.0, 4.0];
        store.insert(42, vec_data.clone()).unwrap();

        assert_eq!(store.get_vector(42), Some(vec_data));
        assert_eq!(store.get_vector(99), None);
    }

    #[test]
    fn test_root_hash_deterministic() {
        let mut store1 = HnswVectorStore::new(4);
        let mut store2 = HnswVectorStore::new(4);

        store1.insert(1, vec![1.0; 4]).unwrap();
        store1.insert(2, vec![2.0; 4]).unwrap();

        store2.insert(1, vec![1.0; 4]).unwrap();
        store2.insert(2, vec![2.0; 4]).unwrap();

        assert_eq!(store1.root_hash(), store2.root_hash());
    }

    #[test]
    fn test_ttl_pruning() {
        let mut store = HnswVectorStore::new(4);

        store.insert_with_block(1, vec![1.0; 4], 100).unwrap();
        store.insert_with_block(2, vec![2.0; 4], 200).unwrap();
        store.insert_with_block(3, vec![3.0; 4], 300).unwrap();

        let pruned = store.prune_before(250);
        assert_eq!(pruned, 2); // IDs 1 and 2 pruned
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut store = HnswVectorStore::new(4);

        store.insert_with_block(10, vec![1.0, 2.0, 3.0, 4.0], 100).unwrap();
        store.insert_with_block(20, vec![5.0, 6.0, 7.0, 8.0], 200).unwrap();

        let bytes = store.to_bytes();
        let restored = HnswVectorStore::from_bytes(&bytes).unwrap();

        assert_eq!(restored.len(), 2);
        assert_eq!(restored.get_vector(10), Some(vec![1.0, 2.0, 3.0, 4.0]));
        assert_eq!(restored.get_vector(20), Some(vec![5.0, 6.0, 7.0, 8.0]));
        assert_eq!(store.root_hash(), restored.root_hash());
    }

    #[test]
    fn test_empty_search() {
        let store = HnswVectorStore::new(4);
        let results = store.search(&[1.0; 4], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_classify() {
        let mut store = HnswVectorStore::new(4);

        store.insert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        store.insert(2, vec![0.0, 1.0, 0.0, 0.0]).unwrap();

        let labels = vec![(1, 10), (2, 20)];
        let (label, confidence) = store.classify(&[0.9, 0.1, 0.0, 0.0], &labels).unwrap();

        assert_eq!(label, 10); // Should match label for vector 1
        assert!(confidence > 0.0);
    }

    #[test]
    fn test_similarity_check() {
        let store = HnswVectorStore::new(4);

        let (is_similar, score) = store
            .similarity_check(&[1.0, 0.0, 0.0, 0.0], &[1.0, 0.0, 0.0, 0.0], 0.5)
            .unwrap();

        assert!(is_similar);
        assert!((score - 1.0).abs() < 0.01);
    }
}
