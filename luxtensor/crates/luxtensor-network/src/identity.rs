//! Node Identity Management
//! Handles persistent Peer ID generation and storage

use crate::error::NetworkError;
use libp2p::identity::Keypair;
use std::fs;
use std::path::Path;
use tracing::info;

/// Manages node identity (keypair) for P2P networking
pub struct NodeIdentity {
    keypair: Keypair,
}

impl NodeIdentity {
    /// Load keypair from file, or generate new one if file doesn't exist
    ///
    /// # Arguments
    /// * `key_path` - Path to the key file (e.g., "./node.key")
    ///
    /// # Returns
    /// * `NodeIdentity` with loaded or newly generated keypair
    pub fn load_or_generate(key_path: &str) -> Result<Self, NetworkError> {
        let path = Path::new(key_path);

        if path.exists() {
            // Load existing keypair
            Self::load_from_file(key_path)
        } else {
            // Generate new keypair and save
            let identity = Self::generate_new()?;
            identity.save_to_file(key_path)?;
            Ok(identity)
        }
    }

    /// Generate a new random keypair
    pub fn generate_new() -> Result<Self, NetworkError> {
        let keypair = Keypair::generate_ed25519();
        info!("🔑 Generated new node identity");
        Ok(Self { keypair })
    }

    /// Load keypair from file
    pub fn load_from_file(path: &str) -> Result<Self, NetworkError> {
        let bytes = fs::read(path)
            .map_err(|e| NetworkError::Connection(format!("Failed to read key file: {}", e)))?;

        // Try protobuf format first (libp2p standard format)
        let keypair = Keypair::from_protobuf_encoding(&bytes)
            .map_err(|e| NetworkError::Connection(format!("Invalid keypair format: {}", e)))?;

        let peer_id = keypair.public().to_peer_id();
        info!("🔑 Loaded node identity from {}", path);
        info!("   Peer ID: {}", peer_id);

        Ok(Self { keypair })
    }

    /// Save keypair to file
    pub fn save_to_file(&self, path: &str) -> Result<(), NetworkError> {
        // Use protobuf encoding (libp2p standard format, works for all key types)
        let bytes = self.keypair.to_protobuf_encoding()
            .map_err(|e| NetworkError::Connection(format!("Failed to encode keypair: {}", e)))?;

        // Create parent directory if needed
        if let Some(parent) = Path::new(path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| NetworkError::Connection(format!("Failed to create directory: {}", e)))?;
            }
        }

        fs::write(path, &bytes)
            .map_err(|e| NetworkError::Connection(format!("Failed to write key file: {}", e)))?;

        let peer_id = self.peer_id();
        info!("💾 Saved node identity to {}", path);
        info!("   Peer ID: {}", peer_id);

        Ok(())
    }

    /// Get the keypair
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }

    /// Get the keypair (owned)
    pub fn into_keypair(self) -> Keypair {
        self.keypair
    }

    /// Get the Peer ID
    pub fn peer_id(&self) -> libp2p::PeerId {
        self.keypair.public().to_peer_id()
    }

    /// Get Peer ID as string
    pub fn peer_id_string(&self) -> String {
        self.peer_id().to_string()
    }
}

/// Print instructions for connecting to this node
pub fn print_connection_info(peer_id: &str, listen_port: u16, external_ip: Option<&str>) {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    🔗 Node Connection Info                     ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Peer ID: {}...", &peer_id[..20]);
    println!("║ Full ID: {}", peer_id);
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ To connect other nodes, add this to their config:             ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");

    if let Some(ip) = external_ip {
        println!("║ bootstrap_nodes = [                                           ║");
        println!("║   \"/ip4/{}/tcp/{}/p2p/{}\"", ip, listen_port, peer_id);
        println!("║ ]                                                             ║");
    } else {
        println!("║ # Replace YOUR_IP with this server's IP address              ║");
        println!("║ bootstrap_nodes = [                                           ║");
        println!("║   \"/ip4/YOUR_IP/tcp/{}/p2p/{}\"", listen_port, peer_id);
        println!("║ ]                                                             ║");
    }

    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_new_identity() {
        let identity = NodeIdentity::generate_new().unwrap();
        let peer_id = identity.peer_id_string();
        assert!(peer_id.starts_with("12D3Koo"));
    }

    #[test]
    fn test_save_and_load_identity() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("test.key");
        let key_path_str = key_path.to_str().unwrap();

        // Generate and save
        let identity1 = NodeIdentity::generate_new().unwrap();
        let peer_id1 = identity1.peer_id_string();
        identity1.save_to_file(key_path_str).unwrap();

        // Load
        let identity2 = NodeIdentity::load_from_file(key_path_str).unwrap();
        let peer_id2 = identity2.peer_id_string();

        // Same Peer ID
        assert_eq!(peer_id1, peer_id2);
    }

    #[test]
    fn test_load_or_generate() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("auto.key");
        let key_path_str = key_path.to_str().unwrap();

        // First call: generate
        let identity1 = NodeIdentity::load_or_generate(key_path_str).unwrap();
        let peer_id1 = identity1.peer_id_string();

        // Second call: load (same peer ID)
        let identity2 = NodeIdentity::load_or_generate(key_path_str).unwrap();
        let peer_id2 = identity2.peer_id_string();

        assert_eq!(peer_id1, peer_id2);
    }
}
