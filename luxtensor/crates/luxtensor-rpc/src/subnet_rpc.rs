//! Subnet RPC Methods
//!
//! JSON-RPC methods for subnet operations:
//! - subnet_register: Register a new subnet
//! - subnet_getAll: Get all registered subnets
//! - subnet_getInfo: Get subnet info by netuid
//! - subnet_setWeights: Set subnet weights (root validators only)
//! - subnet_getRootValidators: Get root validators list
//! - subnet_getEmissions: Get emission distribution
//!
//! Synced with Python SDK: sdk/luxtensor_client.py

use jsonrpc_core::{IoHandler, Params, Value, Error as RpcError, ErrorCode};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::info;

// Re-use types from luxtensor-core
use luxtensor_core::{SubnetInfo, RootConfig, RootValidatorInfo, SubnetWeights, EmissionShare, SubnetRegistrationResult};

/// Root Subnet state (thread-safe wrapper)
pub type RootSubnet = Arc<RwLock<RootSubnetState>>;

/// Create new Root Subnet instance
pub fn new_root_subnet() -> RootSubnet {
    Arc::new(RwLock::new(RootSubnetState::new()))
}

/// Root Subnet state management
#[derive(Default)]
pub struct RootSubnetState {
    pub subnets: HashMap<u16, SubnetInfo>,
    pub root_validators: Vec<RootValidatorInfo>,
    pub weight_matrix: HashMap<String, SubnetWeights>,
    pub emission_shares: HashMap<u16, u16>,
    pub config: RootConfig,
    next_netuid: u16,
}

impl RootSubnetState {
    pub fn new() -> Self {
        Self {
            subnets: HashMap::new(),
            root_validators: Vec::new(),
            weight_matrix: HashMap::new(),
            emission_shares: HashMap::new(),
            config: RootConfig::default(),
            next_netuid: 1,
        }
    }

    pub fn register_subnet(&mut self, name: String, owner: [u8; 20], block_number: u64) -> SubnetRegistrationResult {
        if self.subnets.len() >= self.config.max_subnets as usize {
            return SubnetRegistrationResult::failure("Maximum subnets reached".to_string());
        }
        let netuid = self.next_netuid;
        self.next_netuid += 1;
        let subnet = SubnetInfo::new(netuid, owner, name, block_number);
        self.subnets.insert(netuid, subnet);
        self.emission_shares.insert(netuid, 0);
        SubnetRegistrationResult::success(netuid, self.config.subnet_registration_cost)
    }

    pub fn get_all_subnets(&self) -> Vec<&SubnetInfo> {
        self.subnets.values().collect()
    }

    pub fn get_subnet(&self, netuid: u16) -> Option<&SubnetInfo> {
        self.subnets.get(&netuid)
    }

    pub fn get_emission_distribution(&self, total_emission: u128) -> Vec<EmissionShare> {
        self.emission_shares.iter().map(|(netuid, share_bps)| {
            let share = *share_bps as f64 / 10000.0;
            let amount = (total_emission as f64 * share) as u128;
            EmissionShare::new(*netuid, share, amount)
        }).collect()
    }

    pub fn set_weights(&mut self, validator: [u8; 20], weights: HashMap<u16, u16>, block_number: u64) -> Result<(), String> {
        let total: u32 = weights.values().map(|&v| v as u32).sum();
        if total > 10000 {
            return Err(format!("Weights sum {} exceeds 10000", total));
        }
        let addr_hex = hex::encode(validator);
        let mut subnet_weights = SubnetWeights::new(validator);
        subnet_weights.weights = weights;
        subnet_weights.block_updated = block_number;
        self.weight_matrix.insert(addr_hex, subnet_weights);
        Ok(())
    }
}

/// Register subnet RPC methods
pub fn register_subnet_methods(io: &mut IoHandler, root_subnet: RootSubnet) {
    // subnet_getAll
    let subnet_state = root_subnet.clone();
    io.add_method("subnet_getAll", move |_params: Params| {
        let subnet_state = subnet_state.clone();
        async move {
            let state = subnet_state.read();
            let subnets: Vec<Value> = state.get_all_subnets().iter().map(|s| json!({
                "netuid": s.netuid,
                "name": s.name,
                "owner": format!("0x{}", hex::encode(s.owner)),
                "registeredAt": s.registered_at,
                "totalStake": format!("0x{:x}", s.total_stake),
                "emissionShare": s.emission_share(),
                "active": s.active
            })).collect();
            Ok(json!(subnets))
        }
    });

    // subnet_getInfo
    let subnet_state = root_subnet.clone();
    io.add_method("subnet_getInfo", move |params: Params| {
        let subnet_state = subnet_state.clone();
        async move {
            let p: Vec<serde_json::Value> = params.parse()?;
            let netuid = p.get(0).and_then(|v| v.as_u64()).ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing netuid".to_string(),
                data: None,
            })? as u16;
            let state = subnet_state.read();
            match state.get_subnet(netuid) {
                Some(s) => Ok(json!({
                    "netuid": s.netuid,
                    "name": s.name,
                    "owner": format!("0x{}", hex::encode(s.owner)),
                    "emissionShare": s.emission_share(),
                    "active": s.active
                })),
                None => Err(RpcError {
                    code: ErrorCode::InvalidParams,
                    message: format!("Subnet {} not found", netuid),
                    data: None,
                })
            }
        }
    });

    // subnet_register
    let subnet_state = root_subnet.clone();
    io.add_method("subnet_register", move |params: Params| {
        let subnet_state = subnet_state.clone();
        async move {
            let p: Vec<serde_json::Value> = params.parse()?;
            let name = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing subnet name".to_string(),
                data: None,
            })?.to_string();
            let owner_str = p.get(1).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing owner address".to_string(),
                data: None,
            })?;
            let owner_str = owner_str.strip_prefix("0x").unwrap_or(owner_str);
            let owner_bytes = hex::decode(owner_str).map_err(|_| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Invalid owner address".to_string(),
                data: None,
            })?;
            if owner_bytes.len() != 20 {
                return Err(RpcError {
                    code: ErrorCode::InvalidParams,
                    message: "Owner address must be 20 bytes".to_string(),
                    data: None,
                });
            }
            let mut owner = [0u8; 20];
            owner.copy_from_slice(&owner_bytes);
            let mut state = subnet_state.write();
            let result = state.register_subnet(name, owner, 0);
            Ok(json!({ "success": result.success, "netuid": result.netuid, "error": result.error }))
        }
    });

    // subnet_getRootValidators
    let subnet_state = root_subnet.clone();
    io.add_method("subnet_getRootValidators", move |_params: Params| {
        let subnet_state = subnet_state.clone();
        async move {
            let state = subnet_state.read();
            let validators: Vec<Value> = state.root_validators.iter().map(|v| json!({
                "address": format!("0x{}", hex::encode(v.address)),
                "stake": format!("0x{:x}", v.stake),
                "rank": v.rank,
                "isActive": v.is_active
            })).collect();
            Ok(json!(validators))
        }
    });

    // subnet_getEmissions
    let subnet_state = root_subnet.clone();
    io.add_method("subnet_getEmissions", move |params: Params| {
        let subnet_state = subnet_state.clone();
        async move {
            // Empty params is valid (uses default emission); parse errors should propagate
            let p: Vec<serde_json::Value> = params.parse().unwrap_or_default();
            let total_emission = match p.get(0).and_then(|v| v.as_str()) {
                Some(s) => {
                    let s = s.strip_prefix("0x").unwrap_or(s);
                    u128::from_str_radix(s, 16)
                        .map_err(|e| jsonrpc_core::Error::invalid_params(format!("Invalid hex emission value: {}", e)))?
                }
                None => 1_000_000_000_000_000_000_000u128, // default 1000 MDT
            };
            let state = subnet_state.read();
            let emissions: Vec<Value> = state.get_emission_distribution(total_emission).iter().map(|e| json!({
                "netuid": e.netuid,
                "share": e.share(),
                "amount": format!("0x{:x}", e.amount)
            })).collect();
            Ok(json!(emissions))
        }
    });

    // subnet_getConfig
    let subnet_state = root_subnet.clone();
    io.add_method("subnet_getConfig", move |_params: Params| {
        let subnet_state = subnet_state.clone();
        async move {
            let state = subnet_state.read();
            Ok(json!({
                "maxSubnets": state.config.max_subnets,
                "maxRootValidators": state.config.max_root_validators,
                "minStakeForRoot": format!("0x{:x}", state.config.min_stake_for_root),
                "subnetRegistrationCost": format!("0x{:x}", state.config.subnet_registration_cost)
            }))
        }
    });

    // =========================================================================
    // SDK Compatibility Methods (subnet_mixin.py)
    // =========================================================================

    // subnet_exists - Check if subnet exists (SDK uses this)
    let subnet_state = root_subnet.clone();
    io.add_method("subnet_exists", move |params: Params| {
        let subnet_state = subnet_state.clone();
        async move {
            let p: Vec<serde_json::Value> = params.parse()?;
            let netuid = p.get(0).and_then(|v| v.as_u64()).ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing netuid".to_string(),
                data: None,
            })? as u16;
            let state = subnet_state.read();
            let exists = state.subnets.contains_key(&netuid);
            Ok(Value::Bool(exists))
        }
    });

    // subnet_getHyperparameters - Get subnet hyperparameters (SDK uses this)
    let subnet_state = root_subnet.clone();
    io.add_method("subnet_getHyperparameters", move |params: Params| {
        let subnet_state = subnet_state.clone();
        async move {
            let p: Vec<serde_json::Value> = params.parse()?;
            let netuid = p.get(0).and_then(|v| v.as_u64()).ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing netuid".to_string(),
                data: None,
            })? as u16;
            let state = subnet_state.read();
            match state.get_subnet(netuid) {
                Some(s) => Ok(json!({
                    "netuid": s.netuid,
                    "name": s.name,
                    "tempo": 360,  // Default tempo
                    "rho": 10,
                    "kappa": 32767,
                    "immunity_period": 100,
                    "min_allowed_weights": 1,
                    "max_weights_limit": 1024,
                    "emissionValue": s.emission_share(),
                    "registeredAt": s.registered_at,
                    "owner": format!("0x{}", hex::encode(s.owner))
                })),
                None => Err(RpcError {
                    code: ErrorCode::InvalidParams,
                    message: format!("Subnet {} not found", netuid),
                    data: None,
                })
            }
        }
    });

    // subnet_getCount - Get total subnet count (SDK uses this)
    let subnet_state = root_subnet.clone();
    io.add_method("subnet_getCount", move |_params: Params| {
        let subnet_state = subnet_state.clone();
        async move {
            let state = subnet_state.read();
            let count = state.subnets.len();
            Ok(Value::Number(count.into()))
        }
    });

    info!("📡 Registered subnet RPC methods");
}

