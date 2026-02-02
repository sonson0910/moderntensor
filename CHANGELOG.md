## [0.5.0] - 2026-02-02

### Added

- **Native AI Precompiles**: AI as first-class primitive in EVM
  - `AI_REQUEST` (0x10) - Submit AI inference requests
  - `VERIFY_PROOF` (0x11) - Verify zkML proofs
  - `GET_RESULT` (0x12) - Retrieve inference results
  - `COMPUTE_PAYMENT` (0x13) - Calculate request payments
- **PaymentEscrow Contract**: Pay-per-compute system with MDT tokens
  - Deposit/release/refund mechanisms
  - 1% protocol fee for sustainability
- **Technical Documentation**:
  - `docs/AI_PRECOMPILES.md` - Native AI opcodes specification
  - `docs/PAYMENT_ESCROW.md` - Pay-per-compute system docs

### Changed

- Updated progress to ~90% (Native AI Integration complete)
- Updated roadmap: Phase 4 ✅, targeting Q1 2026 mainnet
- Architecture: 11 Rust crates (added luxtensor-contracts, luxtensor-oracle, luxtensor-zkvm)

---

## [0.4.0] - 2026-01-09

### Changed

- **Major Documentation Cleanup**: Removed 53 redundant documentation files
  - Reduced root documentation from 54 to 12 essential files (77% reduction)
  - Removed all outdated PHASE*_SUMMARY.md files
  - Removed all redundant *_COMPLETION.md and*_FINALIZATION.md files
  - Removed duplicate roadmap, summary, and index files
  - Kept only essential docs: README, Whitepaper, Technical guides, Roadmaps
- Updated DOCUMENTATION_INDEX.md to reflect clean structure
- Updated DOCUMENTATION.md for consistency

### Removed

- **Code Cleanup**: Removed 11 empty/obsolete module files
  - sdk/service/contract_service.py (empty)
  - sdk/metagraph/metagraph_api.py (empty)
  - sdk/metagraph/metagraph_utils.py (empty)
  - sdk/network/schemas.py, client.py, models.py (empty)
  - sdk/cli/metagraph_cli.py (empty)
  - sdk/config/constants.py, env.py (empty)
  - examples/advanced_usage.py, quickstart.py (empty)
- Removed validator_state.json runtime file (added to .gitignore)
- Removed .cleanup_plan.txt temporary file

### Improved

- Cleaner SDK structure focusing on Layer 1 blockchain functionality
- Better documentation organization
- Removed obsolete Cardano-related placeholders

## [0.3.0] - 2023-10-01

### Added

- CLI command `mtcli coldkey info` to view coldkey information.
- Support Python 3.11 (test fully passed).

### Changed

- Upgrade dependency `cryptography` to 43.0.0.
- Changed struct in `hotkey_manager.py`, separating `import_hotkey` into 2 functions.

### Fixed

- Fix `_hrp` parse error when network=None (#45).
- Fix logger error (does not display "User canceled overwrite").

## [0.2.1] - 2023-09-15

### Fixed

- Patch hotfix: `mnemonic.enc` does not record encoding correctly (#37).
