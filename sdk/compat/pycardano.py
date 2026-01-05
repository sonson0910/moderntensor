"""
Cardano to Layer 1 Compatibility Layer

This module provides backward-compatible imports and wrappers
to help transition from pycardano/blockfrost to Layer 1 blockchain.

Import this module instead of pycardano to use Layer 1 equivalents.
"""

from sdk.blockchain import (
    L1HDWallet as HDWallet,
    L1Address,
    L1Network,
    L1ChainContext,
    L1UTxO,
    KeyPair,
    Transaction,
)

# Network type compatibility
class Network:
    """Network type for backward compatibility with pycardano.Network"""
    MAINNET = L1Network.MAINNET
    TESTNET = L1Network.TESTNET
    DEVNET = L1Network.DEVNET

# Address compatibility
Address = L1Address

# Chain context compatibility  
BlockFrostChainContext = L1ChainContext

# UTxO compatibility
UTxO = L1UTxO

# Extended signing key compatibility
class ExtendedSigningKey:
    """
    Compatibility wrapper for pycardano ExtendedSigningKey.
    Uses Layer 1 KeyPair internally.
    """
    def __init__(self, keypair: KeyPair):
        self.keypair = keypair
        self._private_key = keypair.private_key
        self._public_key = keypair.public_key
    
    @classmethod
    def from_hdwallet(cls, wallet_or_keypair):
        """Create from HD wallet or keypair."""
        if isinstance(wallet_or_keypair, KeyPair):
            return cls(wallet_or_keypair)
        # If it's a derived key, just use it
        return cls(wallet_or_keypair)
    
    def to_verification_key(self):
        """Get verification key."""
        return ExtendedVerificationKey(self.keypair)
    
    def sign(self, message: bytes) -> bytes:
        """Sign a message."""
        return self.keypair.sign(message)
    
    def to_cbor(self) -> bytes:
        """Convert to CBOR (returns private key bytes for compatibility)."""
        return self._private_key
    
    @property
    def key(self):
        """Get underlying key."""
        return self._private_key


class ExtendedVerificationKey:
    """
    Compatibility wrapper for pycardano ExtendedVerificationKey.
    Uses Layer 1 KeyPair internally.
    """
    def __init__(self, keypair: KeyPair):
        self.keypair = keypair
        self._public_key = keypair.public_key
    
    def hash(self) -> bytes:
        """Get hash of public key (first 20 bytes for address derivation)."""
        import hashlib
        return hashlib.sha256(self._public_key).digest()[:20]
    
    def verify(self, message: bytes, signature: bytes) -> bool:
        """Verify a signature."""
        return KeyPair.verify(message, signature, self._public_key)
    
    @property
    def key(self):
        """Get underlying key."""
        return self._public_key


# Plutus data structures (stubs for compatibility)
class PlutusData:
    """Stub for PlutusData compatibility."""
    pass


class Redeemer:
    """Stub for Redeemer compatibility."""
    pass


class RawPlutusData:
    """Stub for RawPlutusData compatibility."""
    pass


class IndefiniteList:
    """Stub for IndefiniteList compatibility."""
    pass


class ScriptHash:
    """Stub for ScriptHash compatibility."""
    pass


class AssetName:
    """Stub for AssetName compatibility."""
    pass


class PaymentVerificationKey:
    """Stub for PaymentVerificationKey compatibility."""
    def __init__(self, keypair: KeyPair):
        self.keypair = keypair


class StakeVerificationKey:
    """Stub for StakeVerificationKey compatibility."""
    def __init__(self, keypair: KeyPair):
        self.keypair = keypair


class TransactionId:
    """Transaction ID wrapper."""
    def __init__(self, tx_hash: str):
        self.tx_hash = tx_hash
    
    def __str__(self):
        return self.tx_hash


class TransactionOutput:
    """Transaction output stub."""
    pass


class Value:
    """Value stub for amount representation."""
    pass


class TransactionBuilder:
    """Transaction builder stub."""
    pass


class CBORSerializable:
    """CBOR serializable base class stub."""
    pass


# Export all compatibility types
__all__ = [
    'HDWallet',
    'Network',
    'Address',
    'L1Address',
    'BlockFrostChainContext',
    'L1ChainContext',
    'UTxO',
    'L1UTxO',
    'ExtendedSigningKey',
    'ExtendedVerificationKey',
    'PlutusData',
    'Redeemer',
    'RawPlutusData',
    'IndefiniteList',
    'ScriptHash',
    'AssetName',
    'PaymentVerificationKey',
    'StakeVerificationKey',
    'TransactionId',
    'TransactionOutput',
    'Value',
    'TransactionBuilder',
    'CBORSerializable',
    'KeyPair',
    'Transaction',
]
