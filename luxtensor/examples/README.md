# LuxTensor Examples

This directory contains practical examples demonstrating the capabilities and features of the LuxTensor blockchain platform. These examples serve as both documentation and reference implementations for common blockchain operations.

## Quick Start

### Building Examples

```bash
# Build all examples in the workspace
cargo build --examples --release

# Build a specific example
cargo build --example full_transaction_example --release
```

### Running Examples

```bash
# Run the full transaction example
cargo run --example full_transaction_example

# Run the smart contract example
cargo run --example smart_contract_example

# Run the data synchronization demo
cargo run --example data_sync_demo
```

## Available Examples

### 1. Full Transaction Example

**File**: `full_transaction_example.rs`

This comprehensive example demonstrates the complete transaction lifecycle on LuxTensor:

**Features Demonstrated**:
- Blockchain database initialization with RocksDB
- Account creation and balance management
- Cryptographic key pair generation
- Transaction creation and signing
- Transaction execution and state updates
- Block creation and validation
- State root computation and persistence
- Genesis block initialization

**Key Concepts**:
- Account-based state model
- Nonce management for replay protection
- Balance transfers with validation
- State commitment with Merkle roots

**Use Case**: Understanding how transactions flow through the system from creation to finalization.

### 2. Smart Contract Example

**File**: `smart_contract_example.rs`

Demonstrates the smart contract execution environment and capabilities:

**Features Demonstrated**:
- Smart contract deployment process
- Gas metering and fee calculation
- Contract storage read/write operations
- Contract method invocation
- Execution statistics and monitoring
- Error handling and validation

**Key Concepts**:
- Contract lifecycle (deploy, execute, query)
- Gas-based resource accounting
- Contract state management
- Storage efficiency patterns

**Use Case**: Building and deploying decentralized applications on LuxTensor.

### 3. Data Synchronization Demo

**File**: `data_sync_demo.rs`

An interactive demonstration of blockchain data synchronization between nodes:

**Features Demonstrated**:
- Multi-node blockchain network setup
- Block propagation and synchronization
- State consistency across nodes
- Chain validation and verification
- Peer-to-peer data exchange patterns
- Consensus verification

**Key Concepts**:
- Distributed consensus
- Chain synchronization protocols
- State replication
- Network resilience

**Use Case**: Understanding how blockchain nodes maintain consistency in a distributed network.

## Example Output

Each example provides detailed console output showing:
- Step-by-step execution progress
- Key operations and their results
- Performance metrics where applicable
- Validation checkpoints
- Final state summaries

Example output includes colored terminal output (where supported) for better readability.

## Learning Path

We recommend exploring examples in this order:

1. **Full Transaction Example** - Learn the basics of blockchain operations
2. **Smart Contract Example** - Understand programmability and execution
3. **Data Sync Demo** - Explore distributed network behavior

## Extending Examples

### Creating Your Own Example

1. **Create a new file** in the `examples/` directory:
   ```bash
   touch examples/my_custom_example.rs
   ```

2. **Add the example** to `Cargo.toml`:
   ```toml
   [[example]]
   name = "my_custom_example"
   path = "examples/my_custom_example.rs"
   ```

3. **Implement your example** using LuxTensor crates:
   ```rust
   use luxtensor_core::{Block, Transaction, State};
   use luxtensor_crypto::{KeyPair, Hash};
   
   fn main() -> Result<(), Box<dyn std::error::Error>> {
       // Your example code here
       Ok(())
   }
   ```

4. **Run your example**:
   ```bash
   cargo run --example my_custom_example
   ```

### Best Practices for Examples

- **Clear documentation**: Explain what the example demonstrates
- **Step-by-step output**: Show progress at each major step
- **Error handling**: Use proper Result types and error propagation
- **Resource cleanup**: Use RAII and proper cleanup (e.g., TempDir for databases)
- **Comments**: Add explanatory comments for complex operations
- **Performance**: Consider using `--release` for performance-sensitive examples

## API Documentation

For comprehensive API documentation covering all available types and functions:

```bash
# Generate and open documentation in your browser
cargo doc --open --no-deps

# Include private items for internal understanding
cargo doc --open --document-private-items
```

## Troubleshooting

### Common Issues

**Issue**: Example fails to compile
```bash
# Solution: Ensure all dependencies are up to date
cargo update
cargo build --examples
```

**Issue**: Database errors or conflicts
```bash
# Solution: Examples use temporary directories that auto-clean
# If issues persist, manually clean temp directories
rm -rf /tmp/rust_tempfile_*
```

**Issue**: Performance seems slow
```bash
# Solution: Build in release mode for accurate performance
cargo run --example full_transaction_example --release
```

## Additional Resources

- **Core API Documentation**: Generated via `cargo doc`
- **Integration Tests**: See `crates/luxtensor-tests/` for more examples
- **Security Guide**: `SECURITY_AUDIT_SCRIPTS.md` for security best practices
- **Main Documentation**: Root `README.md` for overall project information

## Contributing Examples

We welcome contributions of new examples! If you have an example that demonstrates a useful pattern or feature:

1. Ensure it follows the structure of existing examples
2. Add clear documentation and comments
3. Test thoroughly in both debug and release modes
4. Submit a pull request with description of what the example demonstrates

Examples that would be particularly valuable:
- Advanced smart contract patterns
- Cross-contract interactions
- Complex transaction scenarios
- Network protocol demonstrations
- Performance optimization techniques
- Security best practices

---

**Note**: Examples use temporary storage and do not persist data between runs. For production node setup, refer to the main documentation.
