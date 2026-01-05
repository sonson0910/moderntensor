# Migration Guide: Cardano to Layer 1 Blockchain

## Tổng Quan / Overview

**Migration Status: ✅ COMPLETE (January 2026)**

ModernTensor has successfully transitioned from a Cardano-based application to an independent Layer 1 blockchain. All Cardano dependencies (pycardano, blockfrost) have been removed and replaced with native Layer 1 blockchain primitives.

ModernTensor has completed its transition from Cardano to an independent Layer 1 blockchain. This document describes the migration that was completed.

## Migration Completed ✅

### What Changed

**Before (Cardano):**
- Used pycardano for key management and transactions
- Used BlockFrost API for chain interaction
- Cardano-specific UTXO model
- Cardano derivation paths (m/1852'/1815'/...)

**After (Layer 1):**
- Native Layer 1 HD wallet (sdk/blockchain/l1_keymanager.py)
- JSON-RPC for chain interaction
- Account-based model
- Standard BIP44 derivation paths (m/44'/0'/...)

### Timeline (Completed)

- ✅ **Phase 1:** Layer 1 blockchain primitives implemented
- ✅ **Phase 2:** Created L1 key management system
- ✅ **Phase 3:** Created L1 chain context (RPC)
- ✅ **Phase 4:** Created compatibility layer (sdk/compat/pycardano.py)
- ✅ **Phase 5:** Replaced all pycardano imports (50+ files)
- ✅ **Phase 6:** Removed pycardano from dependencies

## For Developers: Using Layer 1

### Importing Blockchain Components

**Direct Layer 1 imports (recommended):**
```python
# Layer 1 blockchain primitives
from sdk.blockchain import (
    L1HDWallet,           # HD wallet for key derivation
    L1Address,            # Layer 1 addresses
    L1Network,            # Network types (MAINNET, TESTNET, DEVNET)
    L1ChainContext,       # RPC connection to L1 nodes
    KeyPair,              # Cryptographic key pairs
    Transaction,          # Layer 1 transactions
)

# Create wallet
wallet = L1HDWallet.from_mnemonic("your mnemonic phrase...")
address = wallet.get_root_address()

# Connect to network
context = L1ChainContext(network=L1Network.TESTNET)
balance = context.get_balance(str(address))
```

**Compatibility imports (for gradual migration):**
```python
# Uses compatibility layer that wraps Layer 1
from sdk.compat.pycardano import (
    HDWallet,                # → L1HDWallet
    Network,                 # → L1Network
    BlockFrostChainContext,  # → L1ChainContext
    Address,                 # → L1Address
)

# This code works with minimal changes
wallet = HDWallet.from_mnemonic("your mnemonic phrase...")
```

### Key Management

**HD Wallet (Layer 1):**
```python
from sdk.blockchain import L1HDWallet

# Generate new wallet
wallet = L1HDWallet()  # Auto-generates 24-word mnemonic
print(f"Mnemonic: {wallet.mnemonic}")

# Or restore from mnemonic
wallet = L1HDWallet.from_mnemonic("word1 word2 ... word24")

# Derive keys using BIP44 standard paths
hotkey_0 = wallet.derive_hotkey(0)  # m/44'/0'/0'/0/0
hotkey_1 = wallet.derive_hotkey(1)  # m/44'/0'/0'/0/1

# Get addresses
address = hotkey_0.address()  # Returns 20-byte address
address_hex = "0x" + address.hex()
```

### Network Interaction

**Chain Context (Layer 1):**
```python
from sdk.blockchain import L1ChainContext, L1Network

# Connect to testnet
context = L1ChainContext(
    network=L1Network.TESTNET,
    rpc_url="http://testnet-rpc.moderntensor.io:8545"
)

# Query balance
balance = context.get_balance(address_hex)

# Get nonce for transactions
nonce = context.get_nonce(address_hex)

# Submit transaction
tx_hash = context.submit_tx(signed_transaction)
```

### Creating Transactions

**Layer 1 Transactions:**
```python
from sdk.blockchain import Transaction, KeyPair

# Create transaction
tx = Transaction(
    nonce=0,
    from_address=sender_address,     # 20 bytes
    to_address=recipient_address,     # 20 bytes
    value=1000000,                    # Amount in smallest unit
    gas_price=1000,
    gas_limit=21000,
    data=b"",                         # Optional payload
)

# Sign transaction
keypair = KeyPair(private_key_bytes)
tx.sign(keypair.private_key)

# Submit to network
context = L1ChainContext(network=L1Network.TESTNET)
tx_hash = context.submit_tx(tx)
print(f"Transaction hash: {tx_hash}")
```

# Enable dual mode
mode: "dual"  # Options: "cardano", "l1", "dual"

# L1 Configuration
l1:
  enabled: true
  validator_address: "0x..."
  validator_pubkey: "0x..."
  
# Cardano (legacy)
cardano:
  enabled: true  # Will be false in Phase 3
  network: "testnet"
  # ... existing config
```

### Step 5: Test on Testnet

```bash
# Run validator in dual mode on testnet
mtcli run_validator \
  --mode dual \
  --network testnet \
  --config config/node.yaml
```

### Step 6: Monitor Migration

```python
from sdk.bridge.validator_bridge import ValidatorBridge

bridge = ValidatorBridge()
stats = bridge.get_statistics()

print(f"Synced validators: {stats['total_synced']}")
print(f"Active on L1: {stats['active_validators']}")
```

## For Developers: API Changes

### Transaction Format

**Old (Cardano):**
```python
# UTXO-based transaction
from pycardano import TransactionBuilder

builder = TransactionBuilder(context)
builder.add_input(utxo)
builder.add_output(output)
tx = builder.build()
```

**New (L1):**
```python
# Account-based transaction
from sdk.blockchain import Transaction

tx = Transaction(
    nonce=0,
    from_address=sender_addr,
    to_address=recipient_addr,
    value=1000,
    gas_price=1,
    gas_limit=21000,
)
tx.sign(private_key)
```

### Consensus Mechanism

**Old (Cardano):**
```python
# Cardano PoS (delegated)
from sdk.consensus.state import run_consensus_logic

# Complex validator scoring
scores = run_consensus_logic(...)
```

**New (L1):**
```python
# ModernTensor PoS with AI validation
from sdk.consensus.pos import ProofOfStake
from sdk.consensus.ai_validation import AIValidator

pos = ProofOfStake(state, config)
validator = pos.select_validator(slot=1)

# AI validation integrated
ai_validator = AIValidator()
ai_validator.validate_ai_task(task, result)
```

### State Management

**Old (Cardano):**
```python
# UTXO model
from sdk.metagraph import get_all_validator_data

validators = get_all_validator_data(context, script_hash, network)
```

**New (L1):**
```python
# Account model
from sdk.blockchain.state import StateDB

state = StateDB()
account = state.get_account(address)
balance = account.balance
nonce = account.nonce
```

## Breaking Changes

### Removed Features
- ❌ Direct Cardano UTXO manipulation
- ❌ Plutus smart contract integration (replaced by L1 contracts)
- ❌ BlockFrost API dependency (replaced by L1 RPC)

### Changed APIs
- 🔄 `ValidatorInfo` now includes L1 address field
- 🔄 Transaction format completely different
- 🔄 Block structure uses Merkle trees instead of Cardano format

### New Requirements
- ✅ Must run L1 node software
- ✅ Need L1 keypair (ECDSA)
- ✅ Minimum stake: 1M tokens (configurable)

## Backward Compatibility

### During Phase 2.5-3
- ✅ Cardano operations still work
- ✅ Bridge auto-syncs validators
- ✅ Existing CLI commands work
- ⚠️ Deprecation warnings shown

### During Phase 4-5
- 🌉 Cardano via bridge only
- ⚠️ Performance degradation for Cardano ops
- 📢 Migration reminders

### After Phase 6
- ❌ Cardano support removed
- ❌ Old APIs return errors
- ✅ Full L1 performance

## Testing Your Migration

### Checklist
- [ ] Generate L1 keypair
- [ ] Register validator on L1 testnet
- [ ] Submit test transactions
- [ ] Verify balance and state
- [ ] Test AI task validation
- [ ] Monitor for 24 hours
- [ ] Migrate to mainnet

### Test Commands
```bash
# Test L1 transaction
mtcli tx send \
  --from 0x... \
  --to 0x... \
  --value 1000 \
  --network testnet

# Test validator
mtcli validator status \
  --address 0x... \
  --network testnet

# Check bridge sync
mtcli bridge stats
```

## Getting Help

### Resources
- 📖 Documentation: `/docs/l1-migration/`
- 💬 Discord: #migration-support
- 🐛 Issues: GitHub Issues
- 📧 Email: support@moderntensor.io

### Common Issues

**Q: My Cardano validator isn't syncing to L1**
A: Check bridge logs: `mtcli bridge logs`

**Q: Transaction fails on L1**
A: Verify gas limit and nonce: `mtcli tx estimate`

**Q: Lost my L1 private key**
A: Keys are NOT recoverable. Always backup!

**Q: Can I use same keys for Cardano and L1?**
A: No, different cryptography (Ed25519 vs secp256k1)

## Support Timeline

| Phase | Cardano Support | L1 Support | Recommended Action |
|-------|----------------|------------|-------------------|
| 2.5 (Now) | ✅ Full | ✅ Full | Test L1 on testnet |
| 3-4 (3mo) | 🌉 Bridge | ✅ Primary | Migrate to L1 |
| 5 (6mo) | ⚠️ Deprecated | ✅ Full | Complete migration |
| 6+ (6mo+) | ❌ Removed | ✅ Only | N/A |

## Incentives for Early Migration

### Benefits
- 🎁 Early migrator bonus: +5% staking rewards (first 3 months)
- 🚀 Access to new L1 features first
- 🏆 "Pioneer" badge for early validators
- 📊 Better performance (no bridge overhead)

### How to Qualify
1. Migrate before Phase 3 starts (3 months)
2. Run L1 validator for 30+ days
3. Submit ≥100 transactions on L1
4. Participate in testnet

## Conclusion

The migration to Layer 1 brings:
- ✅ Full control over consensus
- ✅ Better performance
- ✅ Native AI validation
- ✅ Independent tokenomics
- ✅ Simplified architecture

**Act now to secure your early migrator benefits!**

---

*Last Updated: 2026-01-05*  
*Version: 0.1.0 (Phase 2.5)*
