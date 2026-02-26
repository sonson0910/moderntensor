//! Utility functions extracted from `service.rs`.
//!
//! Contains standalone helper functions that don't depend on `NodeService` state:
//! - `current_timestamp`: Unix epoch timestamp
//! - `parse_address_from_hex`: hex string → `[u8; 20]`
//! - `detect_external_ip`: local network IP detection
//! - `peer_id_to_synthetic_ip`: PeerId → synthetic IPv4 for diversity tracking

use anyhow::{Context, Result};

/// Get current Unix timestamp (seconds since epoch)
/// Panics only if system time is before Unix epoch (practically impossible)
#[inline]
pub(crate) fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_secs()
}

/// Parse a hex address string (with or without 0x prefix) into [u8; 20]
pub(crate) fn parse_address_from_hex(addr_str: &str) -> Result<[u8; 20]> {
    let addr_str = addr_str.strip_prefix("0x").unwrap_or(addr_str);
    if addr_str.len() != 40 {
        return Err(anyhow::anyhow!("Invalid address length"));
    }
    let bytes =
        hex::decode(addr_str).context(format!("Failed to decode hex address: {}", addr_str))?;
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Ok(addr)
}

/// Detect external IP address using local network interfaces
/// Returns the first non-loopback IPv4 address found
pub(crate) fn detect_external_ip() -> Option<String> {
    // Try to get local IP by connecting to a public address (doesn't actually send data)
    use std::net::UdpSocket;

    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        // Connect to Google's DNS to determine local IP that would be used for external traffic
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                let ip = addr.ip().to_string();
                // Don't return loopback or link-local addresses
                if !ip.starts_with("127.") && !ip.starts_with("169.254.") {
                    return Some(ip);
                }
            }
        }
    }

    None
}

/// Convert PeerId to synthetic IP for subnet diversity tracking
/// This is a hash-based approach since libp2p PeerIds don't directly contain IPs
pub(crate) fn peer_id_to_synthetic_ip(peer_id: &str) -> std::net::IpAddr {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    peer_id.hash(&mut hasher);
    let hash = hasher.finish();

    // Create a synthetic IPv4 from the hash for subnet diversity calculation
    let bytes = hash.to_be_bytes();
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]))
}
