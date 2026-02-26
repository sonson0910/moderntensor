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
///
/// Nodes are never physically removed from the graph to preserve index stability.
/// Instead, deleted nodes are marked with a tombstone flag and excluded from
/// search results. This ensures consensus safety: all nodes keep the same IDs
/// regardless of deletion order.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswNode<const D: usize> {
    /// The node's unique identifier (monotonically increasing, never reused)
    pub id: usize,
    /// The vector data
    pub vector: FixedPointVector<D>,
    /// Maximum level this node appears in
    pub level: u8,
    /// Neighbors at each level (level -> list of neighbor IDs)
    pub neighbors: Vec<Vec<usize>>,
    /// Tombstone flag: if true, this node is logically deleted and excluded
    /// from search results. Connections are preserved to maintain graph
    /// navigability (deleted nodes still serve as routing hops).
    #[serde(default)]
    pub deleted: bool,
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
            deleted: false,
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

/// Maximum number of nodes in a single HNSW graph.
///
/// This guards against unbounded memory growth. At D=768 with M0=32 connections,
/// each node occupies ~3.2 KB (768×4 bytes vector + 32×8 bytes neighbors + metadata),
/// so 5M nodes ≈ 16 GB. Adjust based on available node memory.
pub const MAX_CAPACITY: usize = 5_000_000;

/// A consensus-safe HNSW graph for D-dimensional vectors.
///
/// # Memory Model
///
/// The entire graph lives in memory (`Vec<HnswNode>`). There is currently no
/// disk-backed mode. For graphs larger than available RAM, shard across multiple
/// `HnswGraph` instances with domain-based routing (see `vector_store.rs`).
///
/// # Deletion Model
///
/// Nodes are **soft-deleted** via tombstone flags rather than physically removed.
/// This preserves:
/// - **Index stability**: Node IDs never change or get reused
/// - **Graph navigability**: Deleted nodes still serve as routing hops
/// - **Consensus safety**: All nodes agree on graph structure regardless of deletion order
///
/// Use [`mark_deleted()`] to delete and [`is_deleted()`] to check status.
/// Search results automatically exclude deleted nodes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswGraph<const D: usize> {
    /// All nodes in the graph (including soft-deleted ones)
    nodes: Vec<HnswNode<D>>,
    /// Entry point node ID (highest level non-deleted node)
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

    /// Get the total number of nodes (including soft-deleted ones).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of active (non-deleted) nodes.
    pub fn active_count(&self) -> usize {
        self.nodes.iter().filter(|n| !n.deleted).count()
    }

    /// Check if the graph is empty (no active nodes).
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() || self.active_count() == 0
    }

    /// Get a node by ID (returns deleted nodes too — check `node.deleted`).
    pub fn get_node(&self, id: usize) -> Option<&HnswNode<D>> {
        self.nodes.get(id)
    }

    /// Get the current maximum level in the graph.
    pub fn max_level(&self) -> u8 {
        self.max_level
    }

    // ── Soft Deletion ───────────────────────────────────────────────────

    /// Mark a node as deleted (tombstone).
    ///
    /// The node remains in the graph for routing purposes but is excluded
    /// from all search results. This is consensus-safe: the graph structure
    /// is unchanged, only the tombstone flag is flipped.
    ///
    /// # Errors
    /// Returns `InvalidNodeId` if the ID is out of bounds.
    pub fn mark_deleted(&mut self, id: usize) -> Result<()> {
        let node = self.nodes.get_mut(id)
            .ok_or(HnswError::InvalidNodeId(id))?;
        node.deleted = true;

        // If we just deleted the entry point, find a new one at the same or lower level
        if self.entry_point == Some(id) {
            self.entry_point = self.find_new_entry_point();
        }

        Ok(())
    }

    /// Check if a node has been soft-deleted.
    pub fn is_deleted(&self, id: usize) -> Option<bool> {
        self.nodes.get(id).map(|n| n.deleted)
    }

    /// Restore a previously deleted node.
    pub fn undelete(&mut self, id: usize) -> Result<()> {
        let node = self.nodes.get_mut(id)
            .ok_or(HnswError::InvalidNodeId(id))?;
        node.deleted = false;
        Ok(())
    }

    /// Find the best non-deleted entry point (highest level, lowest ID for determinism).
    fn find_new_entry_point(&self) -> Option<usize> {
        self.nodes.iter()
            .filter(|n| !n.deleted)
            .max_by_key(|n| (n.level, std::cmp::Reverse(n.id)))
            .map(|n| n.id)
    }

    // ── Insertion ────────────────────────────────────────────────────────

    /// Insert a vector into the graph with deterministic level assignment.
    ///
    /// # Algorithm (HNSW paper: Malkov & Yashunin, 2018)
    ///
    /// The insertion follows the standard HNSW two-phase procedure:
    ///
    /// **Phase 1 — Greedy Descent (layers `max_level` → `node_level + 1`):**
    /// Starting from the entry point at the top layer, perform greedy nearest
    /// neighbor search at each layer, using the result as the entry point for
    /// the next layer down. This quickly narrows down to the neighborhood of
    /// the new vector without the overhead of a full beam search.
    ///
    /// **Phase 2 — Insertion with Connections (layers `min(node_level, max_level)` → 0):**
    /// At each layer where the new node exists:
    /// 1. Run beam search (`ef_construction` candidates) to find the neighborhood
    /// 2. Select the best `M` (or `M0` at layer 0) neighbors
    /// 3. Create **bidirectional edges** between the new node and its neighbors
    /// 4. If any neighbor exceeds `max_connections`, **prune** its neighbor list
    ///    by keeping the closest `max_conn - 1` nodes plus the new connection
    ///
    /// **CONSENSUS-CRITICAL:** The level is assigned by `DeterministicRng`, which
    /// is seeded from `Keccak256(TxHash ⊕ BlockHash)`. All nodes MUST use the
    /// same RNG state for identical graph construction.
    ///
    /// # Arguments
    /// * `vector` - The fixed-point vector to insert (I64F32 components)
    /// * `rng` - A deterministic RNG seeded by consensus artifacts
    ///
    /// # Returns
    /// The ID of the newly inserted node.
    ///
    /// # Errors
    /// Returns `CapacityExceeded` if the graph has reached `MAX_CAPACITY`.
    pub fn insert(
        &mut self,
        vector: FixedPointVector<D>,
        rng: &mut DeterministicRng,
    ) -> Result<usize> {
        // Guard: enforce memory capacity to prevent unbounded growth
        if self.nodes.len() >= MAX_CAPACITY {
            return Err(HnswError::CapacityExceeded);
        }

        let node_id = self.nodes.len();

        // Level assignment: geometric distribution with parameter `ml`.
        // Higher levels are exponentially rarer, creating the hierachical
        // "express lane" structure. M=16 → average level ≈ 0.36.
        let level = rng.next_level(16, self.ml);

        debug!(node_id = node_id, level = level, "Inserting node");

        let node = HnswNode::new(node_id, vector.clone(), level);

        // ── First node: trivially becomes the entry point ──
        if self.entry_point.is_none() {
            self.nodes.push(node);
            self.entry_point = Some(node_id);
            self.max_level = level;
            return Ok(node_id);
        }

        // Push the node first so it has a valid index for bidirectional linking
        self.nodes.push(node);

        let entry_point = self.entry_point
            .unwrap_or_else(|| unreachable!());
        let mut current_node = entry_point;

        // ── Phase 1: Greedy descent from top to (node_level + 1) ──
        // Uses single-best-neighbor search (no beam) for speed.
        // This finds a good starting point for the detailed Phase 2 search.
        for lc in (level as i32 + 1..=self.max_level as i32).rev() {
            let lc = lc as u8;
            current_node = self.search_layer_greedy(&vector, current_node, lc);
        }

        // ── Phase 2: Insert with bidirectional connections ──
        // At each layer where this node exists, find neighbors and connect.
        let start_level = level.min(self.max_level);
        for lc in (0..=start_level).rev() {
            // Step 2a: Beam search to find ef_construction nearest candidates
            let candidates = self.search_layer(&vector, current_node, self.ef_construction, lc);
            // Step 2b: Select best M (or M0 at layer 0) neighbors from candidates
            let neighbors = self.select_neighbors(&vector, &candidates, lc);

            // Step 2c: Set forward connections (new node → neighbors)
            self.nodes[node_id].neighbors[lc as usize] = neighbors.clone();

            // Step 2d: Set reverse connections (neighbors → new node)
            for &neighbor_id in &neighbors {
                if neighbor_id == node_id {
                    continue; // skip self-reference
                }

                let neighbor = &mut self.nodes[neighbor_id];
                if lc as usize >= neighbor.neighbors.len() {
                    continue; // neighbor doesn't exist at this level
                }
                neighbor.neighbors[lc as usize].push(node_id);

                // Step 2e: Prune if neighbor now exceeds max_connections.
                // Keep the closest (max_conn - 1) nodes + always retain the new edge.
                // This heuristic ensures graph connectivity while bounding degree.
                let max_conn = HnswNode::<D>::max_connections(lc);
                if neighbor.neighbors[lc as usize].len() > max_conn {
                    let neighbor_vec = neighbor.vector.clone();
                    let current_neighbors: Vec<usize> =
                        neighbor.neighbors[lc as usize].clone();
                    let candidates: Vec<Candidate> = current_neighbors
                        .iter()
                        .filter(|&&id| id != node_id)
                        .map(|&id| Candidate {
                            id,
                            distance: neighbor_vec.squared_distance(&self.nodes[id].vector),
                        })
                        .collect();
                    let mut pruned = self.select_neighbors_from_candidates(&candidates, max_conn - 1);
                    pruned.push(node_id); // always keep the fresh connection
                    self.nodes[neighbor_id].neighbors[lc as usize] = pruned;
                }
            }

            // Use closest candidate as entry for next layer down
            if !candidates.is_empty() {
                current_node = candidates[0].id;
            }
        }

        // Update entry point if new node has higher level
        if level > self.max_level {
            self.entry_point = Some(node_id);
            self.max_level = level;
        }

        Ok(node_id)
    }

    /// Search for the k nearest **active** (non-deleted) neighbors of a query vector.
    ///
    /// Soft-deleted nodes are used for routing (they still have connections) but
    /// are excluded from the final result set.
    ///
    /// # Arguments
    /// * `query` - The query vector
    /// * `k` - Number of neighbors to return
    /// * `ef` - Search expansion factor (higher = more accurate but slower)
    ///
    /// # Returns
    /// A vector of (node_id, squared_distance) pairs, sorted by distance.
    /// Only active (non-deleted) nodes are included.
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
            .unwrap_or_else(|| unreachable!());

        // Greedy search from top to level 1
        for lc in (1..=self.max_level).rev() {
            current_node = self.search_layer_greedy(query, current_node, lc);
        }

        // Search at layer 0 with ef candidates
        let candidates = self.search_layer(query, current_node, ef.max(k), 0);

        // Return top k, filtering out soft-deleted nodes
        let results: Vec<(usize, I64F32)> = candidates
            .into_iter()
            .filter(|c| !self.nodes[c.id].deleted)
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

        // SECURITY: Validate entry_point is within bounds
        if let Some(ep) = graph.entry_point {
            if ep >= graph.nodes.len() {
                return Err(HnswError::DeserializationError(
                    format!(
                        "entry_point {} is out of bounds (nodes: {})",
                        ep,
                        graph.nodes.len(),
                    ),
                ));
            }
        }

        // SECURITY: Validate all neighbor IDs reference valid nodes
        let node_count = graph.nodes.len();
        for (node_id, node) in graph.nodes.iter().enumerate() {
            for (level, neighbors) in node.neighbors.iter().enumerate() {
                for &neighbor_id in neighbors {
                    if neighbor_id >= node_count {
                        return Err(HnswError::DeserializationError(
                            format!(
                                "node {} has invalid neighbor {} at level {} (max valid ID: {})",
                                node_id,
                                neighbor_id,
                                level,
                                node_count.saturating_sub(1),
                            ),
                        ));
                    }
                }
            }
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

        let id = graph.insert(vector, &mut rng).unwrap();
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
            graph.insert(vector, &mut rng).unwrap();
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

        graph.insert(v1, &mut rng).unwrap();
        graph.insert(v2, &mut rng).unwrap();
        graph.insert(v3, &mut rng).unwrap();

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
            graph1.insert(vector.clone(), &mut rng1).unwrap();
            graph2.insert(vector, &mut rng2).unwrap();
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
            graph.insert(vector, &mut rng).unwrap();
        }

        let bytes = graph.serialize().unwrap();
        let restored: HnswGraph<3> = HnswGraph::deserialize(&bytes).unwrap();

        assert_eq!(graph.len(), restored.len());
        assert_eq!(graph.max_level, restored.max_level);
    }

    #[test]
    fn test_soft_delete() {
        let mut graph: HnswGraph<3> = HnswGraph::new();
        let mut rng = create_test_rng();

        let v0 = FixedPointVector::from_f32_slice(&[0.0, 0.0, 0.0]).unwrap();
        let v1 = FixedPointVector::from_f32_slice(&[1.0, 0.0, 0.0]).unwrap();
        let v2 = FixedPointVector::from_f32_slice(&[10.0, 0.0, 0.0]).unwrap();

        graph.insert(v0, &mut rng).unwrap();
        graph.insert(v1, &mut rng).unwrap();
        graph.insert(v2, &mut rng).unwrap();

        // Delete node 0 (closest to origin)
        graph.mark_deleted(0).unwrap();
        assert_eq!(graph.is_deleted(0), Some(true));
        assert_eq!(graph.active_count(), 2);

        // Search: should NOT return node 0
        let query = FixedPointVector::from_f32_slice(&[0.1, 0.0, 0.0]).unwrap();
        let results = graph.search(&query, 3, 10).unwrap();
        assert!(results.iter().all(|(id, _)| *id != 0), "Deleted node should not appear in results");

        // Undelete
        graph.undelete(0).unwrap();
        assert_eq!(graph.is_deleted(0), Some(false));
        assert_eq!(graph.active_count(), 3);
    }

    #[test]
    fn test_delete_invalid_id() {
        let graph: HnswGraph<3> = HnswGraph::new();
        assert!(graph.is_deleted(999).is_none());
    }
}
