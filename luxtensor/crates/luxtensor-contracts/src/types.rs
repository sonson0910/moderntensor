use serde::{Deserialize, Serialize};

/// Contract address (20 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContractAddress(pub [u8; 20]);

impl ContractAddress {
    /// Create a zero address
    pub fn zero() -> Self {
        Self([0u8; 20])
    }

    /// Check if this is a zero address
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 20]
    }

    /// Convert to bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 20]> for ContractAddress {
    fn from(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }
}

/// Contract bytecode
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractCode(pub Vec<u8>);

impl ContractCode {
    /// Create empty code
    pub fn empty() -> Self {
        Self(Vec::new())
    }

    /// Get code size
    pub fn size(&self) -> usize {
        self.0.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<u8>> for ContractCode {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

/// Contract ABI (Application Binary Interface)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractABI {
    /// Function signatures
    pub functions: Vec<FunctionSignature>,
    /// Events
    pub events: Vec<EventSignature>,
}

/// Function signature in ABI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSignature {
    /// Function name
    pub name: String,
    /// Input parameters
    pub inputs: Vec<ABIType>,
    /// Output parameters
    pub outputs: Vec<ABIType>,
}

/// Event signature in ABI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSignature {
    /// Event name
    pub name: String,
    /// Parameters
    pub inputs: Vec<ABIType>,
}

/// ABI type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ABIType {
    /// Unsigned integer (bits)
    Uint(usize),
    /// Signed integer (bits)
    Int(usize),
    /// Address
    Address,
    /// Boolean
    Bool,
    /// Fixed-size bytes
    FixedBytes(usize),
    /// Dynamic bytes
    Bytes,
    /// String
    String,
    /// Array
    Array(Box<ABIType>),
    /// Fixed-size array
    FixedArray(Box<ABIType>, usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_address() {
        let addr = ContractAddress::zero();
        assert!(addr.is_zero());
        assert_eq!(addr.as_bytes().len(), 20);
    }

    #[test]
    fn test_contract_code() {
        let code = ContractCode::empty();
        assert!(code.is_empty());
        assert_eq!(code.size(), 0);

        let code = ContractCode(vec![0x60, 0x60, 0x60, 0x40]);
        assert!(!code.is_empty());
        assert_eq!(code.size(), 4);
    }

    #[test]
    fn test_contract_address_from_bytes() {
        let bytes = [1u8; 20];
        let addr = ContractAddress::from(bytes);
        assert_eq!(addr.0, bytes);
    }
}
