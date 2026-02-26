//! Benchmarks for HNSW performance
//!
//! Run with: cargo bench -p luxtensor-hnsw

use std::time::{Duration, Instant};
use luxtensor_hnsw::{DeterministicRng, FixedPointVector, HnswGraph};

/// Benchmark results struct
struct BenchResult {
    name: String,
    iterations: usize,
    total_time: Duration,
    avg_time: Duration,
    ops_per_sec: f64,
}

impl BenchResult {
    fn print(&self) {
        println!(
            "📊 {} | {} iterations | Total: {:?} | Avg: {:?} | {:.2} ops/sec",
            self.name, self.iterations, self.total_time, self.avg_time, self.ops_per_sec
        );
    }
}

fn benchmark<F>(name: &str, iterations: usize, mut f: F) -> BenchResult
where
    F: FnMut(),
{
    // Warmup
    for _ in 0..3 {
        f();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let total_time = start.elapsed();
    let avg_time = total_time / iterations as u32;
    let ops_per_sec = iterations as f64 / total_time.as_secs_f64();

    BenchResult {
        name: name.to_string(),
        iterations,
        total_time,
        avg_time,
        ops_per_sec,
    }
}

/// Generate random vectors deterministically
fn generate_vectors<const D: usize>(count: usize, seed: u8) -> Vec<FixedPointVector<D>> {
    (0..count)
        .map(|i| {
            let mut values = vec![0.0f32; D];
            for j in 0..D {
                // Pseudo-random based on seed, i, j
                values[j] = (((seed as usize * 7 + i * 13 + j * 17) % 1000) as f32 / 1000.0) - 0.5;
            }
            FixedPointVector::from_f32_slice(&values).unwrap()
        })
        .collect()
}

#[test]
fn bench_insert_64dim() {
    println!("\n🚀 HNSW INSERT BENCHMARK (64 dimensions)\n");

    let vectors: Vec<FixedPointVector<64>> = generate_vectors(1000, 1);
    let tx_hash = [1u8; 32];
    let block_hash = [2u8; 32];

    let result = benchmark("Insert 1000 vectors (64-dim)", 1, || {
        let mut graph: HnswGraph<64> = HnswGraph::new();
        let mut rng = DeterministicRng::new(tx_hash, block_hash);
        for v in &vectors {
            graph.insert(v.clone(), &mut rng).unwrap();
        }
    });

    result.print();

    // Per-insert timing
    let per_insert_us = result.avg_time.as_micros() as f64 / 1000.0;
    println!("   ⏱️  Per insert: {:.2} μs", per_insert_us);
}

#[test]
fn bench_insert_256dim() {
    println!("\n🚀 HNSW INSERT BENCHMARK (256 dimensions)\n");

    let vectors: Vec<FixedPointVector<256>> = generate_vectors(500, 2);
    let tx_hash = [1u8; 32];
    let block_hash = [2u8; 32];

    let result = benchmark("Insert 500 vectors (256-dim)", 1, || {
        let mut graph: HnswGraph<256> = HnswGraph::new();
        let mut rng = DeterministicRng::new(tx_hash, block_hash);
        for v in &vectors {
            graph.insert(v.clone(), &mut rng).unwrap();
        }
    });

    result.print();
    let per_insert_us = result.avg_time.as_micros() as f64 / 500.0;
    println!("   ⏱️  Per insert: {:.2} μs", per_insert_us);
}

#[test]
fn bench_insert_768dim() {
    println!("\n🚀 HNSW INSERT BENCHMARK (768 dimensions - transformer standard)\n");

    let vectors: Vec<FixedPointVector<768>> = generate_vectors(200, 3);
    let tx_hash = [1u8; 32];
    let block_hash = [2u8; 32];

    let result = benchmark("Insert 200 vectors (768-dim)", 1, || {
        let mut graph: HnswGraph<768> = HnswGraph::new();
        let mut rng = DeterministicRng::new(tx_hash, block_hash);
        for v in &vectors {
            graph.insert(v.clone(), &mut rng).unwrap();
        }
    });

    result.print();
    let per_insert_us = result.avg_time.as_micros() as f64 / 200.0;
    println!("   ⏱️  Per insert: {:.2} μs", per_insert_us);
}

#[test]
fn bench_search_64dim() {
    println!("\n🔍 HNSW SEARCH BENCHMARK (64 dimensions)\n");

    // Build graph first
    let vectors: Vec<FixedPointVector<64>> = generate_vectors(1000, 1);
    let tx_hash = [1u8; 32];
    let block_hash = [2u8; 32];

    let mut graph: HnswGraph<64> = HnswGraph::new();
    let mut rng = DeterministicRng::new(tx_hash, block_hash);
    for v in &vectors {
        graph.insert(v.clone(), &mut rng).unwrap();
    }

    // Prepare query
    let query = generate_vectors::<64>(1, 99)[0].clone();

    let result = benchmark("Search k=10, ef=100 (1000 vectors, 64-dim)", 100, || {
        let _ = graph.search(&query, 10, 100);
    });

    result.print();
}

#[test]
fn bench_search_768dim() {
    println!("\n🔍 HNSW SEARCH BENCHMARK (768 dimensions)\n");

    // Build graph first
    let vectors: Vec<FixedPointVector<768>> = generate_vectors(500, 1);
    let tx_hash = [1u8; 32];
    let block_hash = [2u8; 32];

    let mut graph: HnswGraph<768> = HnswGraph::new();
    let mut rng = DeterministicRng::new(tx_hash, block_hash);
    for v in &vectors {
        graph.insert(v.clone(), &mut rng).unwrap();
    }

    // Prepare query
    let query = generate_vectors::<768>(1, 99)[0].clone();

    let result = benchmark("Search k=10, ef=100 (500 vectors, 768-dim)", 50, || {
        let _ = graph.search(&query, 10, 100);
    });

    result.print();
}

#[test]
fn bench_serialization() {
    println!("\n💾 HNSW SERIALIZATION BENCHMARK\n");

    // Build graph
    let vectors: Vec<FixedPointVector<128>> = generate_vectors(1000, 1);
    let tx_hash = [1u8; 32];
    let block_hash = [2u8; 32];

    let mut graph: HnswGraph<128> = HnswGraph::new();
    let mut rng = DeterministicRng::new(tx_hash, block_hash);
    for v in &vectors {
        graph.insert(v.clone(), &mut rng).unwrap();
    }

    // Benchmark serialize
    let result = benchmark("Serialize 1000 vectors (128-dim)", 10, || {
        let _ = graph.serialize().unwrap();
    });
    result.print();

    let bytes = graph.serialize().unwrap();
    println!("   📦 Serialized size: {} KB", bytes.len() / 1024);

    // Benchmark deserialize
    let result = benchmark("Deserialize 1000 vectors (128-dim)", 10, || {
        let _: HnswGraph<128> = HnswGraph::deserialize(&bytes).unwrap();
    });
    result.print();
}

#[test]
fn bench_distance_calculation() {
    println!("\n📐 DISTANCE CALCULATION BENCHMARK\n");

    let v1: FixedPointVector<768> = generate_vectors(1, 1)[0].clone();
    let v2: FixedPointVector<768> = generate_vectors(1, 2)[0].clone();

    let result = benchmark("Squared distance (768-dim)", 10000, || {
        let _ = v1.squared_distance(&v2);
    });
    result.print();

    let result = benchmark("Dot product (768-dim)", 10000, || {
        let _ = v1.dot(&v2);
    });
    result.print();
}

#[test]
fn bench_full_pipeline() {
    println!("\n");
    println!("═══════════════════════════════════════════════════════════════");
    println!("                    HNSW FULL BENCHMARK SUITE                    ");
    println!("═══════════════════════════════════════════════════════════════");

    // Test different dimensions
    for dim_label in &["64-dim", "256-dim", "768-dim"] {
        println!("\n📊 Testing {} vectors...\n", dim_label);
    }

    // 768-dim full test (most realistic for AI)
    let vectors: Vec<FixedPointVector<768>> = generate_vectors(100, 1);
    let tx_hash = [1u8; 32];
    let block_hash = [2u8; 32];

    println!("📌 Building graph with 100 vectors (768-dim)...");
    let start = Instant::now();
    let mut graph: HnswGraph<768> = HnswGraph::new();
    let mut rng = DeterministicRng::new(tx_hash, block_hash);
    for v in &vectors {
        graph.insert(v.clone(), &mut rng).unwrap();
    }
    let build_time = start.elapsed();
    println!("   ✅ Build time: {:?}", build_time);
    println!("   ✅ Per insert: {:?}", build_time / 100);

    // Search benchmark
    let query = generate_vectors::<768>(1, 99)[0].clone();
    let start = Instant::now();
    for _ in 0..100 {
        let _ = graph.search(&query, 10, 100);
    }
    let search_time = start.elapsed();
    println!("   ✅ 100 searches: {:?}", search_time);
    println!("   ✅ Per search: {:?}", search_time / 100);

    // Serialization
    let start = Instant::now();
    let bytes = graph.serialize().unwrap();
    let ser_time = start.elapsed();
    println!("   ✅ Serialize: {:?} ({} KB)", ser_time, bytes.len() / 1024);

    let start = Instant::now();
    let _: HnswGraph<768> = HnswGraph::deserialize(&bytes).unwrap();
    let deser_time = start.elapsed();
    println!("   ✅ Deserialize: {:?}", deser_time);

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                         SUMMARY                                 ");
    println!("═══════════════════════════════════════════════════════════════");
    println!("   Insert (768-dim):    {:?} per vector", build_time / 100);
    println!("   Search (768-dim):    {:?} per query", search_time / 100);
    println!("   Throughput:          {:.0} inserts/sec, {:.0} searches/sec",
        100.0 / build_time.as_secs_f64(),
        100.0 / search_time.as_secs_f64(),
    );
    println!("═══════════════════════════════════════════════════════════════\n");
}
