# DEPRECATED CARDANO INTEGRATION

## Files Removed

The following Cardano-specific files have been removed as part of the Layer 1 blockchain focus:

### Removed Directories
- `sdk/compat/` - Cardano compatibility layer
- `sdk/bridge/` - Cardano validator bridge  
- `sdk/smartcontract/` - Plutus smart contracts

### Removed Files
- `sdk/service/stake_service.py` - Old Cardano staking (replaced by `sdk/blockchain/l1_staking_service.py`)
- `sdk/cli/stake_cli.py` - Old Cardano staking CLI (replaced by `sdk/cli/l1_stake_cli.py`)

## Affected Files (Cardano Integration Deprecated)

The following files contain Cardano-specific code that has been commented out or deprecated:

- `sdk/agent/miner_agent.py` - Cardano miner agent (deprecated)
- `sdk/consensus/node.py` - Contains some Cardano references
- `sdk/cli/wallet_cli.py` - Contains some Cardano wallet operations
- `sdk/cli/query_cli.py` - Contains some Cardano query operations

## Migration Path

For Layer 1 blockchain functionality, use:

### Instead of Old Cardano Staking:
```bash
# OLD (removed):
mtcli stake add --coldkey my_coldkey --hotkey validator_hk ...

# NEW (use this):
mtcli l1-stake add --address <hex> --private-key <hex> --amount 1000000
```

### For Blockchain Operations:
- Use `sdk.blockchain.*` modules for Layer 1 blockchain
- Use `sdk.consensus.pos` for PoS consensus
- Use `sdk.blockchain.l1_staking_service` for staking

### For State Management:
- Use `sdk.blockchain.state.StateDB` for state management
- Use `sdk.blockchain.transaction.StakingTransaction` for staking transactions

## Reason for Removal

ModernTensor is building a native Layer 1 blockchain (like Subtensor/Bittensor) and does not require Cardano integration. The removed files were part of an earlier architecture that used Cardano as the base layer.

The new architecture is:
- Native Layer 1 blockchain with PoS consensus
- Independent from any external blockchain
- Full control over consensus, staking, and tokenomics
- 83% complete with all essential components

## Test Impact

Approximately 14 test files were using the Cardano compatibility layer. These tests may need to be updated or removed:

- `tests/metagraph/test_*.py` - Metagraph tests using Cardano
- `tests/consensus/test_*.py` - Some consensus tests using Cardano
- `tests/service/test_*.py` - Service tests using Cardano
- `tests/keymanager/test_*.py` - Some keymanager tests using Cardano

Tests for Layer 1 functionality are in:
- `tests/blockchain/test_l1_staking.py` - Layer 1 staking tests (14 tests, all passing)
- Other `tests/blockchain/` tests for Layer 1 blockchain

## Future Work

1. Update or remove Cardano-dependent tests
2. Complete migration of remaining Cardano references in:
   - `sdk/agent/miner_agent.py`
   - `sdk/consensus/node.py`
   - `sdk/cli/wallet_cli.py`
   - `sdk/cli/query_cli.py`
3. Remove or update `sdk/service/` directory for Layer 1 equivalents

## Questions?

See `LAYER1_CLEANUP_PLAN.md` for detailed analysis of what was removed and why.
