// Node Tier RPC Module
// JSON-RPC endpoints for progressive staking node management

use crate::helpers::verify_caller_signature;
use jsonrpc_core::{IoHandler, Params, Error as RpcError, ErrorCode};
use std::sync::Arc;
use parking_lot::RwLock;
use serde::Deserialize;
use serde_json::json;

use luxtensor_consensus::{
    NodeRegistry, NodeTier,
    FULL_NODE_STAKE, VALIDATOR_STAKE, SUPER_VALIDATOR_STAKE
};

/// Register node tier RPC methods
pub fn register_node_methods(
    io: &mut IoHandler,
    registry: Arc<RwLock<NodeRegistry>>,
) {
    let reg = registry.clone();
    // SECURITY: Node registration now requires signature verification
    // to prevent attackers from registering nodes under arbitrary addresses.
    io.add_sync_method("node_register", move |params: Params| {
        let p: RegisterParams = params.parse()?;

        let address = parse_address(&p.address)?;
        let stake = parse_amount(&p.stake)?;
        let block_height = p.block_height.unwrap_or(0);

        // Verify timestamp freshness (within 5 minutes)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > p.timestamp + 300 || p.timestamp > now + 60 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Signature expired or future timestamp".to_string(),
                data: None,
            });
        }

        // Verify caller owns the address
        let message = format!("node_register:{}:{}:{}", hex::encode(address), stake, p.timestamp);
        let addr = luxtensor_core::Address::from(address);
        verify_caller_signature(&addr, &message, &p.signature, 0)
            .or_else(|_| verify_caller_signature(&addr, &message, &p.signature, 1))
            .map_err(|_| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Signature verification failed - caller does not own address".to_string(),
                data: None,
            })?;

        match reg.write().register(address, stake, block_height) {
            Ok(tier) => Ok(json!({
                "success": true,
                "address": p.address,
                "stake": p.stake,
                "tier": format!("{:?}", tier),
                "tier_name": tier.name(),
                "can_produce_blocks": tier.can_produce_blocks(),
                "message": "Node registered (signature verified)"
            })),
            Err(e) => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: e.to_string(),
                data: None,
            }),
        }
    });

    let reg = registry.clone();
    // SECURITY: Stake updates now require signature verification
    // to prevent attackers from manipulating other nodes' stakes.
    io.add_sync_method("node_updateStake", move |params: Params| {
        let p: UpdateStakeParams = params.parse()?;

        let address = parse_address(&p.address)?;
        let new_stake = parse_amount(&p.new_stake)?;

        // Verify timestamp freshness (within 5 minutes)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > p.timestamp + 300 || p.timestamp > now + 60 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Signature expired or future timestamp".to_string(),
                data: None,
            });
        }

        // Verify caller owns the address
        let message = format!("node_updateStake:{}:{}:{}", hex::encode(address), new_stake, p.timestamp);
        let addr = luxtensor_core::Address::from(address);
        verify_caller_signature(&addr, &message, &p.signature, 0)
            .or_else(|_| verify_caller_signature(&addr, &message, &p.signature, 1))
            .map_err(|_| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Signature verification failed - caller does not own address".to_string(),
                data: None,
            })?;

        match reg.write().update_stake(address, new_stake) {
            Some(tier) => Ok(json!({
                "success": true,
                "address": p.address,
                "new_stake": p.new_stake,
                "new_tier": format!("{:?}", tier),
                "tier_name": tier.name(),
                "message": "Stake updated (signature verified)"
            })),
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Node not found".to_string(),
                data: None,
            }),
        }
    });

    let reg = registry.clone();
    io.add_sync_method("node_getTier", move |params: Params| {
        let p: AddressParams = params.parse()?;

        let address = parse_address(&p.address)?;

        match reg.read().get_tier(address) {
            Some(tier) => Ok(json!({
                "address": p.address,
                "tier": format!("{:?}", tier),
                "tier_name": tier.name(),
                "emission_share": tier.emission_share(),
                "can_produce_blocks": tier.can_produce_blocks(),
                "receives_infrastructure_rewards": tier.receives_infrastructure_rewards(),
                "receives_validator_rewards": tier.receives_validator_rewards()
            })),
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Node not found".to_string(),
                data: None,
            }),
        }
    });

    let reg = registry.clone();
    io.add_sync_method("node_getInfo", move |params: Params| {
        let p: AddressParams = params.parse()?;

        let address = parse_address(&p.address)?;

        match reg.read().get(address) {
            Some(info) => Ok(json!({
                "address": p.address,
                "tier": format!("{:?}", info.tier),
                "tier_name": info.tier.name(),
                "stake": format!("0x{:x}", info.stake),
                "stake_decimal": info.stake.to_string(),
                "registered_at": info.registered_at,
                "last_active": info.last_active,
                "uptime_score": info.uptime_score,
                "blocks_produced": info.blocks_produced,
                "tx_relayed": info.tx_relayed
            })),
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Node not found".to_string(),
                data: None,
            }),
        }
    });

    let reg = registry.clone();
    // SECURITY: node_unregister now requires signature verification
    // to prevent attackers from force-unregistering any validator node.
    io.add_sync_method("node_unregister", move |params: Params| {
        let p: SignedAddressParams = params.parse()?;

        let address = parse_address(&p.address)?;
        let timestamp: u64 = p.timestamp.parse()
            .map_err(|_| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Invalid timestamp".to_string(),
                data: None,
            })?;

        // Verify timestamp is recent (within 5 minutes)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > timestamp + 300 || timestamp > now + 60 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Signature expired or future timestamp".to_string(),
                data: None,
            });
        }

        // Verify signature
        let message = format!("node_unregister:{}", hex::encode(address));
        let addr = luxtensor_core::Address::from(address);
        let sig_valid = verify_caller_signature(&addr, &message, &p.signature, 0)
            .or_else(|_| verify_caller_signature(&addr, &message, &p.signature, 1));

        if sig_valid.is_err() {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Signature verification failed - caller does not own address".to_string(),
                data: None,
            });
        }

        match reg.write().unregister(address) {
            Some(info) => Ok(json!({
                "success": true,
                "address": p.address,
                "stake_returned": format!("0x{:x}", info.stake),
                "stake_returned_decimal": info.stake.to_string()
            })),
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Node not found".to_string(),
                data: None,
            }),
        }
    });

    let reg = registry.clone();
    io.add_sync_method("node_getValidators", move |_params: Params| {
        let validators = reg.read().get_validators();

        Ok(json!(validators.iter().map(|info| {
            json!({
                "address": format!("0x{}", hex::encode(info.address)),
                "tier": format!("{:?}", info.tier),
                "stake": format!("0x{:x}", info.stake),
                "stake_decimal": info.stake.to_string(),
                "uptime_score": info.uptime_score,
                "blocks_produced": info.blocks_produced
            })
        }).collect::<Vec<_>>()))
    });

    let reg = registry.clone();
    io.add_sync_method("node_getInfrastructureNodes", move |_params: Params| {
        let nodes = reg.read().get_infrastructure_nodes();

        Ok(json!(nodes.iter().map(|info| {
            json!({
                "address": format!("0x{}", hex::encode(info.address)),
                "tier": format!("{:?}", info.tier),
                "stake": format!("0x{:x}", info.stake),
                "stake_decimal": info.stake.to_string()
            })
        }).collect::<Vec<_>>()))
    });

    let reg = registry.clone();
    io.add_sync_method("node_getStats", move |_params: Params| {
        let counts = reg.read().count_by_tier();
        let total = reg.read().total_nodes();
        let total_stake = reg.read().total_stake();

        Ok(json!({
            "total_nodes": total,
            "total_stake": format!("0x{:x}", total_stake),
            "total_stake_decimal": total_stake.to_string(),
            "by_tier": {
                "light_node": counts.get(&NodeTier::LightNode).unwrap_or(&0),
                "full_node": counts.get(&NodeTier::FullNode).unwrap_or(&0),
                "validator": counts.get(&NodeTier::Validator).unwrap_or(&0),
                "super_validator": counts.get(&NodeTier::SuperValidator).unwrap_or(&0)
            }
        }))
    });

    io.add_sync_method("node_getTierRequirements", move |_params: Params| {
        Ok(json!({
            "tiers": [
                {
                    "tier": "LightNode",
                    "min_stake": "0",
                    "min_stake_mdt": 0,
                    "emission_share": 0.0,
                    "can_produce_blocks": false
                },
                {
                    "tier": "FullNode",
                    "min_stake": format!("0x{:x}", FULL_NODE_STAKE),
                    "min_stake_mdt": 10,
                    "emission_share": 0.02,
                    "can_produce_blocks": false
                },
                {
                    "tier": "Validator",
                    "min_stake": format!("0x{:x}", VALIDATOR_STAKE),
                    "min_stake_mdt": 100,
                    "emission_share": 0.28,
                    "can_produce_blocks": true
                },
                {
                    "tier": "SuperValidator",
                    "min_stake": format!("0x{:x}", SUPER_VALIDATOR_STAKE),
                    "min_stake_mdt": 1000,
                    "emission_share": 0.28,
                    "can_produce_blocks": true
                }
            ]
        }))
    });
}

// Parameter structs
#[derive(Deserialize)]
struct RegisterParams {
    address: String,
    stake: String,
    block_height: Option<u64>,
    timestamp: u64,
    signature: String,
}

#[derive(Deserialize)]
struct UpdateStakeParams {
    address: String,
    new_stake: String,
    timestamp: u64,
    signature: String,
}

#[derive(Deserialize)]
struct AddressParams {
    address: String,
}

/// Params for signed operations requiring authentication
#[derive(Deserialize)]
struct SignedAddressParams {
    address: String,
    timestamp: String,
    signature: String,
}

// Helper functions
fn parse_address(addr: &str) -> Result<[u8; 20], RpcError> {
    let addr = addr.strip_prefix("0x").unwrap_or(addr);
    if addr.len() != 40 {
        return Err(RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address length".to_string(),
            data: None,
        });
    }

    let bytes = hex::decode(addr).map_err(|_| RpcError {
        code: ErrorCode::InvalidParams,
        message: "Invalid hex address".to_string(),
        data: None,
    })?;

    let mut result = [0u8; 20];
    result.copy_from_slice(&bytes);
    Ok(result)
}

fn parse_amount(amt: &str) -> Result<u128, RpcError> {
    if amt.starts_with("0x") {
        u128::from_str_radix(&amt[2..], 16).map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hex amount".to_string(),
            data: None,
        })
    } else {
        amt.parse().map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid decimal amount".to_string(),
            data: None,
        })
    }
}
