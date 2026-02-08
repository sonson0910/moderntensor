//! HNSW Graph Implementation
//!
//! This module provides the core `HnswGraph<D>` data structure - a
//! consensus-safe Hierarchical Navigable Small World graph for approximate
//! nearest neighbor search.
//!
//! ## Architecture
//!
//! The graph consists of multiple layers:
//! - Layer 0: Contains all nodes, most dense
//! - Higher layers: Progressively sparser, act as "express lanes"
//!
//! ## Consensus Safety
//!
//! All operations are strictly deterministic:
//! - Node levels determined by `DeterministicRng`
//! - Distances calculated using `FixedPointVector` (no floats)
//! - Insertions must follow block transaction order

use crate::fixed_point::I64F32;
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;
use tracing::debug;

use crate::{
    deterministic_rng::DeterministicRng,
    error::{HnswError, Result},
    fixed_point::FixedPointVector,
    EF_CONSTRUCTION, M, M0, ml,
};

/// A node in the HNSW graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswNode<const D: usize> {
    /// The node's unique identifier
    pub id: usize,
    /// The vector data
    pub vector: FixedPointVector<D>,
    /// Maximum level this node appears in
    pub level: u8,
    /// Neighbors at each level (level -> list of neighbor IDs)
    pub neighbors: Vec<Vec<usize>>,
}

impl<const D: usize> HnswNode<D> {
    /// Create a new node with the given ID, vector, and level.
    pub fn new(id: usize, vector: FixedPointVector<D>, level: u8) -> Self {
        let mut neighbors = Vec::with_capacity(level as usize + 1);
        for _ in 0..=level {
            neighbors.push(Vec::new());
        }
        Self {
            id,
            vector,
            level,
            neighbors,
        }
    }

    /// Get the maximum connections allowed at the given level.
    pub fn max_connections(level: u8) -> usize {
        if level == 0 {
            M0
        } else {
            M
        }
    }
}

/// A candidate during search, ordered by distance (min-heap).
#[derive(Clone, Debug)]
struct Candidate {
    id: usize,
    distance: I64F32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance && self.id == other.id
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
        // Reverse order for min-heap (smaller distance = higher priority)
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

/// Wrapper for max-heap ordering (furthest first).
#[derive(Clone, Debug)]
struct FurthestCandidate(Candidate);

impl PartialEq for FurthestCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for FurthestCandidate {}

impl PartialOrd for FurthestCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FurthestCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Normal order for max-heap (larger distance = higher priority)
        self.0.distance.partial_cmp(&other.0.distance).unwrap_or(Ordering::Equal)
    }
}

/// A consensus-safe HNSW graph for D-dimensional vectors.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswGraph<const D: usize> {
    /// All nodes in the graph
    nodes: Vec<HnswNode<D>>,
    /// Entry point node ID (highest level node)
    entry_point: Option<usize>,
    /// Current maximum level in the graph
    max_level: u8,
    /// Level multiplier for probability distribution
    ml: f64,
    /// Search expansion factor during construction
    ef_construction: usize,
}

impl<const D: usize> Default for HnswGraph<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const D: usize> HnswGraph<D> {
    /// Create a new empty HNSW graph with default parameters.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            entry_point: None,
            max_level: 0,
            ml: ml(),
            ef_construction: EF_CONSTRUCTION,
        }
    }

    /// Create a new HNSW graph with custom parameters.
    pub fn with_params(ml: f64, ef_construction: usize) -> Self {
        Self {
            nodes: Vec::new(),
            entry_point: None,
            max_level: 0,
            ml,
            ef_construction,
        }
    }

    /// Get the number of nodes in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: usize) -> Option<&HnswNode<D>> {
        self.nodes.get(id)
    }

    /// Get the current maximum level in the graph.
    pub fn max_level(&self) -> u8 {
        self.max_level
    }

    /// Insert a vector into the graph with deterministic level assignment.
    ///
    /// # Arguments
    /// * `vector` - The fixed-point vector to insert
    /// * `rng` - A deterministic RNG seeded by consensus artifacts
    ///
    /// # Returns
    /// The ID of the newly inserted node.
    pub fn insert(&mut self, vector: FixedPointVector<D>, rng: &mut DeterministicRng) -> usize {
        let node_id = self.nodes.len();
        let level = rng.next_level(16, self.ml);

        debug!(node_id = node_id, level = level, "Inserting node");

        let node = HnswNode::new(node_id, vector.clone(), level);

        if self.entry_point.is_none() {
            // First node - just add it
            self.nodes.push(node);
            self.entry_point = Some(node_id);
            self.max_level = level;
            return node_id;
        }

        // Push the node first so it has a valid index
        self.nodes.push(node);

        let entry_point = self.entry_point
            .expect("entry_point guarded by is_empty check above");
        let mut current_node = entry_point;

        // Phase 1: Greedy search from top level down to node's level + 1
        for lc in (level as i32 + 1..=self.max_level as i32).rev() {
            let lc = lc as u8;
            current_node = self.search_layer_greedy(&vector, current_node, lc);
        }

        // Phase 2: Insert at each level from min(level, max_level) down to 0
        let start_level = level.min(self.max_level);
        for lc in (0..=start_level).rev() {
            let candidates = self.search_layer(&vector, current_node, self.ef_construction, lc);
            let neighbors = self.select_neighbors(&vector, &candidates, lc);

            // Add bidirectional connections - update the node we already pushed
            self.nodes[node_id].neighbors[lc as usize] = neighbors.clone();

            for &neighbor_id in &neighbors {
                // Skip self-reference
                if neighbor_id == node_id {
                    continue;
                }

                let neighbor = &mut self.nodes[neighbor_id];
                if lc as usize >= neighbor.neighbors.len() {
                    continue;
                }
                neighbor.neighbors[lc as usize].push(node_id);

                // Prune if too many connections
                let max_conn = HnswNode::<D>::max_connections(lc);
                if neighbor.neighbors[lc as usize].len() > max_conn {
                    let neighbor_vec = neighbor.vector.clone();
                    let current_neighbors: Vec<usize> =
                        neighbor.neighbors[lc as usize].clone();
                    let candidates: Vec<Candidate> = current_neighbors
                        .iter()
                        .filter(|&&id| id != node_id) // Don't include the new node in distance calc
                        .map(|&id| Candidate {
                            id,
                            distance: neighbor_vec.squared_distance(&self.nodes[id].vector),
                        })
                        .collect();
                    let mut pruned = self.select_neighbors_from_candidates(&candidates, max_conn - 1);
                    pruned.push(node_id); // Always keep the new connection
                    self.nodes[neighbor_id].neighbors[lc as usize] = pruned;
                }
            }

            if !candidates.is_empty() {
                current_node = candidates[0].id;
            }
        }

        // Update entry point if new node has higher level
        if level > self.max_level {
            self.entry_point = Some(node_id);
            self.max_level = level;
        }

        node_id
    }

    /// Search for the k nearest neighbors of a query vector.
    ///
    /// # Arguments
    /// * `query` - The query vector
    /// * `k` - Number of neighbors to return
    /// * `ef` - Search expansion factor (higher = more accurate but slower)
    ///
    /// # Returns
    /// A vector of (node_id, squared_distance) pairs, sorted by distance.
    pub fn search(
        &self,
        query: &FixedPointVector<D>,
        k: usize,
        ef: usize,
    ) -> Result<Vec<(usize, I64F32)>> {
        if self.entry_point.is_none() {
            return Err(HnswError::EmptyGraph);
        }

        let mut current_node = self.entry_point
            .expect("entry_point guarded by is_none check above");

        // Greedy search from top to level 1
        for lc in (1..=self.max_level).rev() {
            current_node = self.search_layer_greedy(query, current_node, lc);
        }

        // Search at layer 0 with ef candidates
        let candidates = self.search_layer(query, current_node, ef.max(k), 0);

        // Return top k
        let results: Vec<(usize, I64F32)> = candidates
            .into_iter()
            .take(k)
            .map(|c| (c.id, c.distance))
            .collect();

        Ok(results)
    }

    /// Greedy search at a single layer, returning the closest node.
    fn search_layer_greedy(&self, query: &FixedPointVector<D>, entry: usize, level: u8) -> usize {
        let mut current = entry;
        let mut current_dist = query.squared_distance(&self.nodes[current].vector);

        loop {
            let mut improved = false;
            let neighbors = &self.nodes[current].neighbors.get(level as usize);

            if let Some(neighbors) = neighbors {
                for &neighbor_id in neighbors.iter() {
                    let dist = query.squared_distance(&self.nodes[neighbor_id].vector);
                    if dist < current_dist {
                        current = neighbor_id;
                        current_dist = dist;
                        improved = true;
                    }
                }
            }

            if !improved {
                break;
            }
        }

        current
    }

    /// Search at a single layer, returning top ef candidates.
    fn search_layer(
        &self,
        query: &FixedPointVector<D>,
        entry: usize,
        ef: usize,
        level: u8,
    ) -> Vec<Candidate> {
        let entry_dist = query.squared_distance(&self.nodes[entry].vector);

        let mut visited: HashSet<usize> = HashSet::new();
        visited.insert(entry);

        // Min-heap of candidates to explore
        let mut candidates: BinaryHeap<Candidate> = BinaryHeap::new();
        candidates.push(Candidate {
            id: entry,
            distance: entry_dist,
        });

        // Max-heap of results (furthest first for pruning)
        let mut results: BinaryHeap<FurthestCandidate> = BinaryHeap::new();
        results.push(FurthestCandidate(Candidate {
            id: entry,
            distance: entry_dist,
        }));

        while let Some(current) = candidates.pop() {
            // Stop if current is further than the furthest result
            let furthest_dist = results.peek().map(|f| f.0.distance).unwrap_or(I64F32::MAX);
            if current.distance > furthest_dist {
                break;
            }

            let neighbors = self.nodes[current.id]
                .neighbors
                .get(level as usize)
                .cloned()
                .unwrap_or_default();

            for neighbor_id in neighbors {
                if visited.contains(&neighbor_id) {
                    continue;
                }
                visited.insert(neighbor_id);

                let dist = query.squared_distance(&self.nodes[neighbor_id].vector);
                let furthest_dist = results.peek().map(|f| f.0.distance).unwrap_or(I64F32::MAX);

                if dist < furthest_dist || results.len() < ef {
                    candidates.push(Candidate {
                        id: neighbor_id,
                        distance: dist,
                    });
                    results.push(FurthestCandidate(Candidate {
                        id: neighbor_id,
                        distance: dist,
                    }));

                    // Keep only top ef results
                    while results.len() > ef {
                        results.pop();
                    }
                }
            }
        }

        // Convert to sorted vec
        let mut result_vec: Vec<Candidate> = results.into_iter().map(|f| f.0).collect();
        result_vec.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        result_vec
    }

    /// Select the best neighbors from candidates using the simple heuristic.
    fn select_neighbors(
        &self,
        _query: &FixedPointVector<D>,
        candidates: &[Candidate],
        level: u8,
    ) -> Vec<usize> {
        let max_conn = HnswNode::<D>::max_connections(level);
        self.select_neighbors_from_candidates(candidates, max_conn)
    }

    /// Select up to max_count neighbors from candidates.
    fn select_neighbors_from_candidates(
        &self,
        candidates: &[Candidate],
        max_count: usize,
    ) -> Vec<usize> {
        // Simple selection: take the closest ones
        let mut sorted: Vec<Candidate> = candidates.to_vec();
        sorted.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        sorted.into_iter().take(max_count).map(|c| c.id).collect()
    }

    /// Serialize the graph to bytes for persistence.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(HnswError::from)
    }

    /// Deserialize a graph from bytes.
    ///
    /// SECURITY: Applies size limits to prevent memory exhaustion from malicious data.
    /// Maximum serialized graph size: 256 MB (prevents OOM from crafted payloads).
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        // Limit maximum deserialization size to 256 MB to prevent OOM
        const MAX_GRAPH_SIZE: usize = 256 * 1024 * 1024;
        if data.len() > MAX_GRAPH_SIZE {
            return Err(HnswError::DeserializationError(
                format!(
                    "Graph data too large: {} bytes (max: {} bytes)",
                    data.len(),
                    MAX_GRAPH_SIZE,
                ),
            ));
        }

        let graph: Self = bincode::deserialize(data)
            .map_err(|e| HnswError::DeserializationError(e.to_string()))?;

        // Validate deserialized graph: node count must be reasonable
        const MAX_NODES: usize = 10_000_000; // 10M nodes max
        if graph.len() > MAX_NODES {
            return Err(HnswError::DeserializationError(
                format!(
                    "Graph has too many nodes: {} (max: {})",
                    graph.len(),
                    MAX_NODES,
                ),
            ));
        }

        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_rng() -> DeterministicRng {
        DeterministicRng::from_seed([42u8; 32])
    }

    #[test]
    fn test_empty_graph() {
        let graph: HnswGraph<3> = HnswGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);
    }

    #[test]
    fn test_insert_single() {
        let mut graph: HnswGraph<3> = HnswGraph::new();
        let mut rng = create_test_rng();
        let vector = FixedPointVector::from_f32_slice(&[1.0, 2.0, 3.0]).unwrap();

        let id = graph.insert(vector, &mut rng);
        assert_eq!(id, 0);
        assert_eq!(graph.len(), 1);
    }

    #[test]
    fn test_insert_multiple() {
        let mut graph: HnswGraph<3> = HnswGraph::new();
        let mut rng = create_test_rng();

        for i in 0..100 {
            let vector =
                FixedPointVector::from_f32_slice(&[i as f32, i as f32 * 2.0, i as f32 * 3.0])
                    .unwrap();
            graph.insert(vector, &mut rng);
        }

        assert_eq!(graph.len(), 100);
    }

    #[test]
    fn test_search_basic() {
        let mut graph: HnswGraph<3> = HnswGraph::new();
        let mut rng = create_test_rng();

        // Insert some vectors
        let v1 = FixedPointVector::from_f32_slice(&[0.0, 0.0, 0.0]).unwrap();
        let v2 = FixedPointVector::from_f32_slice(&[1.0, 0.0, 0.0]).unwrap();
        let v3 = FixedPointVector::from_f32_slice(&[10.0, 0.0, 0.0]).unwrap();

        graph.insert(v1, &mut rng);
        graph.insert(v2, &mut rng);
        graph.insert(v3, &mut rng);

        // Search for something close to origin
        let query = FixedPointVector::from_f32_slice(&[0.1, 0.0, 0.0]).unwrap();
        let results = graph.search(&query, 2, 10).unwrap();

        assert_eq!(results.len(), 2);
        // First result should be node 0 (origin) or node 1 (1,0,0)
        assert!(results[0].0 == 0 || results[0].0 == 1);
    }

    #[test]
    fn test_determinism() {
        // Two graphs with same RNG should produce identical structure
        let mut graph1: HnswGraph<3> = HnswGraph::new();
        let mut graph2: HnswGraph<3> = HnswGraph::new();
        let mut rng1 = create_test_rng();
        let mut rng2 = create_test_rng();

        for i in 0..50 {
            let vector =
                FixedPointVector::from_f32_slice(&[i as f32, i as f32 * 2.0, i as f32 * 3.0])
                    .unwrap();
            graph1.insert(vector.clone(), &mut rng1);
            graph2.insert(vector, &mut rng2);
        }

        // Serialize both and compare
        let bytes1 = graph1.serialize().unwrap();
        let bytes2 = graph2.serialize().unwrap();
        assert_eq!(bytes1, bytes2, "Graphs should be identical");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut graph: HnswGraph<3> = HnswGraph::new();
        let mut rng = create_test_rng();

        for i in 0..20 {
            let vector = FixedPointVector::from_f32_slice(&[i as f32, 0.0, 0.0]).unwrap();
            graph.insert(vector, &mut rng);
        }

        let bytes = graph.serialize().unwrap();
        let restored: HnswGraph<3> = HnswGraph::deserialize(&bytes).unwrap();

        assert_eq!(graph.len(), restored.len());
        assert_eq!(graph.max_level, restored.max_level);
    }
}
