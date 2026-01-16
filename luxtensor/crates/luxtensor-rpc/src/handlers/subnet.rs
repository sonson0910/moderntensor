// Subnet RPC handlers
// Extracted from server.rs

use crate::types::SubnetInfo;
use jsonrpc_core::{Params, Value};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Register subnet-related RPC methods
pub fn register_subnet_handlers(
    io: &mut jsonrpc_core::IoHandler,
    subnets: Arc<RwLock<HashMap<u64, SubnetInfo>>>,
) {
    let subnets_clone = subnets.clone();

    // subnet_getInfo - Get subnet information
    io.add_sync_method("subnet_getInfo", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let subnets_map = subnets_clone.read();

        if let Some(subnet) = subnets_map.get(&subnet_id) {
            let subnet_json = serde_json::json!({
                "id": subnet.id,
                "name": subnet.name,
                "owner": subnet.owner,
                "emission_rate": format!("0x{:x}", subnet.emission_rate),
                "participant_count": subnet.participant_count,
                "total_stake": format!("0x{:x}", subnet.total_stake),
                "created_at": format!("0x{:x}", subnet.created_at),
            });
            Ok(subnet_json)
        } else {
            Ok(Value::Null)
        }
    });

    let subnets_clone = subnets.clone();

    // subnet_listAll - List all subnets
    io.add_sync_method("subnet_listAll", move |_params: Params| {
        let subnets_map = subnets_clone.read();

        let subnets_list: Vec<Value> = subnets_map
            .values()
            .map(|subnet| {
                serde_json::json!({
                    "id": subnet.id,
                    "name": subnet.name,
                    "owner": subnet.owner,
                    "emission_rate": format!("0x{:x}", subnet.emission_rate),
                    "participant_count": subnet.participant_count,
                    "total_stake": format!("0x{:x}", subnet.total_stake),
                })
            })
            .collect();

        Ok(Value::Array(subnets_list))
    });

    let subnets_clone = subnets.clone();

    // subnet_create - Create a new subnet
    io.add_sync_method("subnet_create", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 3 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet name, owner, or emission rate",
            ));
        }

        let name = parsed[0]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid name"))?
            .to_string();

        let owner = parsed[1]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid owner"))?
            .to_string();

        let emission_rate_str = parsed[2]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid emission rate"))?;
        let emission_rate = u128::from_str_radix(emission_rate_str.trim_start_matches("0x"), 16)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid emission rate format"))?;

        let mut subnets_map = subnets_clone.write();
        let subnet_id = subnets_map.len() as u64;

        let subnet = SubnetInfo {
            id: subnet_id,
            name,
            owner,
            emission_rate,
            participant_count: 0,
            total_stake: 0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        subnets_map.insert(subnet_id, subnet);

        Ok(serde_json::json!({
            "success": true,
            "subnet_id": subnet_id
        }))
    });

    // subnet_getCount - Get total subnet count
    let subnets_clone = subnets.clone();
    io.add_sync_method("subnet_getCount", move |_params: Params| {
        let subnets_map = subnets_clone.read();
        Ok(Value::Number(subnets_map.len().into()))
    });
}
