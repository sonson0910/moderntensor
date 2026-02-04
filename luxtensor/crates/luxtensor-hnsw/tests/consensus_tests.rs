//! Integration tests for HNSW consensus safety
//!
//! These tests verify that the HNSW implementation produces identical results
//! across multiple independent runs, ensuring consensus safety in a blockchain
//! environment.

use luxtensor_hnsw::{DeterministicRng, FixedPointVector, HnswGraph};

/// Test that two independent HNSW graphs built with the same seed
/// produce identical structures (consensus fork detection).
#[test]
fn test_consensus_identical_graphs() {
    // Same seeds for both graphs
    let tx_hash = [1u8; 32];
    let block_hash = [2u8; 32];

    // Build first graph
    let mut graph1: HnswGraph<128> = HnswGraph::new();
    let mut rng1 = DeterministicRng::new(tx_hash, block_hash);

    // Build second graph with same parameters
    let mut graph2: HnswGraph<128> = HnswGraph::new();
    let mut rng2 = DeterministicRng::new(tx_hash, block_hash);

    // Generate 100 vectors deterministically
    let vectors: Vec<FixedPointVector<128>> = (0..100)
        .map(|i| {
            let mut values = [0.0f32; 128];
            for j in 0..128 {
                values[j] = ((i * 128 + j) as f32 / 12800.0) - 0.5;
            }
            FixedPointVector::from_f32_slice(&values).unwrap()
        })
        .collect();

    // Insert into both graphs
    for vector in &vectors {
        graph1.insert(vector.clone(), &mut rng1);
        graph2.insert(vector.clone(), &mut rng2);
    }

    // Verify both graphs have same structure
    assert_eq!(graph1.len(), graph2.len(), "Graphs should have same size");
    assert_eq!(
        graph1.max_level(),
        graph2.max_level(),
        "Graphs should have same max level"
    );

    // Serialize both and compare
    let bytes1 = graph1.serialize().expect("Serialization should succeed");
    let bytes2 = graph2.serialize().expect("Serialization should succeed");

    assert_eq!(
        bytes1, bytes2,
        "Serialized graphs should be byte-identical (consensus safe)"
    );
}

/// Test that different seeds produce different graphs (sanity check).
#[test]
fn test_different_seeds_different_graphs() {
    let tx_hash1 = [1u8; 32];
    let block_hash1 = [2u8; 32];

    let tx_hash2 = [3u8; 32]; // Different!
    let block_hash2 = [4u8; 32];

    let mut graph1: HnswGraph<64> = HnswGraph::new();
    let mut rng1 = DeterministicRng::new(tx_hash1, block_hash1);

    let mut graph2: HnswGraph<64> = HnswGraph::new();
    let mut rng2 = DeterministicRng::new(tx_hash2, block_hash2);

    // Insert same vectors with different seeds
    let vectors: Vec<FixedPointVector<64>> = (0..50)
        .map(|i| {
            let mut values = [0.0f32; 64];
            for j in 0..64 {
                values[j] = ((i * 64 + j) as f32 / 3200.0) - 0.5;
            }
            FixedPointVector::from_f32_slice(&values).unwrap()
        })
        .collect();

    for vector in &vectors {
        graph1.insert(vector.clone(), &mut rng1);
        graph2.insert(vector.clone(), &mut rng2);
    }

    // Serialize and compare - should be different
    let bytes1 = graph1.serialize().expect("Serialization should succeed");
    let bytes2 = graph2.serialize().expect("Serialization should succeed");

    // They should have the same size but different content (different node levels)
    assert_eq!(graph1.len(), graph2.len());
    assert_ne!(
        bytes1, bytes2,
        "Different seeds should produce different graphs"
    );
}

/// Test search consistency - same query should return same results
#[test]
fn test_search_consistency() {
    let tx_hash = [42u8; 32];
    let block_hash = [43u8; 32];

    let mut graph: HnswGraph<32> = HnswGraph::new();
    let mut rng = DeterministicRng::new(tx_hash, block_hash);

    // Insert vectors
    for i in 0..100 {
        let mut values = [0.0f32; 32];
        for j in 0..32 {
            values[j] = ((i * 32 + j) as f32 / 3200.0) - 0.5;
        }
        graph.insert(FixedPointVector::from_f32_slice(&values).unwrap(), &mut rng);
    }

    // Create query vector
    let query_values = [0.1f32; 32];
    let query = FixedPointVector::from_f32_slice(&query_values).unwrap();

    // Search multiple times - should get identical results
    let ef_search = 100;
    let k = 10;

    let results1 = graph.search(&query, k, ef_search).expect("Search should succeed");
    let results2 = graph.search(&query, k, ef_search).expect("Search should succeed");
    let results3 = graph.search(&query, k, ef_search).expect("Search should succeed");

    assert_eq!(results1.len(), results2.len());
    assert_eq!(results2.len(), results3.len());

    // Check exact same results
    for i in 0..results1.len() {
        assert_eq!(results1[i].0, results2[i].0, "Node IDs should match");
        assert_eq!(results1[i].1, results2[i].1, "Distances should match exactly");
        assert_eq!(results2[i].0, results3[i].0);
        assert_eq!(results2[i].1, results3[i].1);
    }
}

/// Test serialization roundtrip preserves exact graph structure
#[test]
fn test_serialization_determinism() {
    let tx_hash = [100u8; 32];
    let block_hash = [101u8; 32];

    let mut graph: HnswGraph<16> = HnswGraph::new();
    let mut rng = DeterministicRng::new(tx_hash, block_hash);

    // Insert vectors
    for i in 0..50 {
        let mut values = [0.0f32; 16];
        for j in 0..16 {
            values[j] = (i as f32 + j as f32) / 100.0;
        }
        graph.insert(FixedPointVector::from_f32_slice(&values).unwrap(), &mut rng);
    }

    // Serialize
    let bytes = graph.serialize().expect("Serialization should succeed");

    // Deserialize
    let restored: HnswGraph<16> =
        HnswGraph::deserialize(&bytes).expect("Deserialization should succeed");

    // Serialize again
    let bytes2 = restored.serialize().expect("Re-serialization should succeed");

    // Should be byte-identical
    assert_eq!(
        bytes, bytes2,
        "Serialize -> Deserialize -> Serialize should be idempotent"
    );

    // Search should return same results
    let query_values = [0.25f32; 16];
    let query = FixedPointVector::from_f32_slice(&query_values).unwrap();

    let results_original = graph.search(&query, 5, 50).expect("Search should succeed");
    let results_restored = restored.search(&query, 5, 50).expect("Search should succeed");

    assert_eq!(results_original, results_restored, "Search results should be identical after restore");
}

/// Test that fixed-point calculations are bit-exact
#[test]
fn test_fixed_point_bit_exactness() {
    // Test distance calculation determinism
    let v1: FixedPointVector<4> =
        FixedPointVector::from_f32_slice(&[1.0, 2.0, 3.0, 4.0]).unwrap();
    let v2: FixedPointVector<4> =
        FixedPointVector::from_f32_slice(&[4.0, 3.0, 2.0, 1.0]).unwrap();

    let dist1 = v1.squared_distance(&v2);
    let dist2 = v1.squared_distance(&v2);

    assert_eq!(
        dist1.to_bits(),
        dist2.to_bits(),
        "Distance calculations should be bit-exact"
    );

    // Also verify dot product is deterministic
    let dot1 = v1.dot(&v2);
    let dot2 = v1.dot(&v2);

    assert_eq!(
        dot1.to_bits(),
        dot2.to_bits(),
        "Dot product should be bit-exact"
    );
}

/// Stress test: Large graph with many insertions
#[test]
fn test_large_graph_determinism() {
    let tx_hash = [200u8; 32];
    let block_hash = [201u8; 32];

    // Build two large graphs with smaller ef for speed
    let mut graph1: HnswGraph<256> = HnswGraph::with_params(luxtensor_hnsw::ml(), 100);
    let mut rng1 = DeterministicRng::new(tx_hash, block_hash);

    let mut graph2: HnswGraph<256> = HnswGraph::with_params(luxtensor_hnsw::ml(), 100);
    let mut rng2 = DeterministicRng::new(tx_hash, block_hash);

    // Insert 500 vectors
    for i in 0..500 {
        let mut values = [0.0f32; 256];
        for j in 0..256 {
            // Use a deterministic pseudo-random pattern
            values[j] = ((i * 7 + j * 13) % 1000) as f32 / 1000.0 - 0.5;
        }
        let vector = FixedPointVector::from_f32_slice(&values).unwrap();
        graph1.insert(vector.clone(), &mut rng1);
        graph2.insert(vector, &mut rng2);
    }

    // Verify structures match
    assert_eq!(graph1.len(), graph2.len());

    // Serialize and compare (this is the ultimate determinism test)
    let bytes1 = graph1.serialize().unwrap();
    let bytes2 = graph2.serialize().unwrap();

    assert_eq!(
        bytes1, bytes2,
        "Large graphs should be byte-identical with same seed"
    );
}

/// Test cross-run determinism by verifying search results
#[test]
fn test_cross_run_search_determinism() {
    // Simulate what would happen across different validator nodes
    let tx_hash = [50u8; 32];
    let block_hash = [51u8; 32];

    // "Node A" builds graph
    let mut graph_a: HnswGraph<64> = HnswGraph::new();
    let mut rng_a = DeterministicRng::new(tx_hash, block_hash);

    // "Node B" builds graph independently
    let mut graph_b: HnswGraph<64> = HnswGraph::new();
    let mut rng_b = DeterministicRng::new(tx_hash, block_hash);

    // Same insertions in same order (canonical ordering)
    for i in 0..200 {
        let mut values = [0.0f32; 64];
        for j in 0..64 {
            values[j] = ((i + j) as f32).sin() * 0.5;
        }
        let vector = FixedPointVector::from_f32_slice(&values).unwrap();
        graph_a.insert(vector.clone(), &mut rng_a);
        graph_b.insert(vector, &mut rng_b);
    }

    // Same query on both nodes
    let query_values: Vec<f32> = (0..64).map(|i| (i as f32 * 0.1).cos()).collect();
    let query = FixedPointVector::from_f32_slice(&query_values).unwrap();

    let results_a = graph_a.search(&query, 20, 100).unwrap();
    let results_b = graph_b.search(&query, 20, 100).unwrap();

    // CRITICAL: Both nodes must return IDENTICAL results for consensus
    assert_eq!(results_a.len(), results_b.len(), "Result count must match");

    for (i, (a, b)) in results_a.iter().zip(results_b.iter()).enumerate() {
        assert_eq!(a.0, b.0, "Node ID at position {} must match", i);
        assert_eq!(a.1, b.1, "Distance at position {} must match (bit-exact)", i);
    }
}
