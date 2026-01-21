// Allocation RPC Module
// JSON-RPC endpoints for token allocation and vesting

use jsonrpc_core::{IoHandler, Params, Error as RpcError, ErrorCode};
use std::sync::Arc;
use parking_lot::RwLock;
use serde::Deserialize;
use serde_json::json;

use luxtensor_consensus::{
    TokenAllocation, AllocationCategory,
};

/// Register allocation RPC methods
pub fn register_allocation_methods(
    io: &mut IoHandler,
    allocation: Arc<RwLock<TokenAllocation>>,
) {
    let alloc = allocation.clone();
    io.add_sync_method("allocation_executeTge", move |_params: Params| {
        let result = alloc.write().execute_tge();
        Ok(json!({
            "tge_timestamp": result.tge_timestamp,
            "total_pre_minted": format!("0x{:x}", result.total_pre_minted),
            "total_pre_minted_decimal": result.total_pre_minted.to_string(),
            "emission_reserved": format!("0x{:x}", result.emission_reserved),
            "emission_reserved_decimal": result.emission_reserved.to_string(),
            "pre_minted": result.pre_minted.iter().map(|(cat, amt)| {
                json!({
                    "category": format!("{:?}", cat),
                    "amount": format!("0x{:x}", amt),
                    "amount_decimal": amt.to_string()
                })
            }).collect::<Vec<_>>()
        }))
    });

    let alloc = allocation.clone();
    io.add_sync_method("allocation_addVesting", move |params: Params| {
        let p: AddVestingParams = params.parse()?;

        let beneficiary = parse_address(&p.beneficiary)?;
        let category = parse_category(&p.category)?;
        let amount = parse_amount(&p.amount)?;

        match alloc.write().add_vesting(beneficiary, category, amount) {
            Ok(()) => Ok(json!({
                "success": true,
                "beneficiary": p.beneficiary,
                "category": p.category,
                "amount": p.amount,
                "message": "Vesting entry added successfully"
            })),
            Err(e) => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: e.to_string(),
                data: None,
            }),
        }
    });

    let alloc = allocation.clone();
    io.add_sync_method("allocation_claim", move |params: Params| {
        let p: ClaimParams = params.parse()?;

        let beneficiary = parse_address(&p.beneficiary)?;
        let current_timestamp = p.timestamp.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time before UNIX epoch")
                .as_secs()
        });

        let result = alloc.write().claim(beneficiary, current_timestamp);

        Ok(json!({
            "beneficiary": p.beneficiary,
            "amount_claimed": format!("0x{:x}", result.amount_claimed),
            "amount_claimed_decimal": result.amount_claimed.to_string(),
            "timestamp": result.timestamp
        }))
    });

    let alloc = allocation.clone();
    io.add_sync_method("allocation_getClaimable", move |params: Params| {
        let p: QueryParams = params.parse()?;

        let beneficiary = parse_address(&p.beneficiary)?;
        let current_timestamp = p.timestamp.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time before UNIX epoch")
                .as_secs()
        });

        let claimable = alloc.read().get_claimable(beneficiary, current_timestamp);

        Ok(json!({
            "beneficiary": p.beneficiary,
            "claimable": format!("0x{:x}", claimable),
            "claimable_decimal": claimable.to_string(),
            "timestamp": current_timestamp
        }))
    });

    let alloc = allocation.clone();
    io.add_sync_method("allocation_getVested", move |params: Params| {
        let p: QueryParams = params.parse()?;

        let beneficiary = parse_address(&p.beneficiary)?;
        let current_timestamp = p.timestamp.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time before UNIX epoch")
                .as_secs()
        });

        let vested = alloc.read().get_vested(beneficiary, current_timestamp);

        Ok(json!({
            "beneficiary": p.beneficiary,
            "vested": format!("0x{:x}", vested),
            "vested_decimal": vested.to_string(),
            "timestamp": current_timestamp
        }))
    });

    let alloc = allocation.clone();
    io.add_sync_method("allocation_getStats", move |_params: Params| {
        let stats = alloc.read().stats();

        Ok(json!({
            "total_supply": format!("0x{:x}", stats.total_supply),
            "total_supply_decimal": stats.total_supply.to_string(),
            "total_pre_minted": format!("0x{:x}", stats.total_pre_minted),
            "total_pre_minted_decimal": stats.total_pre_minted.to_string(),
            "emission_remaining": format!("0x{:x}", stats.emission_remaining),
            "emission_remaining_decimal": stats.emission_remaining.to_string(),
            "allocations": stats.allocations.iter().map(|(cat, amt)| {
                json!({
                    "category": format!("{:?}", cat),
                    "amount": format!("0x{:x}", amt),
                    "amount_decimal": amt.to_string()
                })
            }).collect::<Vec<_>>()
        }))
    });

    let alloc = allocation.clone();
    io.add_sync_method("allocation_mintEmission", move |params: Params| {
        let p: MintParams = params.parse()?;

        let amount = parse_amount(&p.amount)?;

        match alloc.write().mint_emission(amount) {
            Ok(minted) => Ok(json!({
                "success": true,
                "minted": format!("0x{:x}", minted),
                "minted_decimal": minted.to_string(),
                "remaining_emission": format!("0x{:x}", alloc.read().remaining_emission())
            })),
            Err(e) => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: e.to_string(),
                data: None,
            }),
        }
    });

    io.add_sync_method("allocation_getVestingSchedule", move |params: Params| {
        let p: CategoryParams = params.parse()?;

        let category = parse_category(&p.category)?;
        let schedule = category.vesting();

        Ok(json!({
            "category": p.category,
            "cliff_days": schedule.cliff_days,
            "linear_days": schedule.linear_days,
            "tge_percent": schedule.tge_percent,
            "description": schedule.description
        }))
    });

    io.add_sync_method("allocation_getAllCategories", move |_params: Params| {
        let categories = vec![
            AllocationCategory::EmissionRewards,
            AllocationCategory::EcosystemGrants,
            AllocationCategory::TeamCoreDev,
            AllocationCategory::PrivateSale,
            AllocationCategory::IDO,
            AllocationCategory::DaoTreasury,
            AllocationCategory::InitialLiquidity,
            AllocationCategory::FoundationReserve,
        ];

        Ok(json!(categories.iter().map(|cat| {
            let schedule = cat.vesting();
            json!({
                "category": format!("{:?}", cat),
                "percentage": cat.percentage(),
                "amount": format!("0x{:x}", cat.amount()),
                "amount_decimal": cat.amount().to_string(),
                "vesting": {
                    "cliff_days": schedule.cliff_days,
                    "linear_days": schedule.linear_days,
                    "tge_percent": schedule.tge_percent,
                    "description": schedule.description
                }
            })
        }).collect::<Vec<_>>()))
    });
}

// Parameter structs
#[derive(Deserialize)]
struct AddVestingParams {
    beneficiary: String,
    category: String,
    amount: String,
}

#[derive(Deserialize)]
struct ClaimParams {
    beneficiary: String,
    timestamp: Option<u64>,
}

#[derive(Deserialize)]
struct QueryParams {
    beneficiary: String,
    timestamp: Option<u64>,
}

#[derive(Deserialize)]
struct MintParams {
    amount: String,
}

#[derive(Deserialize)]
struct CategoryParams {
    category: String,
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

fn parse_category(cat: &str) -> Result<AllocationCategory, RpcError> {
    match cat.to_lowercase().as_str() {
        "emission_rewards" | "emissionrewards" => Ok(AllocationCategory::EmissionRewards),
        "ecosystem_grants" | "ecosystemgrants" => Ok(AllocationCategory::EcosystemGrants),
        "team_core_dev" | "teamcoredev" | "team" => Ok(AllocationCategory::TeamCoreDev),
        "private_sale" | "privatesale" | "private" => Ok(AllocationCategory::PrivateSale),
        "ido" => Ok(AllocationCategory::IDO),
        "dao_treasury" | "daotreasury" | "treasury" => Ok(AllocationCategory::DaoTreasury),
        "initial_liquidity" | "initialliquidity" | "liquidity" => Ok(AllocationCategory::InitialLiquidity),
        "foundation_reserve" | "foundationreserve" | "foundation" => Ok(AllocationCategory::FoundationReserve),
        _ => Err(RpcError {
            code: ErrorCode::InvalidParams,
            message: format!("Unknown category: {}", cat),
            data: None,
        }),
    }
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
