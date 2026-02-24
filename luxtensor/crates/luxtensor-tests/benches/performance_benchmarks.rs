// Performance benchmarks for LuxTensor blockchain
// Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use luxtensor_core::{Account, Address, Block, Transaction};
use luxtensor_crypto::{keccak256, blake3_hash, KeyPair};
use luxtensor_storage::{BlockchainDB, StateDB};
use std::sync::Arc;
use tempfile::TempDir;

fn benchmark_block_validation(c: &mut Criterion) {
    let genesis = Block::genesis();

    c.bench_function("block_hash", |b| {
        b.iter(|| {
            black_box(genesis.hash());
        });
    });

    c.bench_function("block_height", |b| {
        b.iter(|| {
            black_box(genesis.height());
        });
    });
}

fn benchmark_transaction_operations(c: &mut Criterion) {
    let keypair = KeyPair::generate();
    let address = Address::from(keypair.address());

    c.bench_function("transaction_create", |b| {
        b.iter(|| {
            black_box(Transaction::new(
                0,
                address,
                Some(address),
                1000,
                1,
                21000,
                vec![],
            ));
        });
    });

    let tx = Transaction::new(
        0,
        address,
        Some(address),
        1000,
        1,
        21000,
        vec![],
    );

    c.bench_function("transaction_hash", |b| {
        b.iter(|| {
            black_box(tx.hash());
        });
    });
}

fn benchmark_cryptography(c: &mut Criterion) {
    let data = b"Hello, LuxTensor blockchain!";

    c.bench_function("keccak256", |b| {
        b.iter(|| {
            black_box(keccak256(data));
        });
    });

    c.bench_function("blake3", |b| {
        b.iter(|| {
            black_box(blake3_hash(data));
        });
    });

    c.bench_function("keypair_generate", |b| {
        b.iter(|| {
            black_box(KeyPair::generate());
        });
    });

    let keypair = KeyPair::generate();
    let message = [0u8; 32];

    c.bench_function("sign_message", |b| {
        b.iter(|| {
            black_box(keypair.sign(&message));
        });
    });
}

fn benchmark_state_operations(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("bench_db");
    let storage = Arc::new(BlockchainDB::open(&db_path).unwrap());
    let state_db = Arc::new(StateDB::new(storage.inner_db()));

    let keypair = KeyPair::generate();
    let address = Address::from(keypair.address());

    let account = Account {
        nonce: 0,
        balance: 1_000_000_000_000_000_000,
        storage_root: [0u8; 32],
        code_hash: [0u8; 32],
        code: None,
    };

    state_db.set_account(address, account.clone());
    state_db.commit().unwrap();

    c.bench_function("state_get_account", |b| {
        b.iter(|| {
            black_box(state_db.get_account(&address).unwrap());
        });
    });

    c.bench_function("state_set_account", |b| {
        b.iter(|| {
            state_db.set_account(address, account.clone());
        });
    });

    c.bench_function("state_get_balance", |b| {
        b.iter(|| {
            black_box(state_db.get_balance(&address).unwrap());
        });
    });

    c.bench_function("state_get_nonce", |b| {
        b.iter(|| {
            black_box(state_db.get_nonce(&address).unwrap());
        });
    });
}

fn benchmark_storage_operations(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("bench_storage");
    let storage = Arc::new(BlockchainDB::open(&db_path).unwrap());

    let genesis = Block::genesis();
    storage.store_block(&genesis).unwrap();

    c.bench_function("storage_store_block", |b| {
        let mut height = 1u64;
        b.iter(|| {
            let header = luxtensor_core::BlockHeader {
                version: 1,
                height,
                timestamp: 0,
                previous_hash: genesis.hash(),
                state_root: [0u8; 32],
                txs_root: [0u8; 32],
                receipts_root: [0u8; 32],
                validator: [0u8; 32],
                signature: vec![0u8; 64],
                gas_used: 0,
                gas_limit: 10_000_000,
                extra_data: vec![],
                vrf_proof: None,
            };
            let block = Block::new(header, vec![]);
            black_box(storage.store_block(&block).unwrap());
            height += 1;
        });
    });

    c.bench_function("storage_get_block", |b| {
        b.iter(|| {
            black_box(storage.get_block(&genesis.hash()).unwrap());
        });
    });

    c.bench_function("storage_get_block_by_height", |b| {
        b.iter(|| {
            black_box(storage.get_block_by_height(0).unwrap());
        });
    });
}

fn benchmark_transaction_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_throughput");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let keypair = KeyPair::generate();
                let address = Address::from(keypair.address());

                let mut transactions = Vec::new();
                for i in 0..size {
                    let tx = Transaction::new(
                        i,
                        address,
                        Some(address),
                        1000,
                        1,
                        21000,
                        vec![],
                    );
                    transactions.push(tx);
                }

                black_box(transactions);
            });
        });
    }

    group.finish();
}

fn benchmark_block_creation_with_transactions(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_with_transactions");

    for tx_count in [0, 10, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(tx_count),
            tx_count,
            |b, &tx_count| {
                let keypair = KeyPair::generate();
                let address = Address::from(keypair.address());

                b.iter(|| {
                    let mut transactions = Vec::new();
                    for i in 0..tx_count {
                        let tx = Transaction::new(
                            i,
                            address,
                            Some(address),
                            1000,
                            1,
                            21000,
                            vec![],
                        );
                        transactions.push(tx);
                    }

                    let header = luxtensor_core::BlockHeader {
                        version: 1,
                        height: 1,
                        timestamp: 0,
                        previous_hash: [0u8; 32],
                        state_root: [0u8; 32],
                        txs_root: [0u8; 32],
                        receipts_root: [0u8; 32],
                        validator: [0u8; 32],
                        signature: vec![0u8; 64],
                        gas_used: 0,
                        gas_limit: 10_000_000,
                        extra_data: vec![],
                        vrf_proof: None,
                    };

                    let block = Block::new(header, transactions);
                    black_box(block);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_parallel_state_reads(c: &mut Criterion) {
    use std::thread;

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("bench_parallel");
    let storage = Arc::new(BlockchainDB::open(&db_path).unwrap());
    let state_db = Arc::new(StateDB::new(storage.inner_db()));

    // Setup: Create multiple accounts
    let mut addresses = Vec::new();
    for i in 0..100 {
        let keypair = KeyPair::generate();
        let address = Address::from(keypair.address());
        addresses.push(address);

        let account = Account {
            nonce: i,
            balance: (i as u128 + 1) * 1_000_000_000,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        };
        state_db.set_account(address, account);
    }
    state_db.commit().unwrap();

    c.bench_function("parallel_state_reads", |b| {
        b.iter(|| {
            let mut handles = vec![];

            for addr in addresses.iter().take(10) {
                let state_db_clone = state_db.clone();
                let address = *addr;

                let handle = thread::spawn(move || {
                    state_db_clone.get_account(&address).unwrap()
                });

                handles.push(handle);
            }

            for handle in handles {
                black_box(handle.join().unwrap());
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_block_validation,
    benchmark_transaction_operations,
    benchmark_cryptography,
    benchmark_state_operations,
    benchmark_storage_operations,
    benchmark_transaction_throughput,
    benchmark_block_creation_with_transactions,
    benchmark_parallel_state_reads,
);

criterion_main!(benches);
