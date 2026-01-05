# ModernTensor Project Restructuring Plan

## Mục đích / Purpose
Chuẩn bị cho Phase 3 bằng cách dọn dẹp, tổ chức lại và loại bỏ các thành phần không còn cần thiết khi chuyển sang Layer 1 độc lập.

## 1. Documentation Consolidation

### Cần loại bỏ / To Remove:
- ❌ `COMPLETE_SUMMARY.md` - Outdated, superseded by newer docs
- ❌ `CONSENSUS_REVIEW.md` - Old review, no longer relevant
- ❌ `CONSENSUS_REVIEW_README.md` - Duplicate info
- ❌ `CONSENSUS_REVIEW_SUMMARY.md` - Duplicate info
- ❌ `IMPLEMENTATION_SUMMARY.md` - Superseded by LAYER1_IMPLEMENTATION_SUMMARY.md

### Giữ lại / Keep:
- ✅ `LAYER1_ROADMAP.md` - Main roadmap
- ✅ `LAYER1_IMPLEMENTATION_SUMMARY.md` - Current progress
- ✅ `README.md` - Project overview
- ✅ `CHANGELOG.md` - Version history

## 2. Cardano Integration Layer

### Chiến lược / Strategy:
Không loại bỏ ngay, mà tạo một bridge layer để:
1. Maintain backward compatibility cho existing users
2. Gradually migrate away from Cardano
3. Keep validator registration working during transition

### Tổ chức lại / Reorganize:
- Move Cardano-specific code to `sdk/legacy/cardano/`
- Create bridge adapters in `sdk/bridge/`
- Update imports gradually

### Files to move to legacy:
- `sdk/metagraph/*` → `sdk/legacy/cardano/metagraph/`
- `sdk/smartcontract/*` → `sdk/legacy/cardano/smartcontract/`
- Cardano-specific CLI commands → mark as deprecated

## 3. Consensus Layer Cleanup

### Current Issues:
- Duplicate consensus logic between old (`sdk/consensus/state.py`) and new (`sdk/consensus/pos.py`)
- `sdk/consensus/scoring.py` and `sdk/consensus/selection.py` overlap with new PoS

### Action Plan:
- ✅ Keep new L1 consensus: `pos.py`, `fork_choice.py`, `ai_validation.py`
- 🔄 Refactor `state.py` to use new PoS underneath
- 🔄 Deprecate `scoring.py` and `selection.py` in favor of PoS logic
- 🔄 Update `node.py` to work with new blockchain primitives

## 4. Dependencies Audit

### To Add:
```python
# For proper ECDSA
"ecdsa==0.18.0"
"coincurve==18.0.0"  # Fast secp256k1

# For Merkle Patricia Trie
"py-trie==0.4.0"

# For storage
"plyvel==1.5.0"  # LevelDB bindings
```

### To Keep (Essential):
- FastAPI, Pydantic (API layer)
- cryptography (general crypto)
- websockets (network)
- loguru (logging)

### To Consider Removing (Cardano-specific):
- ⚠️ pycardano (keep for now, move to legacy)
- ⚠️ bip_utils (keep for HD key derivation)

## 5. New Module Structure

```
sdk/
├── blockchain/          # ✅ New L1 primitives (Phase 1)
│   ├── block.py
│   ├── transaction.py
│   ├── state.py
│   ├── crypto.py
│   └── validation.py
├── consensus/           # ✅ New PoS consensus (Phase 2)
│   ├── pos.py
│   ├── fork_choice.py
│   ├── ai_validation.py
│   └── node.py (updated)
├── network/             # 🔄 To enhance (Phase 3)
│   ├── p2p.py (new)
│   ├── sync.py (new)
│   ├── messages.py (new)
│   └── server.py (existing API)
├── storage/             # 📦 New (Phase 4)
│   ├── blockchain_db.py
│   ├── state_db.py
│   └── indexer.py
├── api/                 # 📦 New (Phase 5)
│   ├── rpc.py
│   └── graphql_api.py
├── bridge/              # 🌉 New bridge layer
│   ├── cardano_adapter.py
│   └── validator_sync.py
├── legacy/              # 📁 Cardano code moved here
│   └── cardano/
│       ├── metagraph/
│       └── smartcontract/
├── cli/                 # ✅ Keep, update commands
├── utils/               # ✅ Keep
├── formulas/            # ✅ Keep (AI scoring)
└── keymanager/          # ✅ Keep (HD wallets)
```

## 6. Immediate Actions

### Phase 2.5 (Pre-Phase 3 Cleanup):

1. **Documentation Cleanup** (5 min)
   - Remove old summary files
   - Update README with new architecture

2. **Create Legacy Module** (10 min)
   - Create `sdk/legacy/cardano/` structure
   - Move Cardano-specific code
   - Add deprecation warnings

3. **Dependencies Update** (5 min)
   - Add new crypto libraries
   - Update pyproject.toml
   - Document migration path

4. **Consensus Refactoring** (30 min)
   - Make `state.py` use new PoS as backend
   - Add compatibility layer
   - Update tests

5. **Create Bridge Layer** (20 min)
   - Create adapters for validator sync
   - Maintain API compatibility
   - Document bridge usage

Total Time: ~70 minutes

## 7. Testing Strategy

- ✅ All new blockchain tests passing
- 🔄 Update existing consensus tests to use new PoS
- 🔄 Add integration tests for bridge layer
- ⚠️ Mark Cardano-specific tests as legacy

## 8. Migration Guide for Users

Create `MIGRATION.md` documenting:
1. How existing validators migrate to L1
2. Breaking changes (if any)
3. Timeline for Cardano deprecation
4. Support for dual-mode operation

## Success Criteria

- ✅ No duplicate documentation
- ✅ Clear separation: L1 core vs legacy Cardano
- ✅ All tests passing
- ✅ Bridge layer working for backward compatibility
- ✅ Ready to implement Phase 3 (Network Layer)

## Timeline

- Phase 2.5 Cleanup: 1-2 hours
- Phase 3 (Network): 6 weeks (as planned)
- Cardano deprecation: 6 months (gradual)
