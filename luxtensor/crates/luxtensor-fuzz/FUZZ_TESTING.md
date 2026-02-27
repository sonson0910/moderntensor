# Fuzz Testing — LuxTensor

## Overview

The `luxtensor-fuzz` crate contains **7 fuzz targets** that exercise critical parsing
and validation paths. The crate uses `cargo-fuzz` (libFuzzer backend) and lives in a
**separate workspace** (`[workspace] members = ["."]` in its own `Cargo.toml`) to
avoid interfering with the main workspace build.

## Targets

| Target              | File                               | What it fuzzes                          |
|---------------------|------------------------------------|-----------------------------------------|
| `tx_parser`         | `fuzz_targets/tx_parser.rs`        | Transaction deserialization / decoding  |
| `block_validator`   | `fuzz_targets/block_validator.rs`  | Block header + body structural checks  |
| `rpc_input`         | `fuzz_targets/rpc_input.rs`        | JSON-RPC request parsing boundary      |
| `address_parser`    | `fuzz_targets/address_parser.rs`   | Ethereum address hex parsing           |
| `consensus_message` | `fuzz_targets/consensus_message.rs`| Consensus message decode / validate    |
| `keccak256`          | `fuzz_targets/keccak256.rs`        | Keccak256 hash determinism             |
| `value_parser`       | `fuzz_targets/value_parser.rs`     | Numeric overflow / arithmetic safety   |

## Running Locally

### Prerequisites

```bash
# Install cargo-fuzz (requires nightly Rust)
cargo install cargo-fuzz
rustup install nightly
```

### Run a single target

```bash
cd luxtensor/crates/luxtensor-fuzz
cargo +nightly fuzz run tx_parser            # runs until stopped (Ctrl+C)
cargo +nightly fuzz run tx_parser -- -max_total_time=300   # 5-minute run
```

### Run all targets (quick smoke test)

```powershell
# PowerShell (Windows)
$targets = @("tx_parser", "block_validator", "rpc_input", "address_parser", "consensus_message", "keccak256", "value_parser")
foreach ($t in $targets) {
    Write-Host "=== Fuzzing $t ===" -ForegroundColor Cyan
    cargo +nightly fuzz run $t -- -max_total_time=60
}
```

### List available targets

```bash
cargo +nightly fuzz list
```

## CI Integration (Future)

`cargo-fuzz` requires **nightly Rust** and is CPU-intensive.
Recommended CI approach:

1. **Nightly-only scheduled job** (e.g. weekly or nightly cron)
2. **Time-bounded** (`-max_total_time=300` per target, ≈25 min total)
3. **Corpus caching** between runs via CI artifact storage

### Example GitHub Actions snippet

```yaml
# .github/workflows/fuzz.yml
name: Fuzz Testing
on:
  schedule:
    - cron: '0 3 * * 1'  # Weekly on Monday 3 AM UTC
  workflow_dispatch:      # Manual trigger

jobs:
  fuzz:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [tx_parser, block_validator, rpc_input, address_parser, consensus_message]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz
      - name: Restore corpus
        uses: actions/cache@v4
        with:
          path: luxtensor/crates/luxtensor-fuzz/corpus/${{ matrix.target }}
          key: fuzz-corpus-${{ matrix.target }}-${{ github.sha }}
          restore-keys: fuzz-corpus-${{ matrix.target }}-
      - name: Run fuzzer
        run: |
          cd luxtensor/crates/luxtensor-fuzz
          cargo +nightly fuzz run ${{ matrix.target }} -- -max_total_time=300
      - name: Upload crash artifacts
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: fuzz-crashes-${{ matrix.target }}
          path: luxtensor/crates/luxtensor-fuzz/artifacts/${{ matrix.target }}/
```

## Current Status

- ✅ 7 fuzz targets implemented (tx_parser, block_validator, rpc_input, address_parser, consensus_message, keccak256, value_parser)
- ✅ All targets build with `cargo +nightly fuzz build`
- ✅ Fuzz logic lives in `luxtensor-tests/src/fuzz_targets.rs` (247 lines, 8 functions)
- ⏳ **Not yet integrated into CI** — runs are local-only
- 📋 Corpus files stored in `corpus/<target>/` (gitignored)
