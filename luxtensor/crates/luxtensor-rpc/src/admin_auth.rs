//! Admin Authentication for RPC Methods
//!
//! Provides authentication for sensitive admin methods like:
//! - admin_addPeer
//! - admin_removePeer
//! - admin_setLogLevel
//! - debug_* methods

use std::collections::HashSet;
use parking_lot::RwLock;
use sha2::{Sha256, Digest};
use tracing::warn;

/// Admin authentication configuration
#[derive(Debug, Clone)]
pub struct AdminAuthConfig {
    /// Whether admin authentication is enabled
    pub enabled: bool,
    /// Admin API key (hashed with SHA256)
    pub api_key_hash: Option<[u8; 32]>,
    /// Allowed admin IPs (if any)
    pub allowed_ips: HashSet<String>,
}

impl Default for AdminAuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key_hash: None,
            allowed_ips: HashSet::new(),
        }
    }
}

impl AdminAuthConfig {
    /// Create config with API key authentication
    pub fn with_api_key(api_key: &str) -> Self {
        let hash = Self::hash_key(api_key);
        Self {
            enabled: true,
            api_key_hash: Some(hash),
            allowed_ips: HashSet::new(),
        }
    }

    /// Create config with IP whitelist
    pub fn with_ip_whitelist(ips: Vec<String>) -> Self {
        Self {
            enabled: true,
            api_key_hash: None,
            allowed_ips: ips.into_iter().collect(),
        }
    }

    /// Hash API key with SHA256
    fn hash_key(key: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.finalize().into()
    }

    /// Verify API key
    pub fn verify_api_key(&self, provided_key: &str) -> bool {
        if let Some(expected_hash) = &self.api_key_hash {
            let provided_hash = Self::hash_key(provided_key);
            return &provided_hash == expected_hash;
        }
        false
    }
}

/// Admin authenticator for RPC
pub struct AdminAuth {
    config: RwLock<AdminAuthConfig>,
}

impl AdminAuth {
    /// Create new authenticator
    pub fn new(config: AdminAuthConfig) -> Self {
        Self {
            config: RwLock::new(config),
        }
    }

    /// Create disabled authenticator (for development)
    pub fn disabled() -> Self {
        Self::new(AdminAuthConfig::default())
    }

    /// Check if authentication is required
    pub fn is_enabled(&self) -> bool {
        self.config.read().enabled
    }

    /// Authenticate request
    ///
    /// # Arguments
    /// * `api_key` - Optional API key from request header
    /// * `client_ip` - Client IP address
    ///
    /// # Returns
    /// * `true` if authenticated
    /// * `false` if authentication failed
    pub fn authenticate(&self, api_key: Option<&str>, client_ip: &str) -> bool {
        let config = self.config.read();

        // If auth is disabled, allow all
        if !config.enabled {
            return true;
        }

        // Check IP whitelist first
        if !config.allowed_ips.is_empty() {
            if config.allowed_ips.contains(client_ip) {
                return true;
            }
            // If IP whitelist exists but IP not in it, require API key
        }

        // Check API key
        if let Some(key) = api_key {
            if config.verify_api_key(key) {
                return true;
            }
        }

        warn!("🔒 Admin auth failed for IP: {}", client_ip);
        false
    }

    /// Update configuration
    pub fn update_config(&self, config: AdminAuthConfig) {
        *self.config.write() = config;
    }

    /// Add IP to whitelist
    pub fn add_allowed_ip(&self, ip: String) {
        self.config.write().allowed_ips.insert(ip);
    }

    /// Remove IP from whitelist
    pub fn remove_allowed_ip(&self, ip: &str) {
        self.config.write().allowed_ips.remove(ip);
    }
}

impl Default for AdminAuth {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Helper to check if method requires admin auth
pub fn requires_admin_auth(method: &str) -> bool {
    matches!(method,
        "admin_addPeer" |
        "admin_removePeer" |
        "admin_peers" |
        "admin_nodeInfo" |
        "admin_setLogLevel" |
        "debug_traceTransaction" |
        "debug_traceBlock" |
        "debug_setHead" |
        "debug_gcStats" |
        "miner_start" |
        "miner_stop" |
        "miner_setGasPrice" |
        "personal_unlockAccount" |
        "personal_lockAccount"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_auth() {
        let auth = AdminAuth::disabled();
        assert!(!auth.is_enabled());
        assert!(auth.authenticate(None, "192.168.1.1"));
    }

    #[test]
    fn test_api_key_auth() {
        let auth = AdminAuth::new(AdminAuthConfig::with_api_key("secret123"));
        assert!(auth.is_enabled());

        // Wrong key
        assert!(!auth.authenticate(Some("wrong"), "192.168.1.1"));

        // Correct key
        assert!(auth.authenticate(Some("secret123"), "192.168.1.1"));
    }

    #[test]
    fn test_ip_whitelist() {
        let auth = AdminAuth::new(AdminAuthConfig::with_ip_whitelist(
            vec!["127.0.0.1".to_string(), "192.168.1.100".to_string()]
        ));

        // Whitelisted IP
        assert!(auth.authenticate(None, "127.0.0.1"));
        assert!(auth.authenticate(None, "192.168.1.100"));

        // Non-whitelisted IP
        assert!(!auth.authenticate(None, "192.168.1.1"));
    }

    #[test]
    fn test_requires_admin_auth() {
        assert!(requires_admin_auth("admin_addPeer"));
        assert!(requires_admin_auth("debug_traceTransaction"));
        assert!(!requires_admin_auth("eth_blockNumber"));
        assert!(!requires_admin_auth("eth_getBalance"));
    }
}
