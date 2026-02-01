# E2E Full Network Test

Comprehensive end-to-end testing suite for ModernTensor/Luxtensor network.

## Quick Start

```bash
# 1. Build the binary
cd luxtensor
cargo build --release

# 2. Install Python dependencies
pip install requests

# 3. Run the test
python scripts/e2e_full_test.py
```

## What It Tests

| Test Group | Description |
|------------|-------------|
| **Connectivity** | All 3 nodes respond to RPC |
| **Block Production** | Validator produces blocks |
| **System Health** | Health endpoint works |
| **Checkpoint System** | Checkpoint RPC endpoints |
| **Staking** | Validator queries |
| **Edge Cases** | Error handling |

## Network Topology

```
┌─────────────────┐
│   Validator A   │ ◄── Block Producer
│  P2P: 30300     │
│  RPC: 9000      │
└────────┬────────┘
         │ mDNS Discovery
    ┌────┴────┐
    ▼         ▼
┌────────┐  ┌────────┐
│ Miner B│  │ Miner C│
│ P:30301│  │ P:30302│
│ R:9001 │  │ R:9002 │
└────────┘  └────────┘
```

## Output

```
✅ Testnet is running!

📋 Test Group: Connectivity
  ✅ PASS: validator-a_responds
  ✅ PASS: miner-b_responds
  ✅ PASS: miner-c_responds

📊 TEST RESULTS
Results: 12/12 passed, 0 failed
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Binary not found | Run `cargo build --release` first |
| Port in use | Kill existing processes on 30300-30302, 9000-9002 |
| Timeout | Increase `STARTUP_WAIT` in script |
