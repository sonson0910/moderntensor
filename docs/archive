# Cardano/PyCardano Deprecation Notice

**Date:** 2026-01-08  
**Status:** Deprecated and Removed

## Background

ModernTensor initially explored using Cardano as its Layer 1 blockchain platform. However, the project has since pivoted to **Luxtensor**, a custom Rust-based Layer 1 blockchain optimized for AI/ML workloads.

## Key Differences

### Cardano (Old Approach)
- UTXO-based transaction model
- Plutus smart contracts with Datum/Redeemer
- BlockFrost API for chain interaction
- PyCardano Python library
- eUTxO accounting model

### Luxtensor (Current Approach)
- Account-based transaction model (Ethereum-style)
- Native AI/ML validation support
- JSON-RPC API for chain interaction
- Custom Python client (`luxtensor_client.py`)
- Account/balance model

## Removed Components

The following Cardano-specific components have been removed as they are incompatible with Luxtensor:

### Scripts
- `scripts/prepare_testnet_datums.py` - Cardano datum preparation script

### Service Layer (Deprecated Cardano APIs)
- `sdk/service/utxos.py` - UTXO querying (not applicable to account-based chains)
- `sdk/metagraph/create_utxo.py` - UTXO creation functions
- `sdk/metagraph/remove_fake_utxo.py` - UTXO cleanup utilities

### Data Types (Replaced)
- `PlutusData` - Replaced with Pydantic models
- `Datum`/`Redeemer` - Not needed in account-based model
- `UTxO` - Replaced with account/transaction model
- `BlockFrostChainContext` - Replaced with `LuxtensorClient`

## Migration Path

### For Wallet Operations
**Before (Cardano):**
```python
from sdk.compat.pycardano import Address, UTxO, BlockFrostChainContext
context = BlockFrostChainContext(project_id, network)
utxos = context.utxos(address)
```

**After (Luxtensor):**
```python
from sdk.luxtensor_client import LuxtensorClient
client = LuxtensorClient("http://localhost:9944")
balance = client.get_balance(address)
```

### For Transaction Building
**Before (Cardano):**
```python
from pycardano import TransactionBuilder, TransactionOutput
builder = TransactionBuilder(context)
builder.add_input(utxo)
builder.add_output(TransactionOutput(...))
```

**After (Luxtensor):**
```python
from sdk.transactions import create_transfer_transaction
tx = create_transfer_transaction(
    from_address=sender,
    to_address=recipient,
    amount=value,
    nonce=nonce
)
client.submit_transaction(tx)
```

### For Smart Contracts
**Before (Cardano Plutus):**
```python
from pycardano import PlutusV3Script, PlutusData, Redeemer
script = PlutusV3Script(cbor_hex)
datum = MyDatum(field1=..., field2=...)
```

**After (Luxtensor Native Contracts):**
```python
# Use Rust-based smart contracts in luxtensor/crates/luxtensor-contracts
# Python SDK interacts via RPC, no need for Python-side contract definitions
```

## What Remains

### Compatible Components
- ✅ **Wallet Management** - Adapted for account-based model
- ✅ **Key Management** - BIP39/BIP32 still used for key derivation
- ✅ **CLI Tools** - Adapted to use LuxtensorClient
- ✅ **Metagraph** - Adapted to query from Luxtensor state
- ✅ **AI/ML Framework** - Compatible with any blockchain backend

### Compatibility Layer
The `sdk/compat/luxtensor_types.py` provides type aliases for backward compatibility during migration, but these are placeholders only. All real functionality now uses `LuxtensorClient`.

## Benefits of Luxtensor

1. **Performance**: 10-100x faster than Python/Cardano
2. **AI-First**: Built specifically for AI/ML validation
3. **Simplicity**: Account-based model is simpler than UTXO
4. **Native Support**: No need for external blockchain APIs
5. **Full Control**: Custom blockchain optimized for our use case

## Questions?

See the following documentation:
- `luxtensor/README.md` - Luxtensor blockchain overview
- `SDK_REDESIGN_ROADMAP.md` - SDK redesign plan
- `LUXTENSOR_MIGRATION.md` - Original migration notes

---

**This deprecation is final. Cardano support will not be restored.**
