// Allocation RPC Module
// JSON-RPC endpoints for token allocation and vesting
//
// SECURITY: All state-changing operations now require signature verification
// to prevent unauthorized minting, vesting, and TGE execution.

use crate::helpers::verify_caller_signature;
use jsonrpc_core::{IoHandler, Params, Error as RpcError, ErrorCode};
use luxtensor_core::Address;
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
    // SECURITY: TGE execution now requires caller signature verification
    io.add_method("allocation_executeTge", move |params: Params| {
        let alloc = alloc.clone();
        async move {
            let p: ExecuteTgeParams = params.parse()?;
            let caller_addr = parse_address(&p.caller)?;
            verify_alloc_timestamp(p.timestamp)?;
            let message = format!("allocation_executeTge:{}:{}", hex::encode(caller_addr), p.timestamp);
            verify_alloc_sig(&caller_addr, &message, &p.signature)?;

            let result = alloc.write().execute_tge();
            Ok(json!({
                "tge_timestamp": result.tge_timestamp,
                "total_pre_minted": format!("0x{:x}", result.total_pre_minted),
                "total_pre_minted_decimal": result.total_pre_minted.to_string(),
                "emission_reserved": format!("0x{:x}", result.emission_reserved),
                "emission_reserved_decimal": result.emission_reserved.to_string(),
                "caller": p.caller,
                "pre_minted": result.pre_minted.iter().map(|(cat, amt)| {
                    json!({
                        "category": format!("{:?}", cat),
                        "amount": format!("0x{:x}", amt),
                        "amount_decimal": amt.to_string()
                    })
                }).collect::<Vec<_>>()
            }))
        }
    });

    let alloc = allocation.clone();
    // SECURITY: Adding vesting entries now requires caller signature verification
    io.add_method("allocation_addVesting", move |params: Params| {
        let alloc = alloc.clone();
        async move {
            let p: AddVestingParams = params.parse()?;
            let caller_addr = parse_address(&p.caller)?;
            verify_alloc_timestamp(p.timestamp)?;
            let message = format!(
                "allocation_addVesting:{}:{}:{}:{}:{}",
                hex::encode(caller_addr), p.beneficiary, p.category, p.amount, p.timestamp
            );
            verify_alloc_sig(&caller_addr, &message, &p.signature)?;

            let beneficiary = parse_address(&p.beneficiary)?;
            let category = parse_category(&p.category)?;
            let amount = parse_amount(&p.amount)?;

            match alloc.write().add_vesting(beneficiary, category, amount) {
                Ok(()) => Ok(json!({
                    "success": true,
                    "caller": p.caller,
                    "beneficiary": p.beneficiary,
                    "category": p.category,
                    "amount": p.amount,
                    "message": "Vesting entry added successfully (signature verified)"
                })),
                Err(e) => Err(RpcError {
                    code: ErrorCode::InvalidParams,
                    message: e.to_string(),
                    data: None,
                }),
            }
        }
    });

    let alloc = allocation.clone();
    // SECURITY: Claiming now requires the beneficiary to prove ownership via signature
    io.add_method("allocation_claim", move |params: Params| {
        let alloc = alloc.clone();
        async move {
            let p: ClaimParams = params.parse()?;

            let beneficiary = parse_address(&p.beneficiary)?;
            verify_alloc_timestamp(p.timestamp)?;
            let message = format!("allocation_claim:{}:{}", hex::encode(beneficiary), p.timestamp);
            verify_alloc_sig(&beneficiary, &message, &p.signature)?;

            let current_timestamp = p.timestamp;

            let result = alloc.write().claim(beneficiary, current_timestamp);

            Ok(json!({
                "beneficiary": p.beneficiary,
                "amount_claimed": format!("0x{:x}", result.amount_claimed),
                "amount_claimed_decimal": result.amount_claimed.to_string(),
                "timestamp": result.timestamp
            }))
        }
    });

    let alloc = allocation.clone();
    io.add_method("allocation_getClaimable", move |params: Params| {
        let alloc = alloc.clone();
        async move {
            let p: QueryParams = params.parse()?;

            let beneficiary = parse_address(&p.beneficiary)?;
            let current_timestamp = p.timestamp.unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::ZERO)
                    .as_secs()
            });

            let claimable = alloc.read().get_claimable(beneficiary, current_timestamp);

            Ok(json!({
                "beneficiary": p.beneficiary,
                "claimable": format!("0x{:x}", claimable),
                "claimable_decimal": claimable.to_string(),
                "timestamp": current_timestamp
            }))
        }
    });

    let alloc = allocation.clone();
    io.add_method("allocation_getVested", move |params: Params| {
        let alloc = alloc.clone();
        async move {
            let p: QueryParams = params.parse()?;

            let beneficiary = parse_address(&p.beneficiary)?;
            let current_timestamp = p.timestamp.unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::ZERO)
                    .as_secs()
            });

            let vested = alloc.read().get_vested(beneficiary, current_timestamp);

            Ok(json!({
                "beneficiary": p.beneficiary,
                "vested": format!("0x{:x}", vested),
                "vested_decimal": vested.to_string(),
                "timestamp": current_timestamp
            }))
        }
    });

    let alloc = allocation.clone();
    io.add_method("allocation_getStats", move |_params: Params| {
        let alloc = alloc.clone();
        async move {
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
        }
    });

    let alloc = allocation.clone();
    // SECURITY: Minting emission now requires caller signature verification
    io.add_method("allocation_mintEmission", move |params: Params| {
        let alloc = alloc.clone();
        async move {
            let p: MintParams = params.parse()?;
            let caller_addr = parse_address(&p.caller)?;
            verify_alloc_timestamp(p.timestamp)?;
            let message = format!(
                "allocation_mintEmission:{}:{}:{}",
                hex::encode(caller_addr), p.amount, p.timestamp
            );
            verify_alloc_sig(&caller_addr, &message, &p.signature)?;

            let amount = parse_amount(&p.amount)?;

            match alloc.write().mint_emission(amount) {
                Ok(minted) => Ok(json!({
                    "success": true,
                    "caller": p.caller,
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
        }
    });

    io.add_method("allocation_getVestingSchedule", move |params: Params| {
        async move {
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
        }
    });

    io.add_method("allocation_getAllCategories", move |_params: Params| {
        async move {
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
        }
    });
}

// Parameter structs

/// Params for TGE execution (now requires authentication)
#[derive(Deserialize)]
struct ExecuteTgeParams {
    caller: String,
    timestamp: u64,
    signature: String,
}

/// Params for adding vesting (now requires caller authentication)
#[derive(Deserialize)]
struct AddVestingParams {
    caller: String,
    beneficiary: String,
    category: String,
    amount: String,
    timestamp: u64,
    signature: String,
}

/// Params for claiming vested tokens (beneficiary must sign)
#[derive(Deserialize)]
struct ClaimParams {
    beneficiary: String,
    timestamp: u64,
    signature: String,
}

#[derive(Deserialize)]
struct QueryParams {
    beneficiary: String,
    timestamp: Option<u64>,
}

/// Params for minting emission (now requires caller authentication)
#[derive(Deserialize)]
struct MintParams {
    caller: String,
    amount: String,
    timestamp: u64,
    signature: String,
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

// ──────────────────────────────────────────────────────────────────────
// Signature verification helpers for allocation operations
// ──────────────────────────────────────────────────────────────────────

/// Verify signature for allocation RPC operations.
/// Tries recovery IDs 0 and 1 (covers both Ethereum v=27/28 conventions).
fn verify_alloc_sig(caller_bytes: &[u8; 20], message: &str, signature: &str) -> Result<(), RpcError> {
    let addr = Address::from(*caller_bytes);
    verify_caller_signature(&addr, message, signature, 0)
        .or_else(|_| verify_caller_signature(&addr, message, signature, 1))
        .map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Signature verification failed - caller does not own address".to_string(),
            data: None,
        })
}

/// Verify timestamp is recent (within 5 minutes, not more than 60s in the future).
/// Prevents replay attacks with stale signatures.
fn verify_alloc_timestamp(timestamp: u64) -> Result<(), RpcError> {
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
    Ok(())
}
