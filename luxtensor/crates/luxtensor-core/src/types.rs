use serde::{Deserialize, Serialize};

/// 32-byte hash type
pub type Hash = [u8; 32];

/// 20-byte address type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Address([u8; 20]);

impl Address {
    pub fn new(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    /// Create address from byte slice (panics if too short — use try_from_slice for untrusted input)
    pub fn from_slice(slice: &[u8]) -> Self {
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&slice[..20]);
        Self(bytes)
    }

    /// Safe address creation from untrusted byte slice
    pub fn try_from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() < 20 {
            return None;
        }
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&slice[..20]);
        Some(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn zero() -> Self {
        Self([0u8; 20])
    }
}

impl From<[u8; 20]> for Address {
    fn from(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}
