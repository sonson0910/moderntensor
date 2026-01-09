# ModernTensor CLI (mtcli) - Implementation Guide

## Overview

The ModernTensor CLI (`mtcli`) is a command-line interface for interacting with the Luxtensor blockchain, inspired by Bittensor's `btcli` but adapted specifically for ModernTensor's architecture.

## Architecture

```
sdk/cli/
├── __init__.py              # Package initialization
├── main.py                  # Main CLI entry point
├── config.py                # Configuration management
├── utils.py                 # Common utilities
└── commands/                # Command modules
    ├── __init__.py
    ├── wallet.py            # Wallet management
    ├── stake.py             # Staking operations
    ├── query.py             # Blockchain queries
    ├── tx.py                # Transactions
    ├── subnet.py            # Subnet management
    ├── validator.py         # Validator operations
    └── utils.py             # Utility commands

sdk/keymanager/              # Key management (new)
├── __init__.py
├── key_generator.py         # BIP39/BIP44 key generation
└── encryption.py            # Password-based encryption
```

## Key Features

### 1. Wallet Management (`mtcli wallet`)

- **create-coldkey**: Generate new BIP39 mnemonic and coldkey
- **restore-coldkey**: Restore from existing mnemonic
- **generate-hotkey**: HD derivation for hotkeys (BIP44)
- **list**: Show all coldkeys
- **register-hotkey**: Register on network (planned)

### 2. Staking Operations (`mtcli stake`)

- **add**: Add stake to validator
- **remove**: Remove stake
- **claim**: Claim rewards
- **info**: Show staking information
- **list**: List all validators

### 3. Query Commands (`mtcli query`)

- **address**: Query address information
- **balance**: Check balance
- **subnet**: Query subnet details
- **list-subnets**: List all subnets
- **validator**: Query validator info
- **miner**: Query miner info

### 4. Transaction Commands (`mtcli tx`)

- **send**: Send tokens
- **history**: View transaction history
- **status**: Check transaction status

### 5. Subnet Commands (`mtcli subnet`)

- **create**: Create new subnet
- **register**: Register on subnet
- **info**: Show subnet details
- **participants**: List participants

### 6. Validator Commands (`mtcli validator`)

- **start**: Start validator node
- **stop**: Stop validator node
- **status**: Check validator status
- **set-weights**: Set validator weights

### 7. Utility Commands (`mtcli utils`)

- **convert**: Convert between units
- **latency**: Test network latency
- **generate-keypair**: Generate test keypair
- **version**: Show version info

## Implementation Status

### ✅ Completed (Phase 1)

1. **Core CLI Framework**
   - Click-based command structure
   - Rich console output
   - Configuration management
   - Error handling

2. **Wallet Commands (Partial)**
   - `create-coldkey`: Full implementation
   - `restore-coldkey`: Full implementation
   - `generate-hotkey`: Full implementation
   - `list`: Full implementation
   - Other wallet commands: Stubs created

3. **Key Management**
   - BIP39 mnemonic generation
   - BIP44 HD derivation
   - Password-based encryption (PBKDF2 + Fernet)
   - Ethereum-compatible addresses

4. **Utilities**
   - Rich table display
   - Error/success/warning messages
   - Password prompts
   - Configuration file support

### 🚧 To Be Implemented

1. **Wallet Commands**
   - import-hotkey
   - regen-hotkey
   - list-hotkeys
   - show-hotkey
   - show-address
   - query-address
   - register-hotkey

2. **All Stake Commands**
   - Integration with Luxtensor staking system
   - Transaction signing and submission

3. **All Query Commands**
   - Integration with Luxtensor RPC
   - Use existing `LuxtensorClient`

4. **All Transaction Commands**
   - Transaction building
   - Signing with wallet keys
   - Broadcasting to network

5. **All Subnet Commands**
   - Subnet creation and management
   - Registration logic

6. **All Validator Commands**
   - Validator node management
   - Weight setting mechanism

## Usage Examples

### Creating a Wallet

```bash
# Create new coldkey
mtcli wallet create-coldkey --name my_coldkey

# Restore from mnemonic
mtcli wallet restore-coldkey --name restored_key

# Generate hotkey
mtcli wallet generate-hotkey --coldkey my_coldkey --hotkey-name miner_hk1

# List wallets
mtcli wallet list
```

### Staking (Coming Soon)

```bash
# Add stake
mtcli stake add --coldkey my_coldkey --hotkey validator_hk --amount 1000000 --network testnet

# Check staking info
mtcli stake info --coldkey my_coldkey --hotkey validator_hk --network testnet
```

### Queries (Coming Soon)

```bash
# Query balance
mtcli query balance --coldkey my_coldkey --hotkey miner_hk1 --network testnet

# Query subnet
mtcli query subnet --subnet-uid 1 --network testnet

# List all subnets
mtcli query list-subnets --network testnet
```

### Utilities

```bash
# Show version
mtcli --version
mtcli utils version

# Convert units
mtcli utils convert --from-mdt 1.5

# Generate test keypair
mtcli utils generate-keypair
```

## Configuration

Default configuration file location: `~/.moderntensor/config.yaml`

Example configuration:

```yaml
network:
  name: testnet
  rpc_url: https://testnet.luxtensor.io
  chain_id: 2
  explorer_url: https://testnet-explorer.luxtensor.io

wallet:
  path: ~/.moderntensor/wallets
  default_coldkey: my_coldkey
  default_hotkey: miner_hk1

verbosity: 1
use_cache: true
cache_ttl: 300
```

## Security Considerations

1. **Password Protection**: All coldkeys are encrypted with password-based encryption (PBKDF2 with 100,000 iterations)

2. **Mnemonic Storage**: Mnemonics are displayed once during creation. Users must save them securely.

3. **Private Keys**: Private keys are never displayed or logged in plaintext (except in generate-keypair for testing)

4. **File Permissions**: Wallet files should have restricted permissions (600)

## Integration with Existing SDK

The CLI integrates with:

1. **LuxtensorClient** (`sdk/luxtensor_client.py`): For blockchain queries and transactions
2. **AsyncLuxtensorClient** (`sdk/async_luxtensor_client.py`): For async operations
3. **Axon/Dendrite** (`sdk/axon/`, `sdk/dendrite/`): For validator/miner operations
4. **Tokenomics** (`sdk/tokenomics/`): For staking and rewards

## Next Steps

### Priority 1: Complete Wallet Commands

1. Implement remaining wallet commands
2. Add wallet export/import functionality
3. Add address derivation display
4. Integrate with LuxtensorClient for balance queries

### Priority 2: Query Commands

1. Implement all query commands using LuxtensorClient
2. Add caching for frequently accessed data
3. Add formatted output (tables, JSON)

### Priority 3: Transaction Commands

1. Implement transaction building
2. Add signing with wallet keys
3. Implement transaction submission
4. Add transaction monitoring

### Priority 4: Staking & Validator

1. Implement staking operations
2. Add validator management
3. Implement weight setting
4. Add validator monitoring

### Priority 5: Testing & Documentation

1. Add unit tests for CLI commands
2. Add integration tests with testnet
3. Create comprehensive user guide
4. Add examples and tutorials

## Comparison with btcli

| Feature | btcli (Bittensor) | mtcli (ModernTensor) | Status |
|---------|-------------------|----------------------|--------|
| Wallet Management | ✅ Full | 🟡 Partial | In Progress |
| Staking | ✅ Full | ⚪ Planned | To Do |
| Queries | ✅ Full | ⚪ Planned | To Do |
| Transactions | ✅ Full | ⚪ Planned | To Do |
| Subnet Management | ✅ Full | ⚪ Planned | To Do |
| Validator Operations | ✅ Full | ⚪ Planned | To Do |
| Configuration | ✅ YAML | ✅ YAML | Complete |
| Output Formatting | ✅ Rich/Typer | ✅ Rich/Click | Complete |
| Framework | Typer | Click | Different |

## Dependencies Added

- `click==8.1.8`: CLI framework
- `rich==13.7.0`: Beautiful terminal output
- `eth-account==0.11.0`: Ethereum-compatible key management
- `bip_utils==2.9.3`: BIP39/BIP44 implementation
- `cryptography==42.0.8`: Encryption primitives

## Notes

- mtcli uses Click instead of Typer (btcli's choice) for more control and flexibility
- Key derivation uses Ethereum-compatible BIP44 paths (m/44'/60'/0'/0/index)
- All wallet data stored in `~/.moderntensor/wallets/` by default
- Configuration stored in `~/.moderntensor/config.yaml`
- Compatible with Luxtensor blockchain's account-based model

## Testing

```bash
# Test CLI installation
python -m sdk.cli.main --version

# Test wallet creation (requires user input)
python -m sdk.cli.main wallet create-coldkey --name test_key

# Test help
python -m sdk.cli.main --help
python -m sdk.cli.main wallet --help
```
