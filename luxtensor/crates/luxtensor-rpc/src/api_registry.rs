//! # RPC API Registry
//!
//! Compile-time catalog of all JSON-RPC methods exposed by the LuxTensor node.
//! Provides a single source of truth for the API surface, useful for documentation,
//! SDK generation, and the `rpc_listMethods` introspection endpoint.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Get all methods:
//! let methods = api_registry::ALL_METHODS;
//!
//! // Get methods by category:
//! let eth_methods = api_registry::methods_by_category("eth");
//! ```

use jsonrpc_core::{IoHandler, Params};

/// A registered RPC method with metadata.
#[derive(Debug, Clone)]
pub struct RpcMethodInfo {
    /// Full method name (e.g. "eth_getBalance")
    pub name: &'static str,
    /// Category for grouping (e.g. "eth", "staking", "subnet")
    pub category: &'static str,
    /// Whether this method mutates state
    pub is_write: bool,
    /// Whether this method requires authentication (signature)
    pub requires_auth: bool,
    /// Brief description
    pub description: &'static str,
}

// ============================================================================
// Compile-time API Registry
// ============================================================================

/// Complete catalog of all RPC methods.
///
/// This is the canonical reference for the API surface.
/// When adding a new RPC method, add an entry here too.
pub const ALL_METHODS: &[RpcMethodInfo] = &[
    // ── Ethereum-Compatible (eth_*) ──
    RpcMethodInfo { name: "eth_blockNumber",        category: "eth",      is_write: false, requires_auth: false, description: "Get current block number" },
    RpcMethodInfo { name: "eth_getBalance",          category: "eth",      is_write: false, requires_auth: false, description: "Get account balance" },
    RpcMethodInfo { name: "eth_getTransactionCount", category: "eth",      is_write: false, requires_auth: false, description: "Get account nonce" },
    RpcMethodInfo { name: "eth_getCode",             category: "eth",      is_write: false, requires_auth: false, description: "Get contract bytecode" },
    RpcMethodInfo { name: "eth_call",                category: "eth",      is_write: false, requires_auth: false, description: "Execute contract call (read-only)" },
    RpcMethodInfo { name: "eth_estimateGas",         category: "eth",      is_write: false, requires_auth: false, description: "Estimate gas for transaction" },
    RpcMethodInfo { name: "eth_gasPrice",            category: "eth",      is_write: false, requires_auth: false, description: "Get current gas price" },
    RpcMethodInfo { name: "eth_sendRawTransaction",  category: "eth",      is_write: true,  requires_auth: false, description: "Submit signed transaction" },
    RpcMethodInfo { name: "eth_getTransactionByHash",category: "eth",      is_write: false, requires_auth: false, description: "Get transaction by hash" },
    RpcMethodInfo { name: "eth_getTransactionReceipt",category: "eth",     is_write: false, requires_auth: false, description: "Get transaction receipt" },
    RpcMethodInfo { name: "eth_getBlockByNumber",    category: "eth",      is_write: false, requires_auth: false, description: "Get block by number" },
    RpcMethodInfo { name: "eth_getBlockByHash",      category: "eth",      is_write: false, requires_auth: false, description: "Get block by hash" },
    RpcMethodInfo { name: "eth_chainId",             category: "eth",      is_write: false, requires_auth: false, description: "Get chain ID" },
    RpcMethodInfo { name: "eth_getLogs",              category: "eth",      is_write: false, requires_auth: false, description: "Get event logs with filter" },
    RpcMethodInfo { name: "eth_feeHistory",          category: "eth",      is_write: false, requires_auth: false, description: "Get fee history" },
    RpcMethodInfo { name: "eth_maxPriorityFeePerGas",category: "eth",      is_write: false, requires_auth: false, description: "Get max priority fee" },
    RpcMethodInfo { name: "eth_getStorageAt",        category: "eth",      is_write: false, requires_auth: false, description: "Get storage at position" },
    RpcMethodInfo { name: "eth_accounts",            category: "eth",      is_write: false, requires_auth: false, description: "List accounts (empty)" },
    RpcMethodInfo { name: "net_version",             category: "eth",      is_write: false, requires_auth: false, description: "Get network version" },
    RpcMethodInfo { name: "web3_clientVersion",      category: "eth",      is_write: false, requires_auth: false, description: "Get client version string" },
    RpcMethodInfo { name: "eth_sendTransaction",     category: "eth",      is_write: true,  requires_auth: false, description: "Send transaction (faucet/dev only)" },

    // ── Staking (staking_*) ──
    RpcMethodInfo { name: "staking_getTotalStake",     category: "staking",  is_write: false, requires_auth: false, description: "Get total network stake" },
    RpcMethodInfo { name: "staking_getStake",          category: "staking",  is_write: false, requires_auth: false, description: "Get stake for address" },
    RpcMethodInfo { name: "staking_getValidators",     category: "staking",  is_write: false, requires_auth: false, description: "List all validators" },
    RpcMethodInfo { name: "staking_getActiveValidators",category: "staking", is_write: false, requires_auth: false, description: "List active validators" },
    RpcMethodInfo { name: "staking_getConfig",         category: "staking",  is_write: false, requires_auth: false, description: "Get staking configuration" },
    RpcMethodInfo { name: "staking_getLockInfo",        category: "staking",  is_write: false, requires_auth: false, description: "Get lock info for address" },
    RpcMethodInfo { name: "staking_addStake",          category: "staking",  is_write: true,  requires_auth: true,  description: "Add stake (requires signature)" },
    RpcMethodInfo { name: "staking_removeStake",       category: "staking",  is_write: true,  requires_auth: true,  description: "Remove stake (requires signature)" },
    RpcMethodInfo { name: "staking_claimRewards",      category: "staking",  is_write: true,  requires_auth: true,  description: "Claim rewards (requires signature)" },
    RpcMethodInfo { name: "staking_registerValidator", category: "staking",  is_write: true,  requires_auth: true,  description: "Register as validator (requires signature)" },
    RpcMethodInfo { name: "staking_deactivateValidator",category: "staking", is_write: true,  requires_auth: true,  description: "Deactivate validator (requires signature)" },
    RpcMethodInfo { name: "staking_lockStake",         category: "staking",  is_write: true,  requires_auth: true,  description: "Lock stake for bonus (requires signature)" },
    RpcMethodInfo { name: "staking_lockStakeSeconds",  category: "staking",  is_write: true,  requires_auth: true,  description: "Lock stake in seconds (dev only, requires signature)" },
    RpcMethodInfo { name: "staking_unlockStake",       category: "staking",  is_write: true,  requires_auth: true,  description: "Unlock expired stake (requires signature)" },

    // ── Subnet (subnet_*) ──
    RpcMethodInfo { name: "subnet_getInfo",   category: "subnet", is_write: false, requires_auth: false, description: "Get subnet by ID" },
    RpcMethodInfo { name: "subnet_listAll",   category: "subnet", is_write: false, requires_auth: false, description: "List all subnets" },
    RpcMethodInfo { name: "subnet_create",    category: "subnet", is_write: true,  requires_auth: true,  description: "Create subnet (requires signature)" },
    RpcMethodInfo { name: "subnet_getCount",  category: "subnet", is_write: false, requires_auth: false, description: "Get subnet count" },

    // ── Neuron (neuron_*) ──
    RpcMethodInfo { name: "neuron_register",  category: "neuron", is_write: true,  requires_auth: true,  description: "Register neuron in subnet (requires signature)" },
    RpcMethodInfo { name: "neuron_getInfo",   category: "neuron", is_write: false, requires_auth: false, description: "Get neuron info" },
    RpcMethodInfo { name: "neuron_listAll",   category: "neuron", is_write: false, requires_auth: false, description: "List neurons in subnet" },

    // ── Weight (weight_*) ──
    RpcMethodInfo { name: "weight_set",       category: "weight", is_write: true,  requires_auth: true,  description: "Set weights (requires signature)" },
    RpcMethodInfo { name: "weight_get",       category: "weight", is_write: false, requires_auth: false, description: "Get weights for neuron" },

    // ── AI/ML (ai_*) ──
    RpcMethodInfo { name: "ai_submitTask",    category: "ai",     is_write: true,  requires_auth: false, description: "Submit AI inference task" },
    RpcMethodInfo { name: "ai_getTaskStatus", category: "ai",     is_write: false, requires_auth: false, description: "Get AI task status" },
    RpcMethodInfo { name: "ai_getTaskResult", category: "ai",     is_write: false, requires_auth: false, description: "Get AI task result" },

    // ── Metagraph (lux_*) ──
    RpcMethodInfo { name: "lux_getMetagraph",      category: "metagraph", is_write: false, requires_auth: false, description: "Get full metagraph state" },
    RpcMethodInfo { name: "lux_registerMiner",     category: "metagraph", is_write: true,  requires_auth: true,  description: "Register miner (requires signature)" },
    RpcMethodInfo { name: "lux_listMiners",        category: "metagraph", is_write: false, requires_auth: false, description: "List registered miners" },

    // ── Query (query_*) ──
    RpcMethodInfo { name: "query_getSubnets",      category: "query", is_write: false, requires_auth: false, description: "List subnets (SDK alias)" },
    RpcMethodInfo { name: "query_getSubnetInfo",   category: "query", is_write: false, requires_auth: false, description: "Get subnet info (SDK alias)" },
    RpcMethodInfo { name: "query_getNeurons",      category: "query", is_write: false, requires_auth: false, description: "List neurons (SDK alias)" },

    // ── System ──
    RpcMethodInfo { name: "system_health",         category: "system", is_write: false, requires_auth: false, description: "Node health status" },
    RpcMethodInfo { name: "system_version",        category: "system", is_write: false, requires_auth: false, description: "Node version info" },
    RpcMethodInfo { name: "system_peers",          category: "system", is_write: false, requires_auth: false, description: "Connected peer count" },
    RpcMethodInfo { name: "system_nodeInfo",       category: "system", is_write: false, requires_auth: false, description: "Node information" },
    RpcMethodInfo { name: "system_metrics",        category: "system", is_write: false, requires_auth: false, description: "Node metrics (JSON)" },
    RpcMethodInfo { name: "system_prometheus",     category: "system", is_write: false, requires_auth: false, description: "Prometheus metrics export" },

    // ── Admin ──
    RpcMethodInfo { name: "admin_runEpoch",        category: "admin", is_write: true,  requires_auth: true, description: "Trigger Yuma epoch (admin only)" },
    RpcMethodInfo { name: "admin_debugMetagraph",  category: "admin", is_write: false, requires_auth: true, description: "Dump MetagraphDB state (debug)" },

    // ── Bridge ──
    RpcMethodInfo { name: "bridge_deposit",        category: "bridge", is_write: true,  requires_auth: true,  description: "Initiate cross-chain deposit" },
    RpcMethodInfo { name: "bridge_withdraw",       category: "bridge", is_write: true,  requires_auth: true,  description: "Initiate cross-chain withdrawal" },
    RpcMethodInfo { name: "bridge_getStatus",      category: "bridge", is_write: false, requires_auth: false, description: "Get bridge transfer status" },

    // ── Multisig ──
    RpcMethodInfo { name: "multisig_create",       category: "multisig", is_write: true,  requires_auth: true,  description: "Create multisig wallet" },
    RpcMethodInfo { name: "multisig_approve",      category: "multisig", is_write: true,  requires_auth: true,  description: "Approve multisig transaction" },
    RpcMethodInfo { name: "multisig_getInfo",      category: "multisig", is_write: false, requires_auth: false, description: "Get multisig wallet info" },

    // ── Agent ──
    RpcMethodInfo { name: "agent_register",        category: "agent", is_write: true,  requires_auth: true,  description: "Register autonomous agent" },
    RpcMethodInfo { name: "agent_getInfo",         category: "agent", is_write: false, requires_auth: false, description: "Get agent info" },
    RpcMethodInfo { name: "agent_listAll",         category: "agent", is_write: false, requires_auth: false, description: "List registered agents" },

    // ── Dispute ──
    RpcMethodInfo { name: "dispute_submit",        category: "dispute", is_write: true,  requires_auth: true,  description: "Submit fraud proof dispute" },
    RpcMethodInfo { name: "dispute_getStatus",     category: "dispute", is_write: false, requires_auth: false, description: "Get dispute status" },

    // ── Introspection ──
    RpcMethodInfo { name: "rpc_listMethods",       category: "rpc", is_write: false, requires_auth: false, description: "List all available RPC methods" },
];

// ============================================================================
// Helper functions
// ============================================================================

/// Get all methods in a specific category.
pub fn methods_by_category(category: &str) -> Vec<&'static RpcMethodInfo> {
    ALL_METHODS.iter().filter(|m| m.category == category).collect()
}

/// Get all unique categories.
pub fn categories() -> Vec<&'static str> {
    let mut cats: Vec<&str> = ALL_METHODS.iter().map(|m| m.category).collect();
    cats.sort_unstable();
    cats.dedup();
    cats
}

/// Get a summary of the API surface.
pub fn api_summary() -> serde_json::Value {
    let categories = categories();
    let mut cat_details = Vec::new();

    for cat in &categories {
        let methods = methods_by_category(cat);
        let write_count = methods.iter().filter(|m| m.is_write).count();
        let auth_count = methods.iter().filter(|m| m.requires_auth).count();

        cat_details.push(serde_json::json!({
            "category": cat,
            "total_methods": methods.len(),
            "write_methods": write_count,
            "auth_required": auth_count,
            "methods": methods.iter().map(|m| serde_json::json!({
                "name": m.name,
                "write": m.is_write,
                "auth": m.requires_auth,
                "description": m.description,
            })).collect::<Vec<_>>(),
        }));
    }

    serde_json::json!({
        "total_methods": ALL_METHODS.len(),
        "total_categories": categories.len(),
        "categories": cat_details,
    })
}

/// Register the `rpc_listMethods` introspection endpoint.
pub fn register_list_methods(io: &mut IoHandler) {
    io.add_method("rpc_listMethods", move |_params: Params| {
        async move {
            Ok(api_summary())
        }
    });
}
