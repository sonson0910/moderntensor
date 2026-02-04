//! HNSW (Hierarchical Navigable Small World) Index Implementation
//!
//! Provides O(log N) approximate nearest neighbor search for the Semantic Layer.
//! Designed for blockchain constraints: deterministic, serializable, gas-efficient.

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;

/// HNSW configuration parameters
#[derive(Clone, Debug)]
pub struct HnswConfig {
    /// Maximum number of connections per element per layer
    pub m: usize,
    /// Maximum number of connections for layer 0
    pub m0: usize,
    /// Size of dynamic candidate list during construction
    pub ef_construction: usize,
    /// Size of dynamic candidate list during search
    pub ef_search: usize,
    /// Maximum number of elements
    pub max_elements: usize,
    /// Vector dimension
    pub dimension: usize,
    /// Normalization factor for level generation
    pub ml: f64,
    /// Maximum layer height (caps random_layer output)
    pub max_layer: usize,
}

/// Default HNSW connections per layer (M parameter)
const DEFAULT_M: usize = 16;

/// Maximum HNSW layer height (prevents excessive hierarchy)
const DEFAULT_MAX_LAYER: usize = 16;

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: DEFAULT_M,        // Connections per layer (balanced recall/speed)
            m0: DEFAULT_M * 2,   // Layer 0 connections (2x m)
            ef_construction: 200, // Build quality (high for production)
            ef_search: 64,       // Production: higher recall (>95%)
            max_elements: 1_000_000, // Production scale: 1M vectors
            dimension: 768,      // Standard embedding dimension
            ml: 1.0 / (DEFAULT_M as f64).ln(),
            max_layer: DEFAULT_MAX_LAYER,
        }
    }
}

/// A node in the HNSW graph
#[derive(Clone, Debug)]
pub struct HnswNode {
    pub id: u64,
    pub vector: Vec<f32>,
    pub connections: Vec<Vec<u64>>,
    pub max_layer: usize,
}

#[derive(Clone, Debug)]
struct Candidate {
    id: u64,
    distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

#[derive(Clone, Debug)]
struct MaxCandidate {
    id: u64,
    distance: f32,
}

impl PartialEq for MaxCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for MaxCandidate {}

impl PartialOrd for MaxCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MaxCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance.partial_cmp(&other.distance).unwrap_or(Ordering::Equal)
    }
}

/// HNSW Index for approximate nearest neighbor search
#[derive(Clone, Debug)]
pub struct HnswIndex {
    config: HnswConfig,
    nodes: HashMap<u64, HnswNode>,
    entry_point: Option<u64>,
    max_layer: usize,
    count: usize,
}

impl HnswIndex {
    pub fn new(dimension: usize) -> Self {
        let mut config = HnswConfig::default();
        config.dimension = dimension;
        Self::with_config(config)
    }

    pub fn with_config(config: HnswConfig) -> Self {
        Self {
            config,
            nodes: HashMap::new(),
            entry_point: None,
            max_layer: 0,
            count: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Insert a vector into the index
    pub fn insert(&mut self, id: u64, vector: Vec<f32>) -> Result<(), HnswError> {
        if vector.len() != self.config.dimension {
            return Err(HnswError::DimensionMismatch {
                expected: self.config.dimension,
                got: vector.len(),
            });
        }

        if self.count >= self.config.max_elements {
            return Err(HnswError::CapacityExceeded);
        }

        if self.nodes.contains_key(&id) {
            return Err(HnswError::DuplicateId(id));
        }

        let node_layer = self.random_layer(id);

        let mut new_node = HnswNode {
            id,
            vector: vector.clone(),
            connections: vec![Vec::new(); node_layer + 1],
            max_layer: node_layer,
        };

        if self.entry_point.is_none() {
            self.nodes.insert(id, new_node);
            self.entry_point = Some(id);
            self.max_layer = node_layer;
            self.count += 1;
            return Ok(());
        }

        let entry_id = self.entry_point.unwrap();
        let mut current_id = entry_id;

        // Phase 1: Greedy search from top layer down to node_layer + 1
        for layer in (node_layer + 1..=self.max_layer).rev() {
            current_id = self.search_layer_greedy(&vector, current_id, layer);
        }

        // Phase 2: Search and connect - collect updates first
        let mut neighbor_updates: Vec<(u64, usize)> = Vec::new();

        for layer in (0..=node_layer.min(self.max_layer)).rev() {
            let ef = self.config.ef_construction;
            let candidates = self.search_layer(&vector, current_id, ef, layer);

            let m = if layer == 0 { self.config.m0 } else { self.config.m };
            let neighbors = self.select_neighbors(&vector, &candidates, m);

            new_node.connections[layer] = neighbors.clone();

            for &neighbor_id in &neighbors {
                neighbor_updates.push((neighbor_id, layer));
            }

            if !candidates.is_empty() {
                current_id = candidates[0].0;
            }
        }

        // Apply neighbor connections
        for (neighbor_id, layer) in &neighbor_updates {
            if let Some(neighbor) = self.nodes.get_mut(neighbor_id) {
                if *layer < neighbor.connections.len() {
                    neighbor.connections[*layer].push(id);
                }
            }
        }

        // Prune overloaded neighbors separately to avoid borrow conflicts
        let m0 = self.config.m0;
        let m = self.config.m;

        for (neighbor_id, layer) in neighbor_updates {
            let max_m = if layer == 0 { m0 } else { m };

            let prune_data = {
                if let Some(neighbor) = self.nodes.get(&neighbor_id) {
                    if layer < neighbor.connections.len() && neighbor.connections[layer].len() > max_m {
                        Some((neighbor.vector.clone(), neighbor.connections[layer].clone()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((neighbor_vec, conn_ids)) = prune_data {
                let candidates: Vec<(u64, f32)> = conn_ids
                    .iter()
                    .filter_map(|&cid| {
                        self.nodes.get(&cid).map(|n| {
                            (cid, self.distance(&neighbor_vec, &n.vector))
                        })
                    })
                    .collect();

                let pruned = self.select_neighbors_simple(&candidates, max_m);

                if let Some(neighbor) = self.nodes.get_mut(&neighbor_id) {
                    if layer < neighbor.connections.len() {
                        neighbor.connections[layer] = pruned;
                    }
                }
            }
        }

        self.nodes.insert(id, new_node);
        self.count += 1;

        if node_layer > self.max_layer {
            self.entry_point = Some(id);
            self.max_layer = node_layer;
        }

        Ok(())
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>, HnswError> {
        if query.len() != self.config.dimension {
            return Err(HnswError::DimensionMismatch {
                expected: self.config.dimension,
                got: query.len(),
            });
        }

        if self.entry_point.is_none() {
            return Ok(Vec::new());
        }

        let entry_id = self.entry_point.unwrap();
        let mut current_id = entry_id;

        for layer in (1..=self.max_layer).rev() {
            current_id = self.search_layer_greedy(query, current_id, layer);
        }

        let ef = self.config.ef_search.max(k);
        let candidates = self.search_layer(query, current_id, ef, 0);

        let results: Vec<(u64, f32)> = candidates.into_iter().take(k).collect();
        Ok(results)
    }

    fn search_layer_greedy(&self, query: &[f32], entry_id: u64, layer: usize) -> u64 {
        let mut current_id = entry_id;
        let mut current_dist = self.distance_to_node(query, current_id);

        loop {
            let mut changed = false;

            if let Some(node) = self.nodes.get(&current_id) {
                if layer < node.connections.len() {
                    for &neighbor_id in &node.connections[layer] {
                        let dist = self.distance_to_node(query, neighbor_id);
                        if dist < current_dist {
                            current_id = neighbor_id;
                            current_dist = dist;
                            changed = true;
                        }
                    }
                }
            }

            if !changed {
                break;
            }
        }

        current_id
    }

    fn search_layer(&self, query: &[f32], entry_id: u64, ef: usize, layer: usize) -> Vec<(u64, f32)> {
        let mut visited = HashSet::new();
        let mut candidates: BinaryHeap<Candidate> = BinaryHeap::new();
        let mut results: BinaryHeap<MaxCandidate> = BinaryHeap::new();

        let entry_dist = self.distance_to_node(query, entry_id);

        visited.insert(entry_id);
        candidates.push(Candidate { id: entry_id, distance: entry_dist });
        results.push(MaxCandidate { id: entry_id, distance: entry_dist });

        while let Some(current) = candidates.pop() {
            let furthest_dist = results.peek().map(|c| c.distance).unwrap_or(f32::MAX);

            if current.distance > furthest_dist {
                break;
            }

            if let Some(node) = self.nodes.get(&current.id) {
                if layer < node.connections.len() {
                    for &neighbor_id in &node.connections[layer] {
                        if visited.insert(neighbor_id) {
                            let dist = self.distance_to_node(query, neighbor_id);
                            let furthest = results.peek().map(|c| c.distance).unwrap_or(f32::MAX);

                            if dist < furthest || results.len() < ef {
                                candidates.push(Candidate { id: neighbor_id, distance: dist });
                                results.push(MaxCandidate { id: neighbor_id, distance: dist });

                                if results.len() > ef {
                                    results.pop();
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut result_vec: Vec<(u64, f32)> = results.into_iter().map(|c| (c.id, c.distance)).collect();
        result_vec.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        result_vec
    }

    fn select_neighbors(&self, _query: &[f32], candidates: &[(u64, f32)], m: usize) -> Vec<u64> {
        candidates.iter().take(m).map(|(id, _)| *id).collect()
    }

    fn select_neighbors_simple(&self, candidates: &[(u64, f32)], m: usize) -> Vec<u64> {
        let mut sorted = candidates.to_vec();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        sorted.into_iter().take(m).map(|(id, _)| id).collect()
    }

    fn distance_to_node(&self, query: &[f32], node_id: u64) -> f32 {
        self.nodes.get(&node_id).map(|node| self.distance(query, &node.vector)).unwrap_or(f32::MAX)
    }

    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum()
    }

    fn random_layer(&self, id: u64) -> usize {
        let hash = Self::hash_u64(id);
        let uniform = (hash as f64) / (u64::MAX as f64);
        let layer = (-uniform.ln() * self.config.ml).floor() as usize;
        layer.min(self.config.max_layer)
    }

    fn hash_u64(x: u64) -> u64 {
        let mut h = x;
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        h
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(&(self.config.dimension as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.config.m as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.config.ef_search as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.count as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.max_layer as u32).to_le_bytes());
        bytes.extend_from_slice(&self.entry_point.unwrap_or(0).to_le_bytes());
        bytes.push(if self.entry_point.is_some() { 1 } else { 0 });

        for (id, node) in &self.nodes {
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.extend_from_slice(&(node.max_layer as u32).to_le_bytes());

            for &v in &node.vector {
                bytes.extend_from_slice(&v.to_le_bytes());
            }

            for layer_conns in &node.connections {
                bytes.extend_from_slice(&(layer_conns.len() as u32).to_le_bytes());
                for &conn_id in layer_conns {
                    bytes.extend_from_slice(&conn_id.to_le_bytes());
                }
            }
        }

        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HnswError> {
        if bytes.len() < 37 {
            return Err(HnswError::InvalidData);
        }

        let mut pos = 0;

        let dimension = u32::from_le_bytes(bytes[pos..pos+4].try_into().map_err(|_| HnswError::InvalidData)?) as usize;
        pos += 4;
        let m = u32::from_le_bytes(bytes[pos..pos+4].try_into().map_err(|_| HnswError::InvalidData)?) as usize;
        pos += 4;
        let ef_search = u32::from_le_bytes(bytes[pos..pos+4].try_into().map_err(|_| HnswError::InvalidData)?) as usize;
        pos += 4;

        let mut config = HnswConfig::default();
        config.dimension = dimension;
        config.m = m;
        config.m0 = m * 2;
        config.ef_search = ef_search;

        let count = u64::from_le_bytes(bytes[pos..pos+8].try_into().map_err(|_| HnswError::InvalidData)?) as usize;
        pos += 8;
        let max_layer = u32::from_le_bytes(bytes[pos..pos+4].try_into().map_err(|_| HnswError::InvalidData)?) as usize;
        pos += 4;
        let entry_id = u64::from_le_bytes(bytes[pos..pos+8].try_into().map_err(|_| HnswError::InvalidData)?);
        pos += 8;
        let has_entry = bytes[pos] == 1;
        pos += 1;

        let entry_point = if has_entry { Some(entry_id) } else { None };

        let mut nodes = HashMap::new();
        for _ in 0..count {
            if pos + 12 > bytes.len() {
                return Err(HnswError::InvalidData);
            }

            let id = u64::from_le_bytes(bytes[pos..pos+8].try_into().map_err(|_| HnswError::InvalidData)?);
            pos += 8;
            let node_max_layer = u32::from_le_bytes(bytes[pos..pos+4].try_into().map_err(|_| HnswError::InvalidData)?) as usize;
            pos += 4;

            let mut vector = Vec::with_capacity(dimension);
            for _ in 0..dimension {
                if pos + 4 > bytes.len() {
                    return Err(HnswError::InvalidData);
                }
                let v = f32::from_le_bytes(bytes[pos..pos+4].try_into().map_err(|_| HnswError::InvalidData)?);
                pos += 4;
                vector.push(v);
            }

            let mut connections = Vec::with_capacity(node_max_layer + 1);
            for _ in 0..=node_max_layer {
                if pos + 4 > bytes.len() {
                    return Err(HnswError::InvalidData);
                }
                let conn_len = u32::from_le_bytes(bytes[pos..pos+4].try_into().map_err(|_| HnswError::InvalidData)?) as usize;
                pos += 4;

                let mut layer_conns = Vec::with_capacity(conn_len);
                for _ in 0..conn_len {
                    if pos + 8 > bytes.len() {
                        return Err(HnswError::InvalidData);
                    }
                    let conn_id = u64::from_le_bytes(bytes[pos..pos+8].try_into().map_err(|_| HnswError::InvalidData)?);
                    pos += 8;
                    layer_conns.push(conn_id);
                }
                connections.push(layer_conns);
            }

            nodes.insert(id, HnswNode { id, vector, connections, max_layer: node_max_layer });
        }

        Ok(Self { config, nodes, entry_point, max_layer, count })
    }
}

/// HNSW-backed vector store for precompile integration
pub struct HnswVectorStore {
    index: HnswIndex,
}

impl HnswVectorStore {
    pub fn new(dimension: usize) -> Self {
        Self { index: HnswIndex::new(dimension) }
    }

    pub fn insert(&mut self, id: u64, vector: Vec<f32>) -> Result<(), HnswError> {
        self.index.insert(id, vector)
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>, HnswError> {
        self.index.search(query, k)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    #[allow(dead_code)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.index.to_bytes()
    }

    #[allow(dead_code)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HnswError> {
        Ok(Self { index: HnswIndex::from_bytes(bytes)? })
    }

    /// Calculate Merkle root hash for consensus verification
    /// Produces a deterministic 32-byte hash from all stored vectors
    pub fn root_hash(&self) -> [u8; 32] {
        if self.index.is_empty() {
            return [0u8; 32];
        }

        // Collect and sort IDs for deterministic ordering
        let mut ids: Vec<u64> = self.index.nodes.keys().copied().collect();
        ids.sort();

        // Hash all vector data in sorted order
        let mut data = Vec::new();
        for id in ids {
            data.extend_from_slice(&id.to_le_bytes());
            if let Some(node) = self.index.nodes.get(&id) {
                for val in &node.vector {
                    data.extend_from_slice(&val.to_le_bytes());
                }
            }
        }

        luxtensor_crypto::keccak256(&data)
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

        // Search for nearest neighbor among all stored vectors
        let results = self.search(query, 1)?;

        if results.is_empty() {
            return Err(HnswError::InvalidData);
        }

        let (nearest_id, distance) = results[0];

        // Find label for nearest vector
        let label = labels.iter()
            .find(|(id, _)| *id == nearest_id)
            .map(|(_, l)| *l)
            .unwrap_or(0);

        // Convert distance to confidence (1.0 = exact match, 0.0 = very far)
        // Using exponential decay: confidence = e^(-distance)
        let confidence = (-distance.sqrt()).exp().clamp(0.0, 1.0);

        Ok((label, confidence))
    }

    /// Calculate anomaly score for a vector relative to the stored vectors.
    /// Higher score means more anomalous (further from all stored vectors).
    ///
    /// # Returns
    /// * Score in range [0.0, 1.0] where 1.0 = highly anomalous
    pub fn anomaly_score(&self, query: &[f32]) -> Result<f32, HnswError> {
        if self.index.is_empty() {
            return Ok(1.0); // No data = everything is anomalous
        }

        // Get k nearest neighbors to calculate average distance
        let k = 5.min(self.index.len());
        let results = self.search(query, k)?;

        if results.is_empty() {
            return Ok(1.0);
        }

        // Calculate average distance to nearest neighbors
        let avg_distance: f32 = results.iter().map(|(_, d)| d).sum::<f32>() / results.len() as f32;

        // Normalize to [0, 1] using sigmoid-like function
        // threshold = 2.0 is "normal" distance, higher = more anomalous
        let threshold = 2.0;
        let score = 1.0 / (1.0 + (-((avg_distance / threshold) - 1.0)).exp());

        Ok(score.clamp(0.0, 1.0))
    }

    /// Check if two vectors are semantically similar above a threshold.
    ///
    /// # Arguments
    /// * `vector_a` - First vector
    /// * `vector_b` - Second vector
    /// * `threshold` - Similarity threshold (0.0 to 1.0)
    ///
    /// # Returns
    /// * `(is_similar, similarity_score)`
    pub fn similarity_check(&self, vector_a: &[f32], vector_b: &[f32], threshold: f32) -> Result<(bool, f32), HnswError> {
        if vector_a.len() != self.index.config.dimension || vector_b.len() != self.index.config.dimension {
            return Err(HnswError::DimensionMismatch {
                expected: self.index.config.dimension,
                got: vector_a.len().min(vector_b.len()),
            });
        }

        // Calculate Euclidean distance
        let distance: f32 = vector_a.iter()
            .zip(vector_b.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();

        // Convert distance to similarity (1.0 = identical, 0.0 = very different)
        let similarity = (-distance.sqrt() / 2.0).exp();
        let is_similar = similarity >= threshold;

        Ok((is_similar, similarity))
    }

    /// Get a vector by ID for cross-contract composability.
    pub fn get_vector(&self, id: u64) -> Option<Vec<f32>> {
        self.index.nodes.get(&id).map(|node| node.vector.clone())
    }

    /// Get the dimension of vectors in this store.
    pub fn dimension(&self) -> usize {
        self.index.config.dimension
    }
}


impl Default for HnswVectorStore {
    fn default() -> Self {
        Self::new(768) // Default to 768 dimensions (standard embedding size)
    }
}

/// HNSW errors
#[derive(Debug, Clone)]
pub enum HnswError {
    DimensionMismatch { expected: usize, got: usize },
    CapacityExceeded,
    DuplicateId(u64),
    InvalidData,
}

impl std::fmt::Display for HnswError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HnswError::DimensionMismatch { expected, got } => {
                write!(f, "Dimension mismatch: expected {}, got {}", expected, got)
            }
            HnswError::CapacityExceeded => write!(f, "Index capacity exceeded"),
            HnswError::DuplicateId(id) => write!(f, "Duplicate ID: {}", id),
            HnswError::InvalidData => write!(f, "Invalid serialized data"),
        }
    }
}

impl std::error::Error for HnswError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_insert_and_search() {
        let mut index = HnswIndex::new(4);

        index.insert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        index.insert(2, vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        index.insert(3, vec![0.0, 0.0, 1.0, 0.0]).unwrap();
        index.insert(4, vec![0.5, 0.5, 0.0, 0.0]).unwrap();

        assert_eq!(index.len(), 4);

        let results = index.search(&[1.0, 0.0, 0.0, 0.0], 2).unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].0, 1);
        assert!(results[0].1 < 0.01);
    }

    #[test]
    fn test_hnsw_serialization() {
        let mut index = HnswIndex::new(3);

        index.insert(10, vec![1.0, 2.0, 3.0]).unwrap();
        index.insert(20, vec![4.0, 5.0, 6.0]).unwrap();

        let bytes = index.to_bytes();
        let restored = HnswIndex::from_bytes(&bytes).unwrap();

        assert_eq!(restored.len(), 2);

        let results = restored.search(&[1.0, 2.0, 3.0], 1).unwrap();
        assert_eq!(results[0].0, 10);
    }

    #[test]
    fn test_hnsw_duplicate_rejection() {
        let mut index = HnswIndex::new(2);

        index.insert(1, vec![1.0, 0.0]).unwrap();
        let result = index.insert(1, vec![0.0, 1.0]);

        assert!(matches!(result, Err(HnswError::DuplicateId(1))));
    }

    #[test]
    fn test_hnsw_dimension_check() {
        let mut index = HnswIndex::new(3);

        let result = index.insert(1, vec![1.0, 2.0]);

        assert!(matches!(result, Err(HnswError::DimensionMismatch { .. })));
    }

    #[test]
    fn test_hnsw_vector_store() {
        let mut store = HnswVectorStore::new(4);

        store.insert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        store.insert(2, vec![0.9, 0.1, 0.0, 0.0]).unwrap();
        store.insert(3, vec![0.0, 1.0, 0.0, 0.0]).unwrap();

        let results = store.search(&[1.0, 0.0, 0.0, 0.0], 2).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 1);
    }

    // ==================== AI Primitives Tests ====================

    #[test]
    fn test_classify() {
        let mut store = HnswVectorStore::new(4);

        // Create labeled categories
        store.insert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap(); // Category A
        store.insert(2, vec![0.0, 1.0, 0.0, 0.0]).unwrap(); // Category B
        store.insert(3, vec![0.0, 0.0, 1.0, 0.0]).unwrap(); // Category C

        let labels = vec![(1, 100), (2, 200), (3, 300)];

        // Classify a vector close to category A
        let query = vec![0.95, 0.05, 0.0, 0.0];
        let (label, confidence) = store.classify(&query, &labels).unwrap();

        assert_eq!(label, 100); // Should match category A
        assert!(confidence > 0.5); // High confidence
    }

    #[test]
    fn test_anomaly_score() {
        let mut store = HnswVectorStore::new(4);

        // Create a cluster of normal vectors
        store.insert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        store.insert(2, vec![0.9, 0.1, 0.0, 0.0]).unwrap();
        store.insert(3, vec![0.95, 0.05, 0.0, 0.0]).unwrap();

        // Normal query (close to cluster)
        let normal_score = store.anomaly_score(&[1.0, 0.0, 0.0, 0.0]).unwrap();

        // Anomalous query (far from cluster)
        let anomaly_score = store.anomaly_score(&[0.0, 0.0, 0.0, 1.0]).unwrap();

        // Anomalous should have higher score
        assert!(anomaly_score > normal_score);
    }

    #[test]
    fn test_similarity_check() {
        let store = HnswVectorStore::new(4);

        // Similar vectors
        let vec_a = vec![1.0, 0.0, 0.0, 0.0];
        let vec_b = vec![0.95, 0.05, 0.0, 0.0];

        let (is_similar, similarity) = store.similarity_check(&vec_a, &vec_b, 0.5).unwrap();
        assert!(is_similar);
        assert!(similarity > 0.5);

        // Different vectors
        let vec_c = vec![0.0, 0.0, 0.0, 1.0];
        let (is_similar, _) = store.similarity_check(&vec_a, &vec_c, 0.9).unwrap();
        assert!(!is_similar);
    }

    #[test]
    fn test_get_vector() {
        let mut store = HnswVectorStore::new(4);

        let vector = vec![1.0, 2.0, 3.0, 4.0];
        store.insert(42, vector.clone()).unwrap();

        // Should find stored vector
        let retrieved = store.get_vector(42);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), vector);

        // Should return None for unknown ID
        let missing = store.get_vector(999);
        assert!(missing.is_none());
    }

    #[test]
    fn test_dimension() {
        let store = HnswVectorStore::new(768);
        assert_eq!(store.dimension(), 768);
    }
}

