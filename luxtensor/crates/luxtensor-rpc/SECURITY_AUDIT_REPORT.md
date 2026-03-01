# LuxTensor RPC Crate — API Security Audit Report

**Scope:** `luxtensor-rpc` crate (~14,700 lines of Rust)
**Date:** 2025-07-11
**Methodology:** Manual source-code review of all 30+ source files
**Classification:** CRITICAL / HIGH / MEDIUM / LOW / INFO

---

## Executive Summary

The `luxtensor-rpc` crate exposes JSON-RPC, Ethereum-compatible, and WebSocket endpoints for the LuxTensor Layer 1 PoS blockchain. While individual security primitives (ECDSA signature verification, constant-time API key comparison, rate limiting) are implemented well, **critical gaps exist in their integration**. The admin authentication module is fully coded but **never wired into the request pipeline**, the validation module defines limits that are **never enforced**, and the WebSocket server has **no access controls whatsoever**. Multiple state-changing endpoints (agent, multisig, dispute) accept caller identity as a plain parameter with **no cryptographic proof of ownership**.

**Finding Summary:**

| Severity | Count |
|----------|-------|
| CRITICAL | 3 |
| HIGH     | 5 |
| MEDIUM   | 6 |
| LOW      | 4 |
| INFO     | 3 |

---

## CRITICAL Findings

### C-1: Admin Authentication Never Enforced in Request Pipeline

**File:** [server.rs](src/server.rs#L624-L690), [admin_auth.rs](src/admin_auth.rs)
**Severity:** CRITICAL

The `AdminAuth` struct and `requires_admin_auth()` function are fully implemented in `admin_auth.rs`, but **neither is called during request processing**. The HTTP request middleware in `server.rs` only checks the rate limiter — it never invokes `AdminAuth::authenticate()` or checks `requires_admin_auth()` for the method name.

**Vulnerable code** (`server.rs` lines 638-688, request middleware):

```rust
builder = builder.request_middleware(
    move |request: hyper::Request<hyper::Body>| {
        // ... IP extraction ...
        if !rate_limiter_mw.check(ip) {
            // Rate limit response
        } else {
            // ⚠️ Proceeds directly — NO admin auth check here
            RequestMiddlewareAction::Proceed { ... }
        }
    },
);
```

The `AdminAuth` struct is **never even instantiated** in `RpcServer` or `RpcServerBuilder`. It exists only as dead code with passing unit tests.

**Impact:** Any network-accessible client can call `admin_addPeer`, `admin_removePeer`, `admin_setLogLevel`, `debug_setHead`, `miner_start`, `miner_stop`, `personal_unlockAccount` if they are ever registered.

**Fix:**
```rust
// In build_http_server(), integrate admin auth check:
let admin_auth = Arc::new(AdminAuth::new(admin_config));
builder = builder.request_middleware(move |request| {
    let ip = /* extract IP */;
    if !rate_limiter_mw.check(ip) { /* 429 */ }

    // Parse the JSON body to extract method name
    // For admin methods, require X-Admin-Api-Key header
    let api_key = request.headers()
        .get("x-admin-api-key")
        .and_then(|v| v.to_str().ok());

    // Check later at IoHandler level via MetaIoHandler middleware
    Proceed { request }
});
```

Better approach — use `jsonrpc_core`'s middleware layer to intercept method names and check admin auth before dispatch.

---

### C-2: `eth_sendTransaction` Allows Unsigned TXs on Ethereum Mainnet Chain IDs

**File:** [tx_rpc.rs](src/tx_rpc.rs#L116-L123)
**Severity:** CRITICAL

The "dev chain" guard includes **chain IDs 1 (Ethereum mainnet) and 5 (Goerli)** alongside actual dev chains:

```rust
let is_dev = matches!(chain_id, 8898 | 1337 | 31337 | 1 | 5 | 11155111);
if !is_dev {
    return Err(/* ... */);
}
```

`eth_sendTransaction` creates and broadcasts **unsigned transactions** — the `from` address is accepted as a plain parameter with no signature. If a node is misconfigured with `chain_id = 1`, anyone can forge transactions from any address.

**Impact:** Complete forgery of transactions from arbitrary addresses on any node configured with chain_id 1, 5, or 11155111. Although LuxTensor's production chain_id is 8898, the explicit inclusion of mainnet/testnet IDs creates a false sense of legitimacy.

**Fix:**
```rust
// Only allow on actual dev/test chains
let is_dev = matches!(chain_id, 8898 | 9999 | 1337 | 31337);
// Better: make this a configurable flag, not chain-ID derived
```

---

### C-3: Agent, Multisig, and Dispute RPC Methods Lack Cryptographic Authentication

**File:** [agent_rpc.rs](src/agent_rpc.rs), [multisig_rpc.rs](src/multisig_rpc.rs), [dispute_rpc.rs](src/dispute_rpc.rs)
**Severity:** CRITICAL

Unlike staking/neuron/weight handlers which require ECDSA signatures with timestamp replay protection, the agent, multisig, and dispute modules accept **caller identity as a plain string parameter** with no signature verification:

**agent_rpc.rs — `agent_register`** (line 55):
```rust
let owner_hex = p.get("owner").and_then(|v| v.as_str())
    .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing owner"))?;
// ⚠️ No signature verification — anyone can claim any owner address
```

**agent_rpc.rs — `agent_deregister`** (line 120):
```rust
let caller_hex = p.get("caller").and_then(|v| v.as_str())...;
// ⚠️ Caller is self-declared — anyone can deregister anyone's agent
```

**multisig_rpc.rs — `multisig_approveTransaction`** (line 203):
```rust
let signer_hex = p.get("signer").and_then(|v| v.as_str())...;
// ⚠️ Signer is self-declared — anyone can approve any multisig TX
```

**multisig_rpc.rs — `multisig_proposeTransaction`** (line 146):
```rust
let proposer_hex = p.get("proposer").and_then(|v| v.as_str())...;
// ⚠️ Proposer is self-declared — anyone can propose TXs to any wallet
```

**dispute_rpc.rs — `dispute_submit`** (line 50):
```rust
let challenger_hex = p.get("challenger").and_then(|v| v.as_str())...;
// ⚠️ Challenger is self-declared — anyone can submit disputes impersonating any address
```

**Impact:** Any RPC client can:
- Register/deregister agents as any owner
- Propose and approve multisig transactions as any signer
- Submit disputes as any challenger
- Withdraw agent gas deposits to any address

**Fix:** All state-changing methods must require `timestamp` + `signature` parameters with ECDSA verification, matching the pattern used in `staking_rpc.rs`:
```rust
let message = format!("agent_register:{}:{}", hex::encode(owner), timestamp);
verify_caller_signature(&addr, &message, &signature, recovery_id)?;
```

---

## HIGH Findings

### H-1: WebSocket Server Has No Authentication, Rate Limiting, or Connection Limits

**File:** [websocket.rs](src/websocket.rs)
**Severity:** HIGH

The WebSocket server (`start_ws_server`) accepts connections from **any origin** with **no authentication**, **no rate limiting**, and **no limit on concurrent connections or subscriptions per connection**.

Key issues:
1. **No origin validation** — no `Origin` header check (line ~45):
   ```rust
   let (ws_stream, _) = tokio_tungstenite::accept_async(raw_stream).await?;
   // ⚠️ Accepts any connection from any origin
   ```

2. **No connection limit** — `tokio::spawn` for every connection with no counter:
   ```rust
   tokio::spawn(handle_connection(ws_stream, /* ... */));
   // ⚠️ Unbounded concurrent connections
   ```

3. **No subscription limit per connection** — client can send unlimited `eth_subscribe` messages

4. **No rate limiting** — HTTP rate limiter only applies to the HTTP server, not WebSocket

**Impact:** An attacker can open thousands of WebSocket connections, each subscribing to `newHeads`, `logs`, and `newPendingTransactions`, causing memory exhaustion on the node. The broadcast channels are bounded (4096/1024), but the connection handlers themselves are unbounded.

**Fix:**
```rust
// Add at server level:
static ACTIVE_WS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
const MAX_WS_CONNECTIONS: usize = 100;
const MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 10;

// Before accepting:
if ACTIVE_WS_CONNECTIONS.load(Ordering::Relaxed) >= MAX_WS_CONNECTIONS {
    raw_stream.shutdown(Shutdown::Both).ok();
    return;
}

// Add Origin header validation for browser clients
```

---

### H-2: Debug and System Monitoring Endpoints Expose Internal State Without Authentication

**File:** [system_rpc.rs](src/system_rpc.rs)
**Severity:** HIGH

Multiple endpoints expose sensitive internal node state to any unauthenticated caller:

1. **`debug_forkChoiceState`** — Exposes block scores and attestation stakes:
   ```rust
   io.add_method("debug_forkChoiceState", move |_params| {
       // Returns blockScores, attestationStakes for all blocks
       // ⚠️ No authentication check
   });
   ```

2. **`system_metrics`** / **`system_prometheusMetrics`** — Expose internal performance metrics

3. **`system_nodeStats`** — Exposes validator count, mempool size, pending TX count

4. **`system_cacheStats`** — Exposes internal cache hit/miss ratios

**Impact:** Information disclosure enables targeted attacks — an attacker can observe mempool size to time attacks, monitor attestation stakes to identify consensus manipulation opportunities, and use metric data to profile node performance characteristics for DoS.

**Fix:** Gate these behind `requires_admin_auth()` or a separate monitoring auth token.

---

### H-3: Validation Module (`RpcLimits`) Defined But Never Applied

**File:** [validation.rs](src/validation.rs)
**Severity:** HIGH

The `validation.rs` module defines comprehensive input validation with configurable limits:

```rust
pub struct RpcLimits {
    pub max_request_size: usize,      // 1MB
    pub max_array_items: usize,       // 1000
    pub max_string_length: usize,     // 256
    pub max_hex_data_size: usize,     // 128KB
}
```

Functions `validate_address`, `validate_hash`, `validate_hex_data`, `validate_u64_hex`, `validate_array_length`, and `validate_string_length` are all well-implemented.

**However, `RpcLimits` is never instantiated and none of these functions are called from any RPC handler.** All handlers use `helpers::parse_address` (which has no length limit on hex input) or inline parsing instead.

**Impact:** The advertised input validation never executes. Large payloads, oversized arrays, and malformed hex data are only caught by downstream parsing (which may not have equivalent protections).

**Fix:** Either:
1. Inject `RpcLimits` into all handler contexts and validate at entry, or
2. Apply validation as middleware before handler dispatch

---

### H-4: No Limit on `eth_getLogs` Block Range — Resource Exhaustion

**File:** [eth_rpc/eth_methods.rs](src/eth_rpc/eth_methods.rs#L1200-L1220)
**Severity:** HIGH

The `eth_getLogs` handler accepts arbitrary `fromBlock` and `toBlock` ranges with no upper bound on the span:

```rust
let filter = parse_log_filter(filter_obj)?;
let current_block = cache_for_logs.block_number();
let logs = store.read().get_logs(&filter, current_block);
// ⚠️ No limit on (toBlock - fromBlock) — could scan 10,000+ blocks
```

While the `LogStore` limits in-memory storage to 10,000 blocks, a query spanning the full range forces iteration over all stored logs.

Similarly, `eth_feeHistory` clamps `block_count` to 1024 (good), but `eth_getLogs` has no equivalent limit.

**Impact:** A single `eth_getLogs` request with `fromBlock: "0x0"` and `toBlock: "latest"` forces the node to scan all 10,000 stored blocks and return all matching logs — CPU and memory spike.

**Fix:**
```rust
const MAX_LOG_BLOCK_RANGE: u64 = 2048;
let range = to.saturating_sub(from);
if range > MAX_LOG_BLOCK_RANGE {
    return Err(RpcError {
        code: ErrorCode::InvalidParams,
        message: format!("Block range too large: max {} blocks", MAX_LOG_BLOCK_RANGE),
        data: None,
    });
}
```

---

### H-5: `eth_getBlockByNumber` Returns Fabricated Block Data

**File:** [eth_rpc/eth_methods.rs](src/eth_rpc/eth_methods.rs#L860-L900)
**Severity:** HIGH

The `eth_getBlockByNumber` handler returns a **hardcoded block object** with all-zero hashes, empty transactions, and a timestamp from `SystemTime::now()` for ANY block number — it **never queries the database**:

```rust
io.add_method("eth_getBlockByNumber", move |params: Params| {
    // ...
    // Return a minimal but valid block object
    Ok(json!({
        "hash": format!("0x{}", hex::encode([0u8; 32])),  // ⚠️ Always zero
        "parentHash": format!("0x{}", hex::encode([0u8; 32])),
        "transactions": [],  // ⚠️ Always empty
        // ...
    }))
});
```

Meanwhile, `eth_getBlockByHash` correctly queries `db.get_block(&hash)` and returns real data.

**Impact:** Any tool or dApp relying on `eth_getBlockByNumber` receives fabricated data. Transaction history, block explorers, and indexers will malfunction. This breaks Ethereum toolchain compatibility (MetaMask, ethers.js, Hardhat).

**Fix:** Query `db.get_block_by_height(block_number)` and return the actual block, as `eth_getBlockByHash` does.

---

## MEDIUM Findings

### M-1: No Pagination on Multiple List Endpoints — Large Response DoS

**Files:** [handlers/neuron.rs](src/handlers/neuron.rs), [handlers/weight.rs](src/handlers/weight.rs), [handlers/metagraph.rs](src/handlers/metagraph.rs), [bridge_rpc.rs](src/bridge_rpc.rs), [agent_rpc.rs](src/agent_rpc.rs)
**Severity:** MEDIUM

Several endpoints return ALL items with no pagination:

| Endpoint | Returns | Risk |
|----------|---------|------|
| `neuron_listBySubnet` | All neurons in subnet | Unbounded |
| `weight_getAllWeights` | All weights for subnet | O(n*m) |
| `lux_getMetagraph` | Full metagraph snapshot | Very large |
| `lux_getAllWeights` | All neurons × all weights | O(n*m) |
| `bridge_listMessages` | All bridge messages | Unbounded |
| `agent_listAll` | All registered agents | Unbounded |
| `eth_pendingTransactions` | All pending txs | Unbounded |

**Impact:** Targeted queries to subnets with many neurons/weights produce multi-MB responses, consume excessive memory, and slow the node.

**Fix:** Add `limit` and `offset` parameters (default limit: 100, max: 1000). Example:
```rust
let limit = p.get("limit").and_then(|v| v.as_u64()).unwrap_or(100).min(1000);
let offset = p.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
```

---

### M-2: Faucet Rate Limiting Is Per-Address, Not Per-IP, and In-Memory Only

**File:** [eth_rpc/faucet.rs](src/eth_rpc/faucet.rs), [eth_rpc/eth_methods.rs](src/eth_rpc/eth_methods.rs#L712-L820)
**Severity:** MEDIUM

1. **Per-address, not per-IP:** An attacker can generate unlimited Ethereum addresses and request faucet drips for each, bypassing the 10-drips/day limit.

2. **In-memory state:** `FaucetRateLimiter` uses a `HashMap` — all rate limiting state is **lost on node restart**, allowing immediate re-dripping to the same addresses.

3. **Configurable amount bypass:** The faucet accepts an optional `amount` parameter, capped at 10× the default (10,000 LUX). While capped, this is still generous.

**Impact:** Faucet funds can be drained rapidly by cycling through generated addresses. On testnet this is acceptable; if the faucet is ever enabled on a production chain, this becomes critical.

**Fix:**
```rust
// Add IP-based rate limiting alongside address-based
pub fn check_and_record_with_ip(&mut self, address: &[u8; 20], ip: IpAddr) -> Result<...> {
    self.check_ip_limit(ip)?;
    self.check_and_record(address)
}

// Persist rate limit state to MetagraphDB for restart resilience
```

---

### M-3: EIP-2930 Access List Ignored in Signing Hash — Potential Signature Mismatch

**File:** [eth_rpc/rlp_decoder.rs](src/eth_rpc/rlp_decoder.rs)
**Severity:** MEDIUM

When computing the signing hash for EIP-2930 (type 1) transactions, the access list is encoded as an **empty RLP list** regardless of the actual transaction content:

```rust
// For EIP-2930 signing hash
let signing_payload = rlp_encode_list(&[
    &chain_id_bytes,
    &nonce_bytes, &gas_price_bytes, &gas_limit_bytes,
    &to_bytes, &value_bytes, &data_bytes,
    &rlp_encode_list(&[]),  // ⚠️ Always empty — ignores actual access list
]);
```

If a user submits an EIP-2930 transaction with a non-empty access list, the signing hash won't match the hash that was actually signed, causing `ecrecover` to recover the **wrong address** — the transaction will be rejected or attributed to a random address.

**Impact:** EIP-2930 transactions with access lists will be silently rejected or misattributed. This breaks Ethereum compatibility for clients using access lists.

**Fix:** Properly encode `items[7]` (the access list) into the signing payload:
```rust
let access_list_rlp = &items[7]; // Use actual access list from TX
```

---

### M-4: X-Forwarded-For Header Spoofing for Rate Limit Bypass

**File:** [server.rs](src/server.rs#L639-L655)
**Severity:** MEDIUM

The rate limiter extracts client IP from `X-Forwarded-For` and `X-Real-IP` headers:

```rust
let ip = request.headers()
    .get("x-forwarded-for")
    .and_then(|v| v.to_str().ok())
    .and_then(|s| s.split(',').next())
    .and_then(|s| s.trim().parse::<std::net::IpAddr>().ok())
    .or_else(|| {
        request.headers().get("x-real-ip")...
    })
    .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
```

If the RPC server is exposed directly (no reverse proxy), an attacker can set `X-Forwarded-For` to a different IP for each request, completely bypassing rate limiting.

**Impact:** Rate limiting becomes ineffective when the server is directly internet-facing.

**Fix:** Add a configuration option to trust proxy headers only when behind a known proxy:
```rust
if self.trust_proxy_headers {
    // Use X-Forwarded-For
} else {
    // Use actual TCP peer address from the connection
}
```

---

### M-5: Contract Address Computation Uses Non-Standard Hashing

**File:** [tx_rpc.rs](src/tx_rpc.rs#L332-L348)
**Severity:** MEDIUM

The `compute_contract_address` function uses `DefaultHasher` (SipHash) instead of the Ethereum-standard RLP + Keccak-256:

```rust
fn compute_contract_address(tx: &Transaction) -> serde_json::Value {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash_slice(from_bytes, &mut hasher);
    std::hash::Hash::hash(&nonce, &mut hasher);
    let hash_val = std::hash::Hasher::finish(&hasher);
    // ⚠️ Non-deterministic across Rust versions (SipHash key changes)
    // ⚠️ Does not match Ethereum's RLP(sender, nonce) -> Keccak-256
}
```

**Impact:** Contract addresses will differ from what Ethereum tools expect. `DefaultHasher` is explicitly documented as not stable across Rust versions, so contract addresses would change after compiler upgrades.

**Fix:**
```rust
use sha3::{Keccak256, Digest};
// Ethereum-standard: keccak256(RLP([sender, nonce]))[12..]
```

---

### M-6: Filters Never Expire — Unbounded Memory Growth

**File:** [logs.rs](src/logs.rs)
**Severity:** MEDIUM

`eth_newFilter`, `eth_newBlockFilter`, and `eth_newPendingTransactionFilter` create `RegisteredFilter` entries stored in a `HashMap`. Filters have a `created_at` field but **no expiration logic**. They are only removed via `eth_uninstallFilter`.

```rust
pub struct RegisteredFilter {
    pub id: String,
    pub filter: LogFilter,
    pub last_block: u64,
    pub created_at: std::time::Instant,  // ⚠️ Never used for expiration
}
```

**Impact:** An attacker can create unlimited filters via `eth_newFilter` calls, each consuming memory indefinitely.

**Fix:**
```rust
// During get_filter_changes or periodically:
const FILTER_TTL: Duration = Duration::from_secs(300); // 5 minutes
filters.retain(|_, f| f.created_at.elapsed() < FILTER_TTL);
```

---

## LOW Findings

### L-1: Error Messages Expose Internal State

**File:** [error.rs](src/error.rs), [staking_rpc.rs](src/staking_rpc.rs)
**Severity:** LOW

Error responses leak internal details:
- Nonce values: `"nonce too low: expected 42 got 3"`
- Balance amounts: `"insufficient balance for transfer"`
- Storage errors: `internal_error(&e.to_string())` propagates raw RocksDB error strings
- Minimum stake amounts: `"Minimum stake is 1000000000000000000 wei"`

**Impact:** Minor information disclosure. Attackers can use nonce information to understand account state.

**Fix:** Use generic error messages for external responses; log details server-side.

---

### L-2: `parse_block_number` Returns 0 for "latest" in deprecated path

**File:** [helpers.rs](src/helpers.rs)
**Severity:** LOW

The deprecated `parse_block_number` function resolves `"latest"` to block `0`:

```rust
pub fn parse_block_number(value: &serde_json::Value) -> Result<u64, RpcError> {
    // "latest" → 0 ⚠️ (should resolve to current block height)
}
```

While marked deprecated, if any code path still uses it, `eth_getBalance` at "latest" would return the genesis balance.

**Fix:** Ensure all callers use the non-deprecated version that resolves to `unified_state.block_number()`.

---

### L-3: `dev_faucet` Chain IDs Differ From `eth_sendTransaction` Chain IDs

**File:** [eth_rpc/eth_methods.rs](src/eth_rpc/eth_methods.rs#L740-L744) vs [tx_rpc.rs](src/tx_rpc.rs#L116-L123)
**Severity:** LOW

Inconsistent chain ID whitelists:

| Endpoint | Allowed Chain IDs |
|----------|------------------|
| `dev_faucet` | 8898, 9999, 1337, 31337 |
| `eth_sendTransaction` | 8898, 1337, 31337, **1, 5, 11155111** |

`dev_faucet` correctly excludes mainnet IDs, but `eth_sendTransaction` includes them. This inconsistency suggests the `eth_sendTransaction` list was not carefully reviewed.

---

### L-4: WebSocket Subscription IDs Are Predictable

**File:** [websocket.rs](src/websocket.rs)
**Severity:** LOW

Subscription IDs are generated from timestamp + counter:
```rust
let sub_id = format!("0x{:x}", timestamp_nanos + counter);
```

While not directly exploitable (subscriptions are per-connection), predictable IDs could theoretically be used in subscription management attacks.

**Fix:** Use `rand::random::<u64>()` or a cryptographic PRNG.

---

## INFO Findings

### I-1: `RpcStateCache` Uses `Ordering::Relaxed`

**File:** [rpc_cache.rs](src/rpc_cache.rs)
**Severity:** INFO

Atomic loads/stores use `Ordering::Relaxed`, which permits stale reads on weakly-ordered architectures (ARM). On x86 this is effectively sequential. For a cache, brief staleness is acceptable by design.

---

### I-2: `eth_accounts` Returns Empty Array (Previously Leaked Dev Keys)

**File:** [eth_rpc/eth_methods.rs](src/eth_rpc/eth_methods.rs)
**Severity:** INFO

A comment indicates this previously returned "hardcoded Hardhat default addresses with publicly-known private keys." It now correctly returns `[]`. The fix is complete; this is noted for audit trail purposes.

---

### I-3: DAO Treasury Balance Publicly Queryable

**File:** [rewards_rpc.rs](src/rewards_rpc.rs)
**Severity:** INFO

`rewards_getDaoBalance` exposes the DAO treasury balance to any caller. While this is public information on-chain, explicitly exposing it via RPC makes reconnaissance easier.

---

## Remediation Priority Matrix

| Priority | Finding | Effort | Impact |
|----------|---------|--------|--------|
| **P0 — Immediate** | C-1: Wire admin auth into pipeline | Medium | Prevents admin endpoint abuse |
| **P0 — Immediate** | C-2: Fix `eth_sendTransaction` chain ID whitelist | Trivial | Prevents TX forgery |
| **P0 — Immediate** | C-3: Add ECDSA auth to agent/multisig/dispute | High | Prevents impersonation |
| **P1 — This Sprint** | H-1: Add WS connection limits + auth | Medium | Prevents WS DoS |
| **P1 — This Sprint** | H-2: Gate debug/system endpoints | Low | Prevents info disclosure |
| **P1 — This Sprint** | H-3: Apply RpcLimits validation | Medium | Enables input validation |
| **P1 — This Sprint** | H-4: Cap `eth_getLogs` block range | Trivial | Prevents resource exhaustion |
| **P1 — This Sprint** | H-5: Fix `eth_getBlockByNumber` to query DB | Medium | Restores Eth compatibility |
| **P2 — Next Sprint** | M-1 through M-6 | Varies | Defense in depth |
| **P3 — Backlog** | L-1 through L-4, I-1 through I-3 | Low | Hardening |

---

## Appendix: Files Reviewed

| File | Lines | Key Concern |
|------|-------|-------------|
| `server.rs` | 885 | Admin auth not wired |
| `admin_auth.rs` | 223 | Dead code |
| `rate_limiter.rs` | 206 | HTTP only |
| `validation.rs` | 211 | Never applied |
| `error.rs` | 175 | Info leakage |
| `helpers.rs` | 235 | Sig verification OK |
| `types.rs` | 200+ | Clean |
| `eth_rpc/mod.rs` | 65 | Clean |
| `eth_rpc/eth_methods.rs` | 1674 | Multiple issues |
| `eth_rpc/rlp_decoder.rs` | 910 | Access list bug |
| `eth_rpc/faucet.rs` | 88 | Per-address only |
| `websocket.rs` | 490 | No access control |
| `handlers/staking.rs` | 1181 | Well-secured |
| `handlers/neuron.rs` | 665 | No pagination |
| `handlers/subnet.rs` | 414 | Clean |
| `handlers/weight.rs` | 316 | No pagination |
| `handlers/metagraph.rs` | 561 | O(n*m) queries |
| `handlers/checkpoint.rs` | 134 | Read-only, clean |
| `tx_rpc.rs` | 408 | Chain ID issue |
| `rpc_cache.rs` | 120 | Relaxed ordering OK |
| `logs.rs` | 508 | No filter expiry |
| `broadcaster.rs` | 306 | Clean |
| `blockchain_rpc.rs` | 332 | Clean |
| `system_rpc.rs` | 283 | Unprotected debug |
| `bridge_rpc.rs` | 207 | No pagination |
| `rewards_rpc.rs` | 250 | Clean (has sig auth) |
| `load_balancer.rs` | 466 | Client-side, clean |
| `agent_rpc.rs` | 335 | No auth |
| `dispute_rpc.rs` | 213 | No auth |
| `multisig_rpc.rs` | 369 | No auth |
| `staking_rpc.rs` | 676 | Well-secured |
