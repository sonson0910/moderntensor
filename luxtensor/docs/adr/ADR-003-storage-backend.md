# ADR-003: Storage Backend Selection

**Status**: Accepted
**Date**: 2026-01-31
**Authors**: Luxtensor Team

---

## Context

Blockchain storage must handle high write throughput, fast reads, and efficient pruning.

## Decision

**Storage backend: RocksDB**

### Alternatives Considered

| Backend | Pros | Cons | Decision |
|---------|------|------|----------|
| **RocksDB** | Fast writes, proven, LSM-tree | Memory usage | ✅ |
| LevelDB | Simple | Slower than RocksDB | ❌ |
| LMDB | Fast reads | Limited compaction | ❌ |
| SQLite | Familiar | Not optimized for blockchain | ❌ |

### Configuration

```rust
RocksDBConfig {
    cache_size: 256 * 1024 * 1024,  // 256 MB
    max_open_files: 1024,
    compression: LZ4,
}
```

## Implementation

- `luxtensor-storage/src/db.rs`: RocksDB wrapper
- `luxtensor-storage/src/maintenance.rs`: Compaction/pruning
