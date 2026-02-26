//! Comprehensive stress tests, recall benchmarks, and adversarial tests
//! for the LuxTensor HNSW vector store implementation.
//!
//! Fast tests: `cargo test`
//! All tests (including slow): `cargo test -- --ignored`
//! Specific test: `cargo test test_insert_1000_vectors`

#[cfg(test)]
mod tests {
    use crate::deterministic_rng::DeterministicRng;
    use crate::fixed_point::{FixedPointVector, I64F32};
    use crate::graph::HnswGraph;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::thread;

    // ═══════════════════════════════════════════════════════════════════
    // Helper Functions
    // ═══════════════════════════════════════════════════════════════════

    /// Generate a single deterministic random FixedPointVector<D>.
    fn generate_fixed_vector<const D: usize>(rng: &mut StdRng) -> FixedPointVector<D> {
        let components: Vec<f32> = (0..D).map(|_| rng.gen_range(-10.0f32..10.0f32)).collect();
        FixedPointVector::from_f32_slice(&components).unwrap()
    }

    /// Generate N deterministic random FixedPointVectors with a given seed.
    fn generate_random_vectors<const D: usize>(
        n: usize,
        seed: [u8; 32],
    ) -> Vec<FixedPointVector<D>> {
        let mut rng = StdRng::from_seed(seed);
        (0..n).map(|_| generate_fixed_vector(&mut rng)).collect()
    }

    /// Brute-force exact k-nearest neighbors using squared Euclidean distance.
    fn brute_force_knn<const D: usize>(
        query: &FixedPointVector<D>,
        dataset: &[FixedPointVector<D>],
        k: usize,
    ) -> Vec<(usize, I64F32)> {
        let mut distances: Vec<(usize, I64F32)> = dataset
            .iter()
            .enumerate()
            .map(|(i, v)| (i, query.squared_distance(v)))
            .collect();
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        distances.truncate(k);
        distances
    }

    /// Calculate recall@k: fraction of true nearest neighbors found by HNSW.
    fn calculate_recall(
        hnsw_results: &[(usize, I64F32)],
        exact_results: &[(usize, I64F32)],
    ) -> f64 {
        let hnsw_ids: HashSet<usize> = hnsw_results.iter().map(|(id, _)| *id).collect();
        let exact_ids: HashSet<usize> = exact_results.iter().map(|(id, _)| *id).collect();
        let intersection = hnsw_ids.intersection(&exact_ids).count();
        if exact_ids.is_empty() {
            return 1.0;
        }
        intersection as f64 / exact_ids.len() as f64
    }

    /// Create a DeterministicRng with the standard test seed.
    fn create_test_rng() -> DeterministicRng {
        DeterministicRng::from_seed([42u8; 32])
    }

    /// Build an HnswGraph from a vector dataset and return the graph.
    fn build_graph<const D: usize>(
        vectors: &[FixedPointVector<D>],
        rng: &mut DeterministicRng,
    ) -> HnswGraph<D> {
        let mut graph = HnswGraph::new();
        for v in vectors {
            graph.insert(v.clone(), rng).unwrap();
        }
        graph
    }

    // ═══════════════════════════════════════════════════════════════════
    // 1. Scale Tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_insert_1000_vectors() {
        let vectors = generate_random_vectors::<4>(1_000, [1u8; 32]);
        let mut rng = create_test_rng();
        let graph = build_graph(&vectors, &mut rng);

        assert_eq!(graph.len(), 1_000);

        // Verify all vectors are searchable
        let mut search_rng = StdRng::from_seed([77u8; 32]);
        for _ in 0..20 {
            let idx = search_rng.gen_range(0..1_000);
            let query = &vectors[idx];
            let results = graph.search(query, 5, 50).unwrap();
            assert!(!results.is_empty(), "Search returned no results for vector {}", idx);
            // The query vector itself should be the nearest neighbor (distance ~ 0)
            assert_eq!(
                results[0].0, idx,
                "Nearest neighbor of vector {} should be itself, got {}",
                idx, results[0].0
            );
        }
    }

    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_insert_10000_vectors() {
        let vectors = generate_random_vectors::<4>(10_000, [2u8; 32]);
        let mut rng = create_test_rng();
        let graph = build_graph(&vectors, &mut rng);

        assert_eq!(graph.len(), 10_000);

        // Verify graph integrity: entry point exists, max_level > 0 for large graphs
        assert!(graph.max_level() > 0, "10K graph should have multiple levels");

        // Spot-check searchability
        let mut search_rng = StdRng::from_seed([88u8; 32]);
        for _ in 0..50 {
            let idx = search_rng.gen_range(0..10_000);
            let results = graph.search(&vectors[idx], 10, 100).unwrap();
            assert!(results.len() <= 10);
            assert!(!results.is_empty());
            // The vector itself should appear in the results
            let result_ids: HashSet<usize> = results.iter().map(|(id, _)| *id).collect();
            assert!(
                result_ids.contains(&idx),
                "Vector {} not found in its own search results",
                idx
            );
        }
    }

    #[test]
    fn test_high_dimension_128() {
        let vectors = generate_random_vectors::<128>(200, [3u8; 32]);
        let mut rng = create_test_rng();
        let graph = build_graph(&vectors, &mut rng);

        assert_eq!(graph.len(), 200);

        // Search with a known vector
        let results = graph.search(&vectors[0], 5, 50).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].0, 0, "Nearest to vector 0 should be itself");
    }

    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_high_dimension_512() {
        let vectors = generate_random_vectors::<512>(50, [4u8; 32]);
        let mut rng = create_test_rng();
        let graph = build_graph(&vectors, &mut rng);

        assert_eq!(graph.len(), 50);

        // Verify search works in high-dimensional space
        let results = graph.search(&vectors[10], 5, 50).unwrap();
        assert!(!results.is_empty());
        // The query vector should be among the nearest neighbors
        let result_ids: HashSet<usize> = results.iter().map(|(id, _)| *id).collect();
        assert!(result_ids.contains(&10));
    }

    // ═══════════════════════════════════════════════════════════════════
    // 2. Recall Accuracy Tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_recall_at_10() {
        let vectors = generate_random_vectors::<8>(500, [10u8; 32]);
        let mut rng = create_test_rng();
        let graph = build_graph(&vectors, &mut rng);

        // Generate 20 random query vectors (disjoint from dataset via different seed)
        let queries = generate_random_vectors::<8>(20, [11u8; 32]);

        let k = 10;
        let ef = 100;
        let mut total_recall = 0.0;

        for query in &queries {
            let exact = brute_force_knn(query, &vectors, k);
            let hnsw = graph.search(query, k, ef).unwrap();
            let recall = calculate_recall(&hnsw, &exact);
            total_recall += recall;
        }

        let avg_recall = total_recall / queries.len() as f64;
        assert!(
            avg_recall > 0.5,
            "Average recall@{} should be > 0.5, got {:.4}",
            k,
            avg_recall,
        );
    }

    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_recall_vs_ef_parameter() {
        let vectors = generate_random_vectors::<16>(1_000, [20u8; 32]);
        let mut rng = create_test_rng();
        let graph = build_graph(&vectors, &mut rng);

        let queries = generate_random_vectors::<16>(30, [21u8; 32]);
        let k = 10;
        let ef_values = [10, 50, 100, 200];
        let mut recalls = Vec::new();

        for &ef in &ef_values {
            let mut total_recall = 0.0;
            for query in &queries {
                let exact = brute_force_knn(query, &vectors, k);
                let hnsw = graph.search(query, k, ef).unwrap();
                let recall = calculate_recall(&hnsw, &exact);
                total_recall += recall;
            }
            let avg_recall = total_recall / queries.len() as f64;
            recalls.push(avg_recall);
        }

        // Higher ef should generally give equal or better recall
        // Check that max-ef recall is at least as good as min-ef recall
        let first_recall = recalls[0];
        let last_recall = recalls[recalls.len() - 1];
        assert!(
            last_recall >= first_recall - 0.05,
            "Recall at ef={} ({:.4}) should be >= recall at ef={} ({:.4}) (with tolerance)",
            ef_values[ef_values.len() - 1],
            last_recall,
            ef_values[0],
            first_recall,
        );

        // All recalls should be above a minimum threshold
        for (i, &recall) in recalls.iter().enumerate() {
            assert!(
                recall > 0.3,
                "Recall at ef={} should be > 0.3, got {:.4}",
                ef_values[i],
                recall,
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // 3. Determinism Tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_determinism_1000_vectors() {
        // Same inputs must produce byte-identical graphs (consensus safety)
        let vectors = generate_random_vectors::<4>(1_000, [30u8; 32]);

        let mut rng1 = DeterministicRng::from_seed([42u8; 32]);
        let mut rng2 = DeterministicRng::from_seed([42u8; 32]);

        let graph1 = build_graph(&vectors, &mut rng1);
        let graph2 = build_graph(&vectors, &mut rng2);

        let bytes1 = graph1.serialize().unwrap();
        let bytes2 = graph2.serialize().unwrap();

        assert_eq!(
            bytes1.len(),
            bytes2.len(),
            "Serialized graph sizes differ"
        );
        assert_eq!(
            bytes1, bytes2,
            "Graphs built with same seed and data must be byte-identical"
        );
    }

    #[test]
    fn test_determinism_across_insertion_order() {
        // HNSW is insertion-order-dependent, so same order + same RNG = same result.
        // Different order should produce different graphs.
        let vectors = generate_random_vectors::<4>(100, [31u8; 32]);

        // Build graph in forward order
        let mut rng_fwd = DeterministicRng::from_seed([50u8; 32]);
        let graph_fwd = build_graph(&vectors, &mut rng_fwd);
        let bytes_fwd = graph_fwd.serialize().unwrap();

        // Build same graph in forward order again — must be identical
        let mut rng_fwd2 = DeterministicRng::from_seed([50u8; 32]);
        let graph_fwd2 = build_graph(&vectors, &mut rng_fwd2);
        let bytes_fwd2 = graph_fwd2.serialize().unwrap();

        assert_eq!(
            bytes_fwd, bytes_fwd2,
            "Same insertion order + same RNG seed must produce identical graphs"
        );

        // Build graph in reverse order — should differ (different topology)
        let reversed: Vec<FixedPointVector<4>> = vectors.iter().rev().cloned().collect();
        let mut rng_rev = DeterministicRng::from_seed([50u8; 32]);
        let graph_rev = build_graph(&reversed, &mut rng_rev);
        let bytes_rev = graph_rev.serialize().unwrap();

        // Reversed order should generally produce a different graph
        // (not guaranteed for trivial cases, but very likely for 100 vectors)
        assert_ne!(
            bytes_fwd, bytes_rev,
            "Reversed insertion order should produce a different graph topology"
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    // 4. Adversarial Tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_all_identical_vectors() {
        // Edge case: all vectors are identical — distances are all 0
        let v = FixedPointVector::<4>::from_f32_slice(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        let mut graph: HnswGraph<4> = HnswGraph::new();
        let mut rng = create_test_rng();

        for _ in 0..100 {
            graph.insert(v.clone(), &mut rng).unwrap();
        }

        assert_eq!(graph.len(), 100);

        // Search should return results without panicking
        let results = graph.search(&v, 10, 50).unwrap();
        assert!(!results.is_empty());
        assert!(results.len() <= 10);

        // All distances should be zero
        for (_id, dist) in &results {
            assert_eq!(
                *dist,
                I64F32::from_num(0),
                "Distance to identical vector should be 0"
            );
        }
    }

    #[test]
    fn test_zero_vectors() {
        // Edge case: all vectors are zero
        let mut graph: HnswGraph<4> = HnswGraph::new();
        let mut rng = create_test_rng();

        for _ in 0..50 {
            let v = FixedPointVector::<4>::zero();
            graph.insert(v, &mut rng).unwrap();
        }

        assert_eq!(graph.len(), 50);

        let query = FixedPointVector::<4>::zero();
        let results = graph.search(&query, 5, 20).unwrap();
        assert!(!results.is_empty());

        // All distances should be zero
        for (_id, dist) in &results {
            assert_eq!(*dist, I64F32::from_num(0));
        }
    }

    #[test]
    fn test_extreme_values() {
        // Test with very large fixed-point values — saturating arithmetic should prevent panics
        let mut graph: HnswGraph<3> = HnswGraph::new();
        let mut rng = create_test_rng();

        // Large positive values (use 1000.0 which is well within I64F32 range)
        let v_large = FixedPointVector::<3>::new([
            I64F32::from_num(1000),
            I64F32::from_num(1000),
            I64F32::from_num(1000),
        ]);
        graph.insert(v_large.clone(), &mut rng).unwrap();

        // Large negative values
        let v_neg = FixedPointVector::<3>::new([
            I64F32::from_num(-1000),
            I64F32::from_num(-1000),
            I64F32::from_num(-1000),
        ]);
        graph.insert(v_neg, &mut rng).unwrap();

        // Near-max values: I64F32 max is ~2^31 ≈ 2.1 billion
        // Use moderately large values to stress saturating arithmetic
        let v_max = FixedPointVector::<3>::new([
            I64F32::from_num(100_000),
            I64F32::from_num(100_000),
            I64F32::from_num(100_000),
        ]);
        graph.insert(v_max, &mut rng).unwrap();

        let v_min = FixedPointVector::<3>::new([
            I64F32::from_num(-100_000),
            I64F32::from_num(-100_000),
            I64F32::from_num(-100_000),
        ]);
        graph.insert(v_min, &mut rng).unwrap();

        assert_eq!(graph.len(), 4);

        // Search should work without panicking
        let results = graph.search(&v_large, 3, 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_single_dimension() {
        // Edge case: dim=1, simplest possible space
        let mut graph: HnswGraph<1> = HnswGraph::new();
        let mut rng = create_test_rng();

        let values: Vec<f32> = (0..50).map(|i| i as f32 * 0.5).collect();
        let vectors: Vec<FixedPointVector<1>> = values
            .iter()
            .map(|&v| FixedPointVector::from_f32_slice(&[v]).unwrap())
            .collect();

        for v in &vectors {
            graph.insert(v.clone(), &mut rng).unwrap();
        }

        assert_eq!(graph.len(), 50);

        // Query for value 5.0 — nearest should be index 10 (5.0/0.5 = 10)
        let query = FixedPointVector::<1>::from_f32_slice(&[5.0]).unwrap();
        let results = graph.search(&query, 3, 20).unwrap();
        assert!(!results.is_empty());

        // The nearest neighbor should be index 10 (value 5.0)
        assert_eq!(results[0].0, 10, "Nearest to 5.0 should be vector at index 10");
    }

    #[test]
    fn test_duplicate_insert() {
        // Same vector inserted multiple times
        let mut graph: HnswGraph<3> = HnswGraph::new();
        let mut rng = create_test_rng();

        let v = FixedPointVector::<3>::from_f32_slice(&[1.0, 2.0, 3.0]).unwrap();
        let id1 = graph.insert(v.clone(), &mut rng).unwrap();
        let id2 = graph.insert(v.clone(), &mut rng).unwrap();

        assert_ne!(id1, id2, "Duplicate inserts should get different IDs");
        assert_eq!(graph.len(), 2);

        // Both nodes should exist
        assert!(graph.get_node(id1).is_some());
        assert!(graph.get_node(id2).is_some());

        // Search should return both (distance 0)
        let results = graph.search(&v, 2, 10).unwrap();
        assert_eq!(results.len(), 2);
        let result_ids: HashSet<usize> = results.iter().map(|(id, _)| *id).collect();
        assert!(result_ids.contains(&id1));
        assert!(result_ids.contains(&id2));
    }

    // ═══════════════════════════════════════════════════════════════════
    // 5. Serialization Stress Tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_serialize_deserialize_10000() {
        let vectors = generate_random_vectors::<4>(10_000, [40u8; 32]);
        let mut rng = create_test_rng();
        let graph = build_graph(&vectors, &mut rng);

        // Serialize
        let bytes = graph.serialize().unwrap();
        assert!(bytes.len() > 0, "Serialized graph should not be empty");

        // Deserialize
        let restored: HnswGraph<4> = HnswGraph::deserialize(&bytes).unwrap();
        assert_eq!(graph.len(), restored.len());
        assert_eq!(graph.max_level(), restored.max_level());

        // Re-serialize and compare (roundtrip fidelity)
        let bytes2 = restored.serialize().unwrap();
        assert_eq!(
            bytes, bytes2,
            "Serialization roundtrip must produce identical bytes"
        );
    }

    #[test]
    fn test_search_after_deserialize() {
        let vectors = generate_random_vectors::<4>(500, [41u8; 32]);
        let mut rng = create_test_rng();
        let graph = build_graph(&vectors, &mut rng);

        // Search before serialization
        let queries = generate_random_vectors::<4>(10, [42u8; 32]);
        let k = 5;
        let ef = 50;

        let results_before: Vec<Vec<(usize, I64F32)>> = queries
            .iter()
            .map(|q| graph.search(q, k, ef).unwrap())
            .collect();

        // Serialize and deserialize
        let bytes = graph.serialize().unwrap();
        let restored: HnswGraph<4> = HnswGraph::deserialize(&bytes).unwrap();

        // Search after deserialization — results must be identical
        let results_after: Vec<Vec<(usize, I64F32)>> = queries
            .iter()
            .map(|q| restored.search(q, k, ef).unwrap())
            .collect();

        for (i, (before, after)) in results_before.iter().zip(results_after.iter()).enumerate() {
            assert_eq!(
                before.len(),
                after.len(),
                "Query {} result count differs after deserialization",
                i,
            );
            for (j, (b, a)) in before.iter().zip(after.iter()).enumerate() {
                assert_eq!(
                    b.0, a.0,
                    "Query {} result {} node ID differs after deserialization",
                    i, j,
                );
                assert_eq!(
                    b.1, a.1,
                    "Query {} result {} distance differs after deserialization",
                    i, j,
                );
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // 6. Concurrent Access Test
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_parallel_search() {
        // Build a graph and search from multiple threads concurrently.
        // Since search takes &self, this should be safe.
        let vectors = generate_random_vectors::<4>(500, [50u8; 32]);
        let mut rng = create_test_rng();
        let graph = build_graph(&vectors, &mut rng);
        let graph = Arc::new(graph);

        let num_threads = 8;
        let queries_per_thread = 20;
        let query_vectors = generate_random_vectors::<4>(
            num_threads * queries_per_thread,
            [51u8; 32],
        );

        let mut handles = Vec::new();
        for t in 0..num_threads {
            let g = Arc::clone(&graph);
            let thread_queries: Vec<FixedPointVector<4>> = query_vectors
                [t * queries_per_thread..(t + 1) * queries_per_thread]
                .to_vec();
            handles.push(thread::spawn(move || {
                for query in &thread_queries {
                    let results = g.search(query, 5, 50).unwrap();
                    assert!(!results.is_empty(), "Parallel search returned no results");
                }
            }));
        }

        // All threads must complete without error
        for (i, handle) in handles.into_iter().enumerate() {
            handle
                .join()
                .unwrap_or_else(|_| panic!("Thread {} panicked during parallel search", i));
        }
    }
}
