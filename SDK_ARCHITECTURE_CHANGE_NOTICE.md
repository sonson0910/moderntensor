# SDK Architecture Changes - January 2026

## ⚠️ IMPORTANT: Cardano Infrastructure Removed

As of **January 8, 2026**, all Cardano/PyCardano dependencies have been **permanently removed** from ModernTensor SDK. The project now exclusively uses **Luxtensor Layer 1** blockchain (Rust implementation).

## What Changed

### Removed ❌
- Cardano PyCardano library
- BlockFrost API integration
- Hydra Layer 2 client
- CBOR serialization (Cardano-specific)
- UTXO-based code (replaced with account model)

### Added ✅
- `sdk/blockchain/` - Native Luxtensor blockchain module
- `LuxtensorClient` - Direct RPC to Luxtensor blockchain
- Clear documentation and migration guides
- Deprecation warnings on legacy code

## New Architecture

```
ModernTensor SDK (Python)
    ↓ JSON-RPC/WebSocket
Luxtensor Blockchain (Rust)
    ↓ Custom PoS Consensus
Layer 1 Blockchain
```

## Quick Start

```python
# New way - Use Luxtensor directly
from sdk.luxtensor_client import LuxtensorClient

client = LuxtensorClient("http://localhost:9944")
block = client.get_block_number()
balance = client.get_balance("your_address")
```

## Documentation

- **[CARDANO_DEPRECATION_NOTICE.md](CARDANO_DEPRECATION_NOTICE.md)** - Why we removed Cardano
- **[LUXTENSOR_CLEANUP_SUMMARY.md](LUXTENSOR_CLEANUP_SUMMARY.md)** - Complete cleanup summary
- **[SDK_FINALIZATION_ROADMAP.md](SDK_FINALIZATION_ROADMAP.md)** - Implementation roadmap
- **[SDK_REDESIGN_INDEX.md](SDK_REDESIGN_INDEX.md)** - Documentation index

## For Developers

### If You Were Using Cardano Code
**Stop!** Cardano support is removed. Use Luxtensor instead:
- Replace `BlockFrostChainContext` with `LuxtensorClient`
- Replace UTXO queries with account queries
- Replace CBOR serialization with JSON/Pydantic

### If You Find Cardano References
Some legacy code remains marked as `DEPRECATED`:
- `sdk/service/` - Being refactored in Phase 2
- `sdk/metagraph/metagraph_data.py` - Being migrated
- These will be removed in upcoming phases

### Contributing
See `SDK_FINALIZATION_ROADMAP.md` for:
- Phase 1: Blockchain client expansion (2-3 weeks)
- Phase 2: Service layer migration (4-6 weeks)  
- Phase 3: Legacy code removal (2-3 weeks)

## Timeline

- **Jan 8, 2026** - Cardano infrastructure removed (this PR)
- **Next 2-3 weeks** - Phase 1: Expand async Luxtensor client
- **Next 6-8 weeks** - Phase 2: Complete service layer migration
- **8 months** - Full production-ready SDK

## Questions?

- **Technical FAQ:** `LUXTENSOR_TECHNICAL_FAQ_VI.md` (Vietnamese)
- **Usage Guide:** `LUXTENSOR_USAGE_GUIDE.md` (English)
- **Migration Help:** `CARDANO_DEPRECATION_NOTICE.md`

---

**TL;DR:** Cardano is gone. Use Luxtensor. Old code marked deprecated. See roadmap for details.
