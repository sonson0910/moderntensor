// Stress Tests and Benchmarks for LuxTensor
// Run with: cargo bench --package luxtensor-tests --bench stress_tests

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;

// ============================================================
// RPC call benchmarks (requires running node)
// ============================================================

fn bench_eth_block_number(c: &mut Criterion) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create client");

    c.bench_function("eth_blockNumber", |b| {
        b.iter(|| {
            let body = r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#;
            let _ = client.post("http://localhost:8545")
                .header("Content-Type", "application/json")
                .body(body)
                .send();
        });
    });
}

fn bench_eth_get_block_by_number(c: &mut Criterion) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create client");

    c.bench_function("eth_getBlockByNumber", |b| {
        b.iter(|| {
            let body = r#"{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["latest",false],"id":1}"#;
            let _ = client.post("http://localhost:8545")
                .header("Content-Type", "application/json")
                .body(body)
                .send();
        });
    });
}

fn bench_eth_get_balance(c: &mut Criterion) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create client");

    c.bench_function("eth_getBalance", |b| {
        b.iter(|| {
            let body = r#"{"jsonrpc":"2.0","method":"eth_getBalance","params":["0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","latest"],"id":1}"#;
            let _ = client.post("http://localhost:8545")
                .header("Content-Type", "application/json")
                .body(body)
                .send();
        });
    });
}

// ============================================================
// Transaction throughput benchmark
// ============================================================

fn bench_tx_throughput(c: &mut Criterion) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create client");

    let mut group = c.benchmark_group("tx_throughput");
    group.sample_size(10); // Fewer samples for TX tests
    group.measurement_time(Duration::from_secs(20));

    for batch_size in [1, 5, 10].iter() {
        group.bench_with_input(BenchmarkId::new("batch", batch_size), batch_size, |b, &size| {
            b.iter(|| {
                for i in 0..size {
                    let value = format!("0x{:x}", 1000 + i * 100);
                    let body = format!(
                        r#"{{"jsonrpc":"2.0","method":"eth_sendTransaction","params":[{{"from":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","to":"0x70997970C51812dc3A010C7d01b50e0d17dc79C8","value":"{}","gas":"0x5208"}}],"id":{}}}"#,
                        value, i + 1
                    );
                    let _ = client.post("http://localhost:8545")
                        .header("Content-Type", "application/json")
                        .body(body)
                        .send();
                }
            });
        });
    }

    group.finish();
}

// ============================================================
// Concurrent request benchmark
// ============================================================

fn bench_concurrent_requests(c: &mut Criterion) {
    use std::sync::Arc;

    let client = Arc::new(
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create client")
    );

    let mut group = c.benchmark_group("concurrent_requests");
    group.sample_size(10);

    for num_threads in [2, 4, 8].iter() {
        group.bench_with_input(BenchmarkId::new("threads", num_threads), num_threads, |b, &threads| {
            b.iter(|| {
                let mut handles = Vec::new();

                for _ in 0..threads {
                    let client = Arc::clone(&client);
                    handles.push(std::thread::spawn(move || {
                        let body = r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#;
                        let _ = client.post("http://localhost:8545")
                            .header("Content-Type", "application/json")
                            .body(body)
                            .send();
                    }));
                }

                for handle in handles {
                    let _ = handle.join();
                }
            });
        });
    }

    group.finish();
}

// ============================================================
// Offline benchmarks (no node required)
// ============================================================

fn bench_transaction_hash(c: &mut Criterion) {
    use luxtensor_core::{Transaction, Address};

    let from = Address::zero();
    let to = Some(Address::zero());

    c.bench_function("transaction_hash", |b| {
        b.iter(|| {
            let tx = Transaction::new(
                black_box(0),
                black_box(from),
                black_box(to),
                black_box(1000),
                black_box(1),
                black_box(21000),
                black_box(vec![]),
            );
            black_box(tx.hash())
        });
    });
}

fn bench_block_hash(c: &mut Criterion) {
    use luxtensor_core::Block;

    c.bench_function("block_hash", |b| {
        b.iter(|| {
            let block = Block::genesis();
            black_box(block.hash())
        });
    });
}

fn bench_keccak256(c: &mut Criterion) {
    use luxtensor_crypto::keccak256;

    let data_sizes = vec![32, 256, 1024, 4096];

    let mut group = c.benchmark_group("keccak256");

    for size in data_sizes {
        let data = vec![0u8; size];
        group.bench_with_input(BenchmarkId::new("bytes", size), &data, |b, data| {
            b.iter(|| black_box(keccak256(data)))
        });
    }

    group.finish();
}

fn bench_keypair_generation(c: &mut Criterion) {
    use luxtensor_crypto::KeyPair;

    c.bench_function("keypair_generation", |b| {
        b.iter(|| black_box(KeyPair::generate()))
    });
}

criterion_group!(
    benches,
    // RPC benchmarks (require running node)
    bench_eth_block_number,
    bench_eth_get_block_by_number,
    bench_eth_get_balance,
    bench_tx_throughput,
    bench_concurrent_requests,
    // Offline benchmarks
    bench_transaction_hash,
    bench_block_hash,
    bench_keccak256,
    bench_keypair_generation,
);

criterion_main!(benches);
