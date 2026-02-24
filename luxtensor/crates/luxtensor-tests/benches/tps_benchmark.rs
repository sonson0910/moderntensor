// TPS (Transactions Per Second) Benchmark for LuxTensor
//
// Measures ACTUAL transaction processing throughput using in-process components.
// Run with: cargo bench --bench tps_benchmark
//
// Benchmark groups:
// 1. Transaction creation throughput
// 2. Transaction signing throughput
// 3. Block assembly throughput
// 4. StateDB read/write throughput
// 5. Mempool throughput (simulated in-process)
// 6. End-to-end pipeline: create + sign + mempool + block production

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use luxtensor_core::{Account, Address, Block, BlockHeader, Transaction};
use luxtensor_crypto::{keccak256, KeyPair};
use std::collections::HashMap;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Default devnet chain_id (matches Transaction::new default)
#[allow(dead_code)]
const CHAIN_ID: u64 = 8898;

/// Create a fresh keypair + address pair.
fn new_identity() -> (KeyPair, Address) {
    let kp = KeyPair::generate();
    let addr = Address::from(kp.address());
    (kp, addr)
}

/// Create unsigned test transaction.
fn make_unsigned_tx(nonce: u64, from: Address, to: Address) -> Transaction {
    Transaction::new(nonce, from, Some(to), 1_000, 1_000_000_000, 21_000, vec![])
}

/// Sign a transaction in-place and return it.
fn sign_transaction(tx: &mut Transaction, keypair: &KeyPair) {
    let msg_hash = keccak256(&tx.signing_message());
    if let Ok(sig) = keypair.sign(&msg_hash) {
        tx.r.copy_from_slice(&sig[..32]);
        tx.s.copy_from_slice(&sig[32..]);
        tx.v = 0; // recovery id
    }
}

/// Build a block from a vector of transactions.
fn assemble_block(height: u64, prev_hash: [u8; 32], txs: Vec<Transaction>) -> Block {
    let gas_used: u64 = txs.iter().map(|t| t.gas_limit).sum();
    let header = BlockHeader {
        version: 1,
        height,
        timestamp: 0,
        previous_hash: prev_hash,
        state_root: [0u8; 32],
        txs_root: [0u8; 32],
        receipts_root: [0u8; 32],
        validator: [0u8; 32],
        signature: vec![0u8; 64],
        gas_used,
        gas_limit: gas_used.max(30_000_000), // ensure fits
        extra_data: vec![],
        vrf_proof: None,
    };
    Block::new(header, txs)
}

// ---------------------------------------------------------------------------
// Lightweight in-process mempool (mirrors luxtensor-node Mempool behaviour)
// ---------------------------------------------------------------------------

struct BenchMempool {
    txs: HashMap<[u8; 32], Transaction>,
    max_size: usize,
}

impl BenchMempool {
    fn new(max_size: usize) -> Self {
        Self { txs: HashMap::with_capacity(max_size), max_size }
    }

    fn add(&mut self, tx: Transaction) -> bool {
        if self.txs.len() >= self.max_size {
            return false;
        }
        let hash = tx.hash();
        self.txs.insert(hash, tx);
        true
    }

    fn drain_for_block(&mut self, limit: usize) -> Vec<Transaction> {
        let mut out = Vec::with_capacity(limit);
        let keys: Vec<[u8; 32]> = self.txs.keys().copied().take(limit).collect();
        for k in keys {
            if let Some(tx) = self.txs.remove(&k) {
                out.push(tx);
            }
        }
        out
    }

    fn len(&self) -> usize {
        self.txs.len()
    }
}

// ---------------------------------------------------------------------------
// In-memory state helper (wraps luxtensor_core::StateDB)
// ---------------------------------------------------------------------------

fn new_state_with_accounts(n: usize) -> (luxtensor_core::StateDB, Vec<Address>) {
    let mut state = luxtensor_core::StateDB::new();
    let mut addrs = Vec::with_capacity(n);
    for i in 0..n {
        let (_, addr) = new_identity();
        let acct = Account {
            nonce: 0,
            balance: (i as u128 + 1) * 1_000_000_000_000_000_000,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        };
        state.set_account(addr, acct);
        addrs.push(addr);
    }
    (state, addrs)
}

// ===========================================================================
// 1. Transaction creation throughput
// ===========================================================================

fn bench_tx_creation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("tps_tx_creation");

    for &count in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("create_txs", count), &count, |b, &n| {
            let (_, addr) = new_identity();
            b.iter(|| {
                let mut txs = Vec::with_capacity(n);
                for i in 0..n as u64 {
                    txs.push(make_unsigned_tx(i, addr, addr));
                }
                black_box(&txs);
            });
        });
    }

    // Print effective TPS for 1000 txs
    {
        let (_, addr) = new_identity();
        let start = Instant::now();
        let iters = 100u64;
        let count = 1000u64;
        for _ in 0..iters {
            let mut txs = Vec::with_capacity(count as usize);
            for i in 0..count {
                txs.push(make_unsigned_tx(i, addr, addr));
            }
            black_box(&txs);
        }
        let elapsed = start.elapsed();
        let total = iters * count;
        let tps = total as f64 / elapsed.as_secs_f64();
        eprintln!(
            "\n[TPS] tx_creation_throughput: {:.0} txs/sec ({} txs in {:.3}s)\n",
            tps,
            total,
            elapsed.as_secs_f64()
        );
    }

    group.finish();
}

// ===========================================================================
// 2. Transaction signing throughput
// ===========================================================================

fn bench_tx_signing_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("tps_tx_signing");

    for &count in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("sign_txs", count), &count, |b, &n| {
            let (kp, addr) = new_identity();
            // Pre-create unsigned transactions
            let unsigned: Vec<Transaction> =
                (0..n as u64).map(|i| make_unsigned_tx(i, addr, addr)).collect();

            b.iter(|| {
                let mut txs = unsigned.clone();
                for tx in txs.iter_mut() {
                    sign_transaction(tx, &kp);
                }
                black_box(&txs);
            });
        });
    }

    // Print effective TPS for 1000 signed txs
    {
        let (kp, addr) = new_identity();
        let unsigned: Vec<Transaction> =
            (0..1000u64).map(|i| make_unsigned_tx(i, addr, addr)).collect();
        let start = Instant::now();
        let iters = 10u64;
        for _ in 0..iters {
            let mut txs = unsigned.clone();
            for tx in txs.iter_mut() {
                sign_transaction(tx, &kp);
            }
            black_box(&txs);
        }
        let elapsed = start.elapsed();
        let total = iters * 1000;
        let tps = total as f64 / elapsed.as_secs_f64();
        eprintln!(
            "\n[TPS] tx_signing_throughput: {:.0} txs/sec ({} txs in {:.3}s)\n",
            tps,
            total,
            elapsed.as_secs_f64()
        );
    }

    group.finish();
}

// ===========================================================================
// 3. Block assembly throughput
// ===========================================================================

fn bench_block_assembly_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("tps_block_assembly");

    for &tx_count in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("block_assemble", tx_count), &tx_count, |b, &n| {
            let (kp, addr) = new_identity();
            // Pre-build signed txs
            let txs: Vec<Transaction> = (0..n as u64)
                .map(|i| {
                    let mut tx = make_unsigned_tx(i, addr, addr);
                    sign_transaction(&mut tx, &kp);
                    tx
                })
                .collect();

            b.iter(|| {
                let block = assemble_block(1, [0u8; 32], txs.clone());
                // Also compute block hash to simulate real work
                black_box(block.hash());
            });
        });
    }

    // Print effective TPS for 1000 tx block
    {
        let (kp, addr) = new_identity();
        let txs: Vec<Transaction> = (0..1000u64)
            .map(|i| {
                let mut tx = make_unsigned_tx(i, addr, addr);
                sign_transaction(&mut tx, &kp);
                tx
            })
            .collect();
        let start = Instant::now();
        let iters = 50u64;
        for _ in 0..iters {
            let block = assemble_block(1, [0u8; 32], txs.clone());
            black_box(block.hash());
        }
        let elapsed = start.elapsed();
        let total = iters * 1000;
        let tps = total as f64 / elapsed.as_secs_f64();
        eprintln!(
            "\n[TPS] block_assembly_throughput (1000 txs/block): {:.0} txs/sec ({} txs in {:.3}s)\n",
            tps, total, elapsed.as_secs_f64()
        );
    }

    group.finish();
}

// ===========================================================================
// 4. StateDB write throughput
// ===========================================================================

fn bench_statedb_write_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("tps_statedb_write");

    for &count in &[1000, 5000, 10000] {
        group.bench_with_input(BenchmarkId::new("state_write", count), &count, |b, &n| {
            // Pre-generate addresses
            let addrs: Vec<Address> = (0..n).map(|_| new_identity().1).collect();

            b.iter(|| {
                let mut state = luxtensor_core::StateDB::new();
                for (i, &addr) in addrs.iter().enumerate() {
                    state.set_account(
                        addr,
                        Account {
                            nonce: i as u64,
                            balance: (i as u128 + 1) * 1_000_000_000,
                            storage_root: [0u8; 32],
                            code_hash: [0u8; 32],
                            code: None,
                        },
                    );
                }
                black_box(state.account_count());
            });
        });
    }

    // Print effective TPS
    {
        let n = 10_000usize;
        let addrs: Vec<Address> = (0..n).map(|_| new_identity().1).collect();
        let start = Instant::now();
        let iters = 10u64;
        for _ in 0..iters {
            let mut state = luxtensor_core::StateDB::new();
            for (i, &addr) in addrs.iter().enumerate() {
                state.set_account(
                    addr,
                    Account {
                        nonce: i as u64,
                        balance: (i as u128 + 1) * 1_000_000_000,
                        storage_root: [0u8; 32],
                        code_hash: [0u8; 32],
                        code: None,
                    },
                );
            }
            black_box(state.account_count());
        }
        let elapsed = start.elapsed();
        let total = iters as usize * n;
        let ops = total as f64 / elapsed.as_secs_f64();
        eprintln!(
            "\n[TPS] statedb_write_throughput: {:.0} ops/sec ({} writes in {:.3}s)\n",
            ops,
            total,
            elapsed.as_secs_f64()
        );
    }

    group.finish();
}

// ===========================================================================
// 5. StateDB read throughput
// ===========================================================================

fn bench_statedb_read_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("tps_statedb_read");

    for &count in &[1000, 5000, 10000] {
        group.bench_with_input(BenchmarkId::new("state_read", count), &count, |b, &n| {
            let (state, addrs) = new_state_with_accounts(n);

            b.iter(|| {
                for addr in &addrs {
                    black_box(state.get_account(addr));
                }
            });
        });
    }

    // Print effective TPS
    {
        let n = 10_000usize;
        let (state, addrs) = new_state_with_accounts(n);
        let start = Instant::now();
        let iters = 100u64;
        for _ in 0..iters {
            for addr in &addrs {
                black_box(state.get_account(addr));
            }
        }
        let elapsed = start.elapsed();
        let total = iters as usize * n;
        let ops = total as f64 / elapsed.as_secs_f64();
        eprintln!(
            "\n[TPS] statedb_read_throughput: {:.0} ops/sec ({} reads in {:.3}s)\n",
            ops,
            total,
            elapsed.as_secs_f64()
        );
    }

    group.finish();
}

// ===========================================================================
// 6. Mempool throughput
// ===========================================================================

fn bench_mempool_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("tps_mempool");

    for &count in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("mempool_add", count), &count, |b, &n| {
            let (_, addr) = new_identity();
            // Pre-create transactions
            let txs: Vec<Transaction> =
                (0..n as u64).map(|i| make_unsigned_tx(i, addr, addr)).collect();

            b.iter(|| {
                let mut pool = BenchMempool::new(n + 100);
                for tx in txs.iter() {
                    pool.add(tx.clone());
                }
                black_box(pool.len());
            });
        });
    }

    // Print effective TPS for 1000 txs
    {
        let (_, addr) = new_identity();
        let txs: Vec<Transaction> = (0..1000u64).map(|i| make_unsigned_tx(i, addr, addr)).collect();
        let start = Instant::now();
        let iters = 100u64;
        for _ in 0..iters {
            let mut pool = BenchMempool::new(1100);
            for tx in txs.iter() {
                pool.add(tx.clone());
            }
            black_box(pool.len());
        }
        let elapsed = start.elapsed();
        let total = iters * 1000;
        let tps = total as f64 / elapsed.as_secs_f64();
        eprintln!(
            "\n[TPS] mempool_throughput: {:.0} txs/sec ({} txs in {:.3}s)\n",
            tps,
            total,
            elapsed.as_secs_f64()
        );
    }

    group.finish();
}

// ===========================================================================
// 7. End-to-end TPS: create → sign → mempool → block assembly
// ===========================================================================

fn bench_end_to_end_tps(c: &mut Criterion) {
    let mut group = c.benchmark_group("tps_end_to_end");

    for &tx_count in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("e2e_pipeline", tx_count), &tx_count, |b, &n| {
            b.iter(|| {
                let (kp, addr) = new_identity();

                // 1. Create transactions
                let mut txs: Vec<Transaction> =
                    (0..n as u64).map(|i| make_unsigned_tx(i, addr, addr)).collect();

                // 2. Sign transactions
                for tx in txs.iter_mut() {
                    sign_transaction(tx, &kp);
                }

                // 3. Add to mempool
                let mut pool = BenchMempool::new(n + 100);
                for tx in txs {
                    pool.add(tx);
                }

                // 4. Drain mempool and assemble block
                let block_txs = pool.drain_for_block(n);
                let block = assemble_block(1, [0u8; 32], block_txs);

                // 5. Compute block hash (finalization)
                black_box(block.hash());
            });
        });
    }

    // Print effective E2E TPS for 1000 txs
    {
        let start = Instant::now();
        let iters = 10u64;
        let count = 1000usize;
        for _ in 0..iters {
            let (kp, addr) = new_identity();

            let mut txs: Vec<Transaction> =
                (0..count as u64).map(|i| make_unsigned_tx(i, addr, addr)).collect();
            for tx in txs.iter_mut() {
                sign_transaction(tx, &kp);
            }

            let mut pool = BenchMempool::new(count + 100);
            for tx in txs {
                pool.add(tx);
            }

            let block_txs = pool.drain_for_block(count);
            let block = assemble_block(1, [0u8; 32], block_txs);
            black_box(block.hash());
        }
        let elapsed = start.elapsed();
        let total = iters as usize * count;
        let tps = total as f64 / elapsed.as_secs_f64();
        eprintln!(
            "\n[TPS] end_to_end (create+sign+mempool+block): {:.0} txs/sec ({} txs in {:.3}s)\n",
            tps,
            total,
            elapsed.as_secs_f64()
        );
    }

    group.finish();
}

// ===========================================================================
// 8. State commit (Merkle root) throughput
// ===========================================================================

fn bench_state_commit_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("tps_state_commit");

    for &count in &[100, 1000, 5000] {
        group.bench_with_input(BenchmarkId::new("state_commit", count), &count, |b, &n| {
            let (mut state, _) = new_state_with_accounts(n);

            b.iter(|| {
                black_box(state.commit().unwrap());
            });
        });
    }

    // Print effective TPS
    {
        let n = 5000usize;
        let (mut state, _) = new_state_with_accounts(n);
        let start = Instant::now();
        let iters = 20u64;
        for _ in 0..iters {
            black_box(state.commit().unwrap());
        }
        let elapsed = start.elapsed();
        let commits_per_sec = iters as f64 / elapsed.as_secs_f64();
        eprintln!(
            "\n[TPS] state_commit_throughput ({} accounts): {:.1} commits/sec ({} commits in {:.3}s)\n",
            n, commits_per_sec, iters, elapsed.as_secs_f64()
        );
    }

    group.finish();
}

// ===========================================================================
// Criterion configuration & main
// ===========================================================================

criterion_group! {
    name = tps_benches;
    config = Criterion::default()
        .sample_size(10)           // fewer samples for heavy benchmarks
        .measurement_time(std::time::Duration::from_secs(10))
        .warm_up_time(std::time::Duration::from_secs(3));
    targets =
        bench_tx_creation_throughput,
        bench_tx_signing_throughput,
        bench_block_assembly_throughput,
        bench_statedb_write_throughput,
        bench_statedb_read_throughput,
        bench_mempool_throughput,
        bench_end_to_end_tps,
        bench_state_commit_throughput,
}

criterion_main!(tps_benches);
