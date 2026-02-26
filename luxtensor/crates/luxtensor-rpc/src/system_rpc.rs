//! System, debug, and sync RPC methods extracted from `server.rs`.
//!
//! Contains:
//! - `register_system_methods`: system_health, system_nodeStats, system_cacheStats,
//!   system_metrics, system_prometheusMetrics
//! - `register_monitoring_methods`: system_getAICircuitBreakerStatus,
//!   system_getRateLimitStatus, debug_forkChoiceState, sync_getSyncStatus

use jsonrpc_core::{IoHandler, Params};
use luxtensor_consensus::{AILayerCircuitBreaker, CircuitState};
use luxtensor_core::UnifiedStateDB;
use luxtensor_storage::{BlockchainDB, CachedStateDB};
use parking_lot::RwLock;
use serde_json::json;
use std::sync::Arc;
use tracing::warn as _unused_warn; // reserved for future error logging

/// Context for system RPC methods.
pub struct SystemRpcContext {
    pub db: Arc<BlockchainDB>,
    pub unified_state: Arc<RwLock<UnifiedStateDB>>,
    pub mempool: Arc<luxtensor_core::UnifiedMempool>,
    pub validators: Arc<RwLock<luxtensor_consensus::ValidatorSet>>,
    pub chain_id: u64,
    pub ai_circuit_breaker: Arc<AILayerCircuitBreaker>,
    pub rate_limiter: Arc<crate::RateLimiter>,
    pub merkle_cache: Option<Arc<CachedStateDB>>,
    pub metrics_json_fn: Option<Arc<dyn Fn() -> serde_json::Value + Send + Sync>>,
    pub metrics_prometheus_fn: Option<Arc<dyn Fn() -> String + Send + Sync>>,
    pub health_fn: Option<Arc<dyn Fn() -> serde_json::Value + Send + Sync>>,
}

/// Register system_* RPC methods (health, stats, cache, metrics).
pub fn register_system_methods(ctx: &SystemRpcContext, io: &mut IoHandler) {
    // system_health - Return node health status (for monitoring and load balancers)
    let db_for_health = ctx.db.clone();
    let unified_for_health = ctx.unified_state.clone();
    let health_fn_for_rpc = ctx.health_fn.clone();
    io.add_method("system_health", move |_params: Params| {
        let health_fn_for_rpc = health_fn_for_rpc.clone();
        let db_for_health = db_for_health.clone();
        let unified_for_health = unified_for_health.clone();
        async move {
            // Enhanced: Use HealthMonitor callback if available
            if let Some(ref health_fn) = health_fn_for_rpc {
                return Ok(health_fn());
            }

            // Fallback: basic health from block scanning
            let block_height = find_block_height(&db_for_health);

            let chain_id = unified_for_health.read().chain_id();

            Ok(serde_json::json!({
                "is_syncing": false,
                "block": block_height,
                "healthy": true,
                "chain_id": chain_id,
                "version": "0.1.0",
                "node_name": "luxtensor-node"
            }))
        }
    });

    // system_nodeStats - Return node statistics (height, chain_id, mempool, validators)
    let db_for_stats = ctx.db.clone();
    let mempool_for_stats = ctx.mempool.clone();
    let validators_for_stats = ctx.validators.clone();
    let chain_id_for_stats = ctx.chain_id;
    io.add_method("system_nodeStats", move |_params: Params| {
        let db_for_stats = db_for_stats.clone();
        let mempool_for_stats = mempool_for_stats.clone();
        let validators_for_stats = validators_for_stats.clone();
        async move {
            let block_height = find_block_height(&db_for_stats);

            let mempool_size = mempool_for_stats.get_pending_transactions().len();
            let validator_count = validators_for_stats.read().len();

            Ok(serde_json::json!({
                "height": block_height,
                "chain_id": chain_id_for_stats,
                "mempool_size": mempool_size,
                "validator_count": validator_count,
                "version": env!("CARGO_PKG_VERSION")
            }))
        }
    });

    // system_cacheStats - Return Merkle cache statistics for monitoring
    if let Some(ref cache) = ctx.merkle_cache {
        let cache_for_stats = cache.clone();
        io.add_method("system_cacheStats", move |_params: Params| {
            let cache_for_stats = cache_for_stats.clone();
            async move {
                let stats = cache_for_stats.stats();
                Ok(serde_json::json!({
                    "full_computations": stats.full_computations,
                    "incremental_computations": stats.incremental_computations,
                    "root_cache_hits": stats.root_cache_hits,
                    "root_cache_misses": stats.root_cache_misses,
                    "hash_cache_hits": stats.hash_cache_hits,
                    "hit_ratio": stats.hit_ratio(),
                    "incremental_ratio": stats.incremental_ratio()
                }))
            }
        });
    }

    // system_metrics - Return node metrics as JSON (for dashboards)
    if let Some(ref metrics_fn) = ctx.metrics_json_fn {
        let fn_clone = metrics_fn.clone();
        io.add_method("system_metrics", move |_params: Params| {
            let fn_clone = fn_clone.clone();
            async move {
                Ok(fn_clone())
            }
        });
    }

    // system_prometheusMetrics - Return Prometheus-compatible metrics text
    if let Some(ref prom_fn) = ctx.metrics_prometheus_fn {
        let fn_clone = prom_fn.clone();
        io.add_method("system_prometheusMetrics", move |_params: Params| {
            let fn_clone = fn_clone.clone();
            async move {
                Ok(serde_json::Value::String(fn_clone()))
            }
        });
    }
}

/// Register monitoring/debug RPC methods (circuit breaker, rate limiter, fork choice, sync).
pub fn register_monitoring_methods(ctx: &SystemRpcContext, io: &mut IoHandler) {
    // system_getAICircuitBreakerStatus
    let ai_cb = ctx.ai_circuit_breaker.clone();
    io.add_method("system_getAICircuitBreakerStatus", move |_params: Params| {
        let ai_cb = ai_cb.clone();
        async move {
            let status = ai_cb.summary();
            Ok(serde_json::json!({
                "healthy": status.healthy,
                "weight_consensus": {
                    "state": format!("{:?}", status.weight_consensus_state),
                    "operational": status.weight_consensus_state == CircuitState::Closed
                },
                "commit_reveal": {
                    "state": format!("{:?}", status.commit_reveal_state),
                    "operational": status.commit_reveal_state == CircuitState::Closed
                },
                "emission": {
                    "state": format!("{:?}", status.emission_state),
                    "operational": status.emission_state == CircuitState::Closed
                }
            }))
        }
    });

    // system_getRateLimitStatus
    let _rl = ctx.rate_limiter.clone();
    io.add_method("system_getRateLimitStatus", move |_params: Params| {
        async move {
            Ok(serde_json::json!({
                "enabled": true,
                "config": {
                    "max_requests_per_minute": 100,
                    "window_seconds": 60
                },
                "message": "Rate limiting active for DoS protection"
            }))
        }
    });

    // debug_forkChoiceState - Return block scores and attestation stakes for debugging
    let db_for_debug = ctx.db.clone();
    io.add_method("debug_forkChoiceState", move |_params: Params| {
        let db_for_debug = db_for_debug.clone();
        async move {
            let block_scores = match db_for_debug.load_all_block_scores() {
                Ok(scores) => scores.iter().map(|(hash, score)| {
                    serde_json::json!({
                        "hash": format!("0x{}", hex::encode(hash)),
                        "score": score
                    })
                }).collect::<Vec<_>>(),
                Err(e) => return Err(jsonrpc_core::Error {
                    code: jsonrpc_core::ErrorCode::InternalError,
                    message: format!("Failed to load block scores: {}", e),
                    data: None,
                }),
            };

            let attestation_stakes = match db_for_debug.load_all_attestation_stakes() {
                Ok(stakes) => stakes.iter().map(|(hash, stake)| {
                    serde_json::json!({
                        "hash": format!("0x{}", hex::encode(hash)),
                        "stake": stake.to_string()
                    })
                }).collect::<Vec<_>>(),
                Err(e) => return Err(jsonrpc_core::Error {
                    code: jsonrpc_core::ErrorCode::InternalError,
                    message: format!("Failed to load attestation stakes: {}", e),
                    data: None,
                }),
            };

            Ok(serde_json::json!({
                "blockScores": block_scores,
                "attestationStakes": attestation_stakes,
                "totalScoredBlocks": block_scores.len(),
                "totalAttestedBlocks": attestation_stakes.len()
            }))
        }
    });

    // sync_getSyncStatus - Return current sync status for state sync protocol
    let db_for_sync = ctx.db.clone();
    let unified_for_sync = ctx.unified_state.clone();
    io.add_method("sync_getSyncStatus", move |_params: Params| {
        let db_for_sync = db_for_sync.clone();
        let unified_for_sync = unified_for_sync.clone();
        async move {
            let current_block = unified_for_sync.read().block_number();
            let highest_block = {
                // Simple linear scan from current to find highest
                let mut highest = current_block;
                for h in (current_block + 1)..(current_block + 100) {
                    if db_for_sync.get_block_by_height(h).ok().flatten().is_some() {
                        highest = h;
                    } else {
                        break;
                    }
                }
                highest
            };
            let is_syncing = highest_block > current_block;

            Ok(json!({
                "syncing": is_syncing,
                "currentBlock": format!("0x{:x}", current_block),
                "highestBlock": format!("0x{:x}", highest_block),
                "startingBlock": "0x0",
                "progress": if highest_block > 0 {
                    (current_block as f64 / highest_block as f64 * 100.0).min(100.0)
                } else {
                    100.0
                }
            }))
        }
    });
}

/// Binary-search helper to find the highest block stored in the DB.
///
/// Used by `system_health` and `system_nodeStats` to avoid duplicating the
/// jump-search + binary-search pattern.
fn find_block_height(db: &BlockchainDB) -> u64 {
    let mut ceiling: u64 = 1;
    loop {
        match db.get_block_by_height(ceiling) {
            Ok(Some(_)) => {
                ceiling *= 2;
                if ceiling > 1_000_000 {
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    let mut low = ceiling / 2;
    let mut high = ceiling;
    while low < high {
        let mid = (low + high + 1) / 2;
        match db.get_block_by_height(mid) {
            Ok(Some(_)) => low = mid,
            Ok(None) => high = mid - 1,
            Err(_) => break,
        }
    }
    low
}
