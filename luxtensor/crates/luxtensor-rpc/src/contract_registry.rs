// Global contract registry for syncing deployed contracts between executor and RPC
// This allows executor to store deployed contract code that eth_getCode can read

use parking_lot::RwLock;
use std::collections::HashMap;

/// Deployed contract info
#[derive(Clone, Debug)]
pub struct DeployedContractInfo {
    pub code: Vec<u8>,
    pub deployer: [u8; 20],
    pub deploy_block: u64,
}

lazy_static::lazy_static! {
    /// Global contract registry: address -> contract info
    static ref CONTRACT_REGISTRY: RwLock<HashMap<[u8; 20], DeployedContractInfo>> = {
        RwLock::new(HashMap::new())
    };
}

/// Register a deployed contract (called by executor after successful deploy)
pub fn register_contract(address: [u8; 20], code: Vec<u8>, deployer: [u8; 20], deploy_block: u64) {
    let info = DeployedContractInfo {
        code,
        deployer,
        deploy_block,
    };
    CONTRACT_REGISTRY.write().insert(address, info);
    tracing::info!("📦 Contract registered at 0x{}", hex::encode(&address));
}

/// Get contract code by address
pub fn get_contract_code(address: &[u8; 20]) -> Option<Vec<u8>> {
    CONTRACT_REGISTRY.read().get(address).map(|info| info.code.clone())
}

/// Check if contract exists
pub fn contract_exists(address: &[u8; 20]) -> bool {
    CONTRACT_REGISTRY.read().contains_key(address)
}

/// Get contract info
pub fn get_contract_info(address: &[u8; 20]) -> Option<DeployedContractInfo> {
    CONTRACT_REGISTRY.read().get(address).cloned()
}
