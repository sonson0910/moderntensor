//! Hardcoded Seed Nodes for Luxtensor Network
//!
//! These are the official seed nodes that every node will try to connect to
//! when starting up. This allows nodes to join the network without any configuration.
//!
//! ## Network Types
//! - **Devnet (Chain ID: 1)**: Local development, uses mDNS, no seed nodes needed
//! - **Testnet (Chain ID: 9999)**: Public testnet, requires seed nodes
//! - **Mainnet (Chain ID: 8899)**: Production network, requires seed nodes
//!
//! ## Adding New Seed Nodes
//! 1. Run the seed node and get its Peer ID from startup log
//! 2. Add the multiaddr to the appropriate network section below
//! 3. Rebuild and release the binary

use tracing::info;

/// Get seed nodes for mainnet (Chain ID: 8899)
/// Production network - add official seed nodes before mainnet launch
pub fn mainnet_seeds() -> Vec<String> {
    vec![
        // TODO: Add mainnet seed nodes before mainnet launch
        // Format: "/ip4/IP/tcp/30303/p2p/PEER_ID" or "/dns4/HOSTNAME/tcp/30303/p2p/PEER_ID"
        // Example:
        // "/dns4/seed-1.luxtensor.network/tcp/30303/p2p/12D3KooW...".to_string(),
    ]
}

/// Get seed nodes for testnet (Chain ID: 9999)
/// Public testnet - add seed nodes when launching testnet
pub fn testnet_seeds() -> Vec<String> {
    vec![
        // TODO: Add testnet seed nodes when launching testnet
        // Example:
        // "/ip4/203.0.113.10/tcp/30303/p2p/12D3KooWHxUxbJpYmFt...".to_string(),
    ]
}

/// Get seed nodes for devnet/local development (Chain ID: 1, 31337, etc.)
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
        8899 => mainnet_seeds(),
        9999 => testnet_seeds(),
        _ => devnet_seeds(), // Chain ID 1, 31337, etc. = devnet
    };

    if !seeds.is_empty() {
        info!("🌐 Found {} hardcoded seed(s) for chain {}", seeds.len(), chain_id);
    }

    seeds
}

/// Check if the given chain has hardcoded seeds
pub fn has_hardcoded_seeds(chain_id: u64) -> bool {
    !get_seeds_for_chain(chain_id).is_empty()
}

/// Get network name from chain ID
pub fn get_network_name(chain_id: u64) -> &'static str {
    match chain_id {
        8899 => "Mainnet",
        9999 => "Testnet",
        1 => "Devnet",
        31337 => "Local",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_devnet_uses_mdns() {
        // Devnet should have no seeds (uses mDNS)
        assert!(get_seeds_for_chain(1).is_empty());
        assert!(get_seeds_for_chain(31337).is_empty());
    }

    #[test]
    fn test_network_names() {
        assert_eq!(get_network_name(8899), "Mainnet");
        assert_eq!(get_network_name(9999), "Testnet");
        assert_eq!(get_network_name(1), "Devnet");
    }
}
