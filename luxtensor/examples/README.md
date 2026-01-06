# LuxTensor Examples

This directory contains working examples demonstrating LuxTensor features.

## Running Examples

```bash
# Build all examples
cargo build --examples

# Run transaction example
cargo run --example full_transaction_example

# Run smart contract example
cargo run --example smart_contract_example
```

## Examples

### 1. Full Transaction Example (`full_transaction_example.rs`)

Demonstrates end-to-end transaction processing:
- Database initialization
- Account creation with balances
- Transaction creation and execution
- Block creation and storage
- State persistence

### 2. Smart Contract Example (`smart_contract_example.rs`)

Shows smart contract functionality:
- Contract deployment with gas metering
- Contract storage management
- Contract method calls
- Statistics tracking

## Adding New Examples

1. Create a new `.rs` file in this directory
2. Add it to the examples in the workspace or crate `Cargo.toml`
3. Run with `cargo run --example <filename>`

## Documentation

For complete API documentation:
```bash
cargo doc --open
```
