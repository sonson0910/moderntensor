//! Hardcoded Seed Nodes for Luxtensor Network
//!
//! These are the official seed nodes that every node will try to connect to
//! when starting up. This allows nodes to join the network without any configuration.
//!
//! ## Network Types
//! - **Devnet (Chain ID: 8898)**: Local development, uses mDNS, no seed nodes needed
//! - **Testnet (Chain ID: 9999)**: Public testnet, requires seed nodes
//! - **Mainnet (Chain ID: 8898)**: Production network, requires seed nodes
//!
//! ## Adding New Seed Nodes
//! 1. Run the seed node and get its Peer ID from startup log
//! 2. Add the multiaddr to the appropriate network section below
//! 3. Rebuild and release the binary

use tracing::{info, warn};

/// Get seed nodes for mainnet (Chain ID: 8898)
///
/// # Production Deployment
///
/// Before mainnet launch, seed nodes must be deployed and their addresses
/// added here. Until then, mainnet nodes MUST configure bootstrap nodes
/// manually via config file. The node will log a warning and fall back
/// to mDNS if no seeds and no config bootstrap nodes are available.
///
/// ## Deployment Instructions:
/// 1. Deploy 3+ seed nodes on geographically distributed servers
/// 2. Run each seed node and note its Peer ID from startup logs
/// 3. Update the addresses below with real IPs and Peer IDs
/// 4. Rebuild and release the binary
pub fn mainnet_seeds() -> Vec<String> {
    // No mainnet seed nodes configured yet.
    // Operators MUST provide bootstrap_nodes in their config.toml.
    vec![]
}

/// Get seed nodes for testnet (Chain ID: 9999)
///
/// Before testnet launch, seed nodes must be deployed and their addresses
/// added here. Until then, testnet nodes MUST configure bootstrap nodes
/// manually via config file.
pub fn testnet_seeds() -> Vec<String> {
    // No testnet seed nodes configured yet.
    // Operators MUST provide bootstrap_nodes in their config.toml.
    vec![]
}

/// Get seed nodes for devnet/local development (Chain ID: 8898, 1337, 31337)
/// Uses mDNS for automatic peer discovery on local network
pub fn devnet_seeds() -> Vec<String> {
    // Devnet uses mDNS discovery - no hardcoded seeds needed
    // All nodes on the same LAN will automatically find each other
    vec![]
}

/// Get seed nodes based on chain ID
///
/// Priority:
/// 1. Config file bootstrap_nodes (if specified)
/// 2. Hardcoded seeds for the chain (this function)
/// 3. mDNS discovery (fallback for devnet)
pub fn get_seeds_for_chain(chain_id: u64) -> Vec<String> {
    let seeds = match chain_id {
        8898 => mainnet_seeds(),
        9999 => testnet_seeds(),
        _ => devnet_seeds(), // Chain ID 1337, 31337, etc. = devnet
    };

    if seeds.is_empty() && (chain_id == 8898 || chain_id == 9999) {
        warn!(
            "⚠️  No built-in seed nodes for {} (chain {}). \
             Nodes MUST configure bootstrap_nodes in config.toml or discovery will fail.",
            get_network_name(chain_id),
            chain_id
        );
    } else if !seeds.is_empty() {
        info!("🌐 Found {} built-in seed(s) for chain {}", seeds.len(), chain_id);
    }

    seeds
}

/// Check if the given chain has hardcoded seeds
pub fn has_hardcoded_seeds(chain_id: u64) -> bool {
    !get_seeds_for_chain(chain_id).is_empty()
}

/// 🔧 FIX F19: Validate that the node has at least one way to discover peers.
///
/// For mainnet/testnet, either built-in seeds or config bootstrap_nodes
/// must be present.  Aborts with an error if the node would be isolated.
///
/// # Arguments
/// * `chain_id` - The chain ID of the network
/// * `config_bootstrap_nodes` - Bootstrap nodes from the config file
/// * `enable_mdns` - Whether mDNS is enabled
///
/// # Returns
/// `Err(reason)` if the node cannot discover any peers.
pub fn validate_bootstrap_config(
    chain_id: u64,
    config_bootstrap_nodes: &[String],
    enable_mdns: bool,
) -> Result<(), String> {
    let seeds = get_seeds_for_chain(chain_id);
    let has_seeds = !seeds.is_empty();
    let has_config = !config_bootstrap_nodes.is_empty();

    // Devnet allows mDNS-only operation
    if chain_id != 8898 && chain_id != 9999 {
        if !has_seeds && !has_config && !enable_mdns {
            return Err(format!(
                "No seed nodes, no bootstrap_nodes in config, and mDNS disabled for chain {}. \
                 Node will be completely isolated.",
                chain_id
            ));
        }
        return Ok(());
    }

    // Mainnet/testnet require explicit bootstrap
    if !has_seeds && !has_config {
        return Err(format!(
            "FATAL: {} (chain {}) has no built-in seed nodes and no bootstrap_nodes \
             configured. The node cannot join the network. Add bootstrap_nodes to config.toml.",
            get_network_name(chain_id),
            chain_id
        ));
    }

    Ok(())
}

/// Get network name from chain ID
pub fn get_network_name(chain_id: u64) -> &'static str {
    match chain_id {
        8898 => "Mainnet",
        9999 => "Testnet",
        1337 | 31337 => "Local",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_devnet_uses_mdns() {
        // Devnet should have no seeds (uses mDNS)
        assert!(get_seeds_for_chain(8898).is_empty());
        assert!(get_seeds_for_chain(1337).is_empty());
        assert!(get_seeds_for_chain(31337).is_empty());
    }

    #[test]
    fn test_network_names() {
        assert_eq!(get_network_name(8898), "Mainnet");
        assert_eq!(get_network_name(9999), "Testnet");
    }
}
