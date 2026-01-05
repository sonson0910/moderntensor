# Migration Guide: Cardano to Layer 1 Blockchain

## Tổng Quan / Overview

ModernTensor đang chuyển đổi từ một ứng dụng chạy trên Cardano sang một blockchain Layer 1 độc lập. Tài liệu này hướng dẫn quá trình migration.

ModernTensor is transitioning from a Cardano-based application to an independent Layer 1 blockchain. This guide explains the migration process.

## Timeline / Lộ Trình

### Phase 2.5 (Current) - Dual Mode Operation
**Status:** ✅ Active  
**Duration:** 3 months  
**Mode:** Both Cardano and L1 active

**What's Available:**
- ✅ New L1 blockchain primitives (blocks, transactions, state)
- ✅ PoS consensus with AI validation
- ✅ Bridge layer for Cardano compatibility
- 🔄 Validators can operate on both systems

**For Validators:**
- Continue using Cardano for now
- Test L1 functionality on testnet
- Prepare for migration

### Phase 3-5 (3-6 months) - L1 Primary
**Status:** 🔄 Planned  
**Duration:** 3 months  
**Mode:** L1 primary, Cardano bridge only

**What Changes:**
- 🚀 L1 becomes primary network
- 🌉 Cardano available via bridge only
- ⚠️ New validators must use L1
- 📉 Cardano features marked deprecated

**For Validators:**
- **Required:** Migrate to L1 by end of Phase 5
- Learn L1 transaction format
- Update node software

### Phase 6+ (6+ months) - Cardano Deprecated
**Status:** 📅 Future  
**Duration:** Ongoing  
**Mode:** L1 only

**What Changes:**
- ❌ Cardano support removed
- ✅ Full L1 functionality
- 🎯 100% independent blockchain

## For Validators: Migration Steps

### Step 1: Understand the Changes

**Old System (Cardano):**
```python
# Cardano UTXO-based
from sdk.metagraph import ValidatorDatum
from pycardano import BlockFrostChainContext

# Register on Cardano
context = BlockFrostChainContext(...)
# Submit datum to Plutus contract
```

**New System (L1):**
```python
# Account-based blockchain
from sdk.blockchain import Transaction, StateDB
from sdk.consensus.pos import ProofOfStake

# Register on L1
state = StateDB()
pos = ProofOfStake(state, config)
pos.validator_set.add_validator(address, pubkey, stake)
```

### Step 2: Generate L1 Keys

```python
from sdk.blockchain.crypto import KeyPair

# Generate new L1 keypair
keypair = KeyPair()
address = keypair.address()  # 20 bytes
pubkey = keypair.public_key  # 64 bytes

# Save securely
private_key_hex = keypair.export_private_key()
```

### Step 3: Register on L1 (Using Bridge)

```python
from sdk.bridge.validator_bridge import ValidatorBridge
from sdk.core.datatypes import ValidatorInfo

# Your existing Cardano validator info
cardano_validator = ValidatorInfo(
    uid="your_uid",
    stake=1000000,
    address="cardano_addr",
    # ... other fields
)

# Sync to L1
bridge = ValidatorBridge()
success = bridge.migrate_validator(
    cardano_uid="your_uid",
    validator_info=cardano_validator
)
```

### Step 4: Update Node Configuration

```yaml
# config/node.yaml

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
