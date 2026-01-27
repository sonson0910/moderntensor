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
/// Production network - update these addresses before mainnet launch
///
/// ## Deployment Instructions:
/// 1. Deploy 3+ seed nodes on geographically distributed servers
/// 2. Run each seed node and note its Peer ID from startup logs
/// 3. Update the addresses below with real IPs and Peer IDs
/// 4. Rebuild and release the binary
pub fn mainnet_seeds() -> Vec<String> {
    vec![
        // Seed Node 1 - Region: US-East (placeholder - update before mainnet)
        // "/dns4/seed-us-east.luxtensor.network/tcp/30303/p2p/12D3KooWxxxxxxxxx".to_string(),

        // Seed Node 2 - Region: EU-West (placeholder - update before mainnet)
        // "/dns4/seed-eu-west.luxtensor.network/tcp/30303/p2p/12D3KooWyyyyyyyyy".to_string(),

        // Seed Node 3 - Region: Asia-Pacific (placeholder - update before mainnet)
        // "/dns4/seed-ap.luxtensor.network/tcp/30303/p2p/12D3KooWzzzzzzzzz".to_string(),
    ]
}

/// Get seed nodes for testnet (Chain ID: 9999)
/// Public testnet - update these addresses before testnet launch
pub fn testnet_seeds() -> Vec<String> {
    vec![
        // Testnet Seed 1 (placeholder - update before testnet)
        // "/ip4/203.0.113.10/tcp/30303/p2p/12D3KooWtest1...".to_string(),

        // Testnet Seed 2 (placeholder - update before testnet)
        // "/ip4/203.0.113.20/tcp/30303/p2p/12D3KooWtest2...".to_string(),
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
