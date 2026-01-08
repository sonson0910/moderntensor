# Cardano/PyCardano Deprecation Notice

**Date:** 2026-01-08  
**Status:** DEPRECATED - All Cardano infrastructure removed

## Summary

ModernTensor has transitioned from Cardano Layer 2 to **Luxtensor Layer 1** blockchain (Rust implementation). All Cardano/PyCardano dependencies have been removed.

## What Was Removed

### 1. Dependencies
- ❌ `pycardano` (Cardano Python SDK)
- ❌ `cbor2` (CBOR serialization for Cardano)
- ❌ BlockFrost API integration
- ❌ Cardano Hydra Layer 2 client

### 2. Files Removed
- `sdk/network/hydra_client.py` - Cardano Hydra WebSocket client
- `scripts/prepare_testnet_datums.py` - Cardano testnet setup script
- Legacy node modules (cardano_client.py, cardano_contract.py) - Already removed

### 3. Code Deprecated
The following modules contain legacy Cardano code that is being phased out:
- `sdk/service/` - Cardano chain context, UTxO management
- `sdk/metagraph/metagraph_data.py` - BlockFrost integration
- `sdk/agent/miner_agent.py` - Cardano transaction submission
- Various CLI commands with Cardano references

## Migration Path

### Old Architecture (Deprecated)
```
ModernTensor SDK (Python)
    ↓
Cardano Layer 2 (Hydra)
    ↓
Cardano Mainnet
```

### New Architecture (Current)
```
ModernTensor SDK (Python)
    ↓ JSON-RPC
Luxtensor Blockchain (Rust)
    ↓
Custom Layer 1 PoS Chain
```

## What to Use Instead

### 1. Blockchain Interaction
**Old (Cardano):**
```python
from sdk.compat.pycardano import BlockFrostChainContext
context = BlockFrostChainContext(project_id, network)
```

**New (Luxtensor):**
```python
from sdk.luxtensor_client import LuxtensorClient
client = LuxtensorClient("http://localhost:9944")
```

### 2. Data Structures
**Old (Cardano):**
```python
from pycardano import PlutusData, UTxO
class MyDatum(PlutusData):
    CONSTR_ID = 0
```

**New (Luxtensor):**
```python
from sdk.compat.luxtensor_types import L1Data
from dataclasses import dataclass

@dataclass
class MyData(L1Data):
    """Native Luxtensor data structure"""
```

### 3. Transactions
**Old (Cardano):**
```python
tx_cbor = signed_tx.to_cbor()
tx_id = context.submit_tx(tx_cbor)
```

**New (Luxtensor):**
```python
# Via RPC to Luxtensor blockchain
tx_hash = await client.submit_transaction(tx_data)
```

## Implementation Status

### ✅ Completed
- [x] Removed PyCardano dependency
- [x] Removed Hydra client
- [x] Removed Cardano testnet scripts
- [x] Created placeholder Luxtensor types
- [x] Documentation updated

### 🔄 In Progress (See SDK_FINALIZATION_ROADMAP.md)
- [ ] Full Luxtensor RPC client implementation
- [ ] Refactor service layer for Luxtensor
- [ ] Migrate metagraph to Luxtensor storage
- [ ] Update CLI commands for Luxtensor
- [ ] Remove remaining Cardano artifacts

### 📋 Planned (Phase 1-2, 6-8 weeks)
- [ ] Complete async Luxtensor client
- [ ] Native Luxtensor data models
- [ ] Transaction system for Luxtensor
- [ ] State queries via Luxtensor RPC
- [ ] Full test coverage

## Rationale

### Why Move from Cardano to Luxtensor?

1. **Performance**: Rust-based Layer 1 is 10-100x faster than Cardano Layer 2
2. **Control**: Full control over blockchain parameters and consensus
3. **Simplicity**: Direct Layer 1 vs complex Layer 2 + Layer 1 stack
4. **Cost**: No Cardano mainnet fees or infrastructure costs
5. **Customization**: AI/ML-specific blockchain features

### Why Remove Cardano Code Now?

1. **Technical Debt**: Maintaining two blockchain backends is complex
2. **Dependency Risk**: PyCardano dependencies add attack surface
3. **Code Clarity**: Clean codebase easier to develop and maintain
4. **Focus**: Resources better spent on Luxtensor development
5. **User Request**: Explicit request to remove old infrastructure

## Timeline

| Phase | Description | Status | ETA |
|-------|-------------|--------|-----|
| **Phase 0** | Remove PyCardano dependencies | ✅ Complete | 2026-01-08 |
| **Phase 1** | Implement Luxtensor client | 🔄 In Progress | 2-3 weeks |
| **Phase 2** | Migrate service layer | 📋 Planned | 4-6 weeks |
| **Phase 3** | Full Luxtensor integration | 📋 Planned | 8-10 weeks |

## For Developers

### If You Need Cardano Features

Cardano support has been **permanently removed**. ModernTensor is now exclusively built on Luxtensor Layer 1 blockchain.

### If You Find Cardano References

These are **legacy artifacts** being phased out. You can:
1. Ignore them (marked as deprecated)
2. Help migrate them to Luxtensor (see roadmap)
3. Report them for cleanup

### Contributing

To contribute to the Luxtensor migration:
1. See `SDK_FINALIZATION_ROADMAP.md` for priorities
2. Check `SDK_REDESIGN_INDEX.md` for documentation
3. Focus on Phases 1-3 (blockchain client, communication, data models)

## References

- **Luxtensor Blockchain**: `/luxtensor/` directory (Rust)
- **SDK Roadmap**: `SDK_REDESIGN_ROADMAP.md`
- **Finalization Plan**: `SDK_FINALIZATION_ROADMAP.md`
- **Architecture**: `SDK_ARCHITECTURE_CLARIFICATION.md`
- **Luxtensor Migration**: `LUXTENSOR_MIGRATION.md`

## Questions?

See:
- `LUXTENSOR_TECHNICAL_FAQ_VI.md` (Vietnamese)
- `LUXTENSOR_USAGE_GUIDE.md` (English)
- GitHub Issues for technical questions

---

**TL;DR:** Cardano is gone. Use Luxtensor. Old code marked deprecated. Migration in progress. See roadmap docs.
