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

        // Set restrictive file permissions (owner-only read/write) to protect private key
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(path, perms)
                .map_err(|e| NetworkError::Connection(format!("Failed to set key file permissions: {}", e)))?;
        }

        // 🔧 FIX F10: Restrict key file access on Windows via icacls.
        // Removes inherited permissions and grants full control only to the current user.
        #[cfg(windows)]
        {
            if let Ok(username) = std::env::var("USERNAME") {
                let _ = std::process::Command::new("icacls")
                    .args([path, "/inheritance:r", "/grant:r", &format!("{}:F", username)])
                    .output();
            }
        }

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

    /// 🔧 FIX F13: Rotate the node keypair while preserving the old key for
    /// attestation.
    ///
    /// Key rotation limits the blast radius of a compromised key. This method:
    /// 1. Generates a fresh Ed25519 keypair.
    /// 2. Saves it to `new_key_path` with restricted permissions.
    /// 3. Optionally backs up the *old* key to `old_key_backup_path` so it can
    ///    be used to sign a rotation attestation before being deleted.
    /// 4. Updates `self.keypair` in place.
    ///
    /// **Callers** should:
    ///   - Sign a `KeyRotation { old_peer_id, new_peer_id, timestamp }` message
    ///     with the *old* keypair and broadcast it so peers can update routing
    ///     tables.
    ///   - Restart swarm-level connections after rotation.
    ///   - Delete `old_key_backup_path` once the attestation has been broadcast.
    pub fn rotate_key(
        &mut self,
        new_key_path: &str,
        old_key_backup_path: Option<&str>,
    ) -> Result<RotationResult, NetworkError> {
        let old_peer_id = self.peer_id();

        // Optionally back up the current key
        if let Some(backup_path) = old_key_backup_path {
            self.save_to_file(backup_path)?;
            info!("📦 Old key backed up to {}", backup_path);
        }

        // Generate and persist new key
        let new_keypair = Keypair::generate_ed25519();
        let new_identity = NodeIdentity { keypair: new_keypair };
        new_identity.save_to_file(new_key_path)?;

        let new_peer_id = new_identity.peer_id();
        info!(
            "🔄 Key rotated: {} → {}",
            old_peer_id, new_peer_id
        );

        // Swap in the new keypair
        self.keypair = new_identity.keypair;

        Ok(RotationResult {
            old_peer_id,
            new_peer_id,
        })
    }
}

/// Result of a key rotation operation.
#[derive(Debug, Clone)]
pub struct RotationResult {
    /// The peer ID derived from the old (now-retired) keypair.
    pub old_peer_id: libp2p::PeerId,
    /// The peer ID derived from the newly generated keypair.
    pub new_peer_id: libp2p::PeerId,
}

/// Print instructions for connecting to this node
pub fn print_connection_info(peer_id: &str, listen_port: u16, external_ip: Option<&str>) {
    info!("");
    info!("╔═══════════════════════════════════════════════════════════════╗");
    info!("║                    🔗 Node Connection Info                     ║");
    info!("╠═══════════════════════════════════════════════════════════════╣");
    info!("║ Peer ID: {}...", &peer_id[..20.min(peer_id.len())]);
    info!("║ Full ID: {}", peer_id);
    info!("╠═══════════════════════════════════════════════════════════════╣");
    info!("║ To connect other nodes, add this to their config:             ║");
    info!("╠═══════════════════════════════════════════════════════════════╣");

    if let Some(ip) = external_ip {
        info!("║ bootstrap_nodes = [                                           ║");
        info!("║   \"/ip4/{}/tcp/{}/p2p/{}\"", ip, listen_port, peer_id);
        info!("║ ]                                                             ║");
    } else {
        info!("║ # Replace YOUR_IP with this server's IP address              ║");
        info!("║ bootstrap_nodes = [                                           ║");
        info!("║   \"/ip4/YOUR_IP/tcp/{}/p2p/{}\"", listen_port, peer_id);
        info!("║ ]                                                             ║");
    }

    info!("╚═══════════════════════════════════════════════════════════════╝");
    info!("");
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
