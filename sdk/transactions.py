"""
Luxtensor Transaction Module

Provides functionality for creating and signing transactions for Luxtensor blockchain.
This module implements the Luxtensor transaction format compatible with the Rust implementation.
Uses native Python cryptography matching Luxtensor's Rust crypto implementation.
"""

from typing import Dict, Any, Optional
from dataclasses import dataclass
import struct
import hashlib
from Crypto.Hash import keccak
from ecdsa import SigningKey, SECP256k1, util


def keccak256(data: bytes) -> bytes:
    """
    Keccak256 hash function (matching Luxtensor Rust implementation).

    Args:
        data: Input data to hash

    Returns:
        32-byte hash
    """
    k = keccak.new(digest_bits=256)
    k.update(data)
    return k.digest()


def derive_address_from_private_key(private_key: str) -> str:
    """
    Derive ModernTensor-style address from private key (matching Luxtensor Rust).

    Args:
        private_key: Private key in hex (with or without 0x prefix)

    Returns:
        Address string with 0x prefix
    """
    # Remove 0x prefix if present
    if private_key.startswith('0x'):
        private_key = private_key[2:]

    # Convert to bytes
    private_key_bytes = bytes.fromhex(private_key)

    # Create signing key
    signing_key = SigningKey.from_string(private_key_bytes, curve=SECP256k1)
    verifying_key = signing_key.get_verifying_key()

    # Get uncompressed public key (65 bytes: 0x04 + 32 bytes X + 32 bytes Y)
    public_key_bytes = b'\x04' + verifying_key.to_string()

    # Hash public key with keccak256 (skip first byte 0x04)
    hash_result = keccak256(public_key_bytes[1:])

    # Take last 20 bytes as address
    address_bytes = hash_result[12:]

    # Convert to hex with 0x prefix
    return '0x' + address_bytes.hex()


@dataclass
class LuxtensorTransaction:
    """
    Luxtensor transaction structure matching the Rust implementation.

    Corresponds to the Transaction struct in luxtensor-core/src/transaction.rs
    """
    chain_id: int  # Chain ID for replay protection (default: 1)
    nonce: int
    from_address: str  # ModernTensor address (0x...)
    to_address: Optional[str]  # None for contract creation
    value: int  # Amount in base units
    gas_price: int
    gas_limit: int
    data: bytes

    # Signature components (set after signing)
    v: int = 0
    r: bytes = b'\x00' * 32
    s: bytes = b'\x00' * 32

    def get_signing_message(self) -> bytes:
        """
        Get the signing message for this transaction.

        This matches the signing_message() method in Luxtensor Rust code:
        - chain_id (8 bytes, little endian) - FIRST for replay protection
        - nonce (8 bytes, little endian)
        - from address (20 bytes)
        - to address (20 bytes if present)
        - value (16 bytes, little endian, u128)
        - gas_price (8 bytes, little endian)
        - gas_limit (8 bytes, little endian)
        - data
        """
        msg = bytearray()

        # Chain ID (u64 little endian) - FIRST for cross-chain replay protection
        msg.extend(struct.pack('<Q', self.chain_id))

        # Nonce (u64 little endian)
        msg.extend(struct.pack('<Q', self.nonce))

        # From address (20 bytes)
        from_bytes = bytes.fromhex(self.from_address[2:] if self.from_address.startswith('0x') else self.from_address)
        msg.extend(from_bytes)

        # To address (20 bytes if present)
        if self.to_address:
            to_bytes = bytes.fromhex(self.to_address[2:] if self.to_address.startswith('0x') else self.to_address)
            msg.extend(to_bytes)

        # Value (u128 little endian)
        msg.extend(struct.pack('<QQ', self.value & 0xFFFFFFFFFFFFFFFF, self.value >> 64))

        # Gas price (u64 little endian)
        msg.extend(struct.pack('<Q', self.gas_price))

        # Gas limit (u64 little endian)
        msg.extend(struct.pack('<Q', self.gas_limit))

        # Data
        msg.extend(self.data)

        return bytes(msg)

    def hash(self) -> bytes:
        """
        Calculate transaction hash using keccak256.
        """
        # Serialize the transaction (matching Rust bincode serialization)
        msg = self.get_signing_message()

        # Add signature components if signed
        if self.v != 0:
            msg = msg + bytes([self.v]) + self.r + self.s

        # Keccak256 hash (using helper function)
        return keccak256(msg)

    def to_dict(self) -> Dict[str, Any]:
        """Convert transaction to dictionary for RPC submission."""
        return {
            'chainId': self.chain_id,
            'nonce': self.nonce,
            'from': self.from_address,
            'to': self.to_address,
            'value': self.value,
            'gasPrice': self.gas_price,
            'gasLimit': self.gas_limit,
            'data': '0x' + self.data.hex() if self.data else '0x',
            'v': self.v,
            'r': '0x' + self.r.hex(),
            's': '0x' + self.s.hex(),
        }


def create_transfer_transaction(
    from_address: str,
    to_address: str,
    amount: int,
    nonce: int,
    private_key: str,
    gas_price: int = 50,
    gas_limit: int = 21000,
    data: bytes = b'',
    chain_id: int = 1
) -> Dict[str, Any]:
    """
    Create and sign a transfer transaction for Luxtensor blockchain.

    Args:
        from_address: Sender's address (0x...)
        to_address: Recipient's address (0x...)
        amount: Amount to send in base units
        nonce: Transaction nonce
        private_key: Private key for signing (hex string with or without 0x)
        gas_price: Gas price (default: 50)
        gas_limit: Gas limit (default: 21000)
        data: Additional transaction data (default: empty)
        chain_id: Chain ID for replay protection (default: 1)

    Returns:
        Signed transaction dictionary ready for submission via RPC

    Example:
        >>> tx = create_transfer_transaction(
        ...     from_address="0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
        ...     to_address="0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3",
        ...     amount=1000000000,  # 1 MDT
        ...     nonce=0,
        ...     private_key="0x...",
        ...     chain_id=1  # mainnet
        ... )
        >>> client.submit_transaction(tx)
    """
    # Create unsigned transaction
    tx = LuxtensorTransaction(
        chain_id=chain_id,
        nonce=nonce,
        from_address=from_address,
        to_address=to_address,
        value=amount,
        gas_price=gas_price,
        gas_limit=gas_limit,
        data=data
    )

    # Sign the transaction
    signed_tx = sign_transaction(tx, private_key)

    return signed_tx.to_dict()


def sign_transaction(tx: LuxtensorTransaction, private_key: str) -> LuxtensorTransaction:
    """
    Sign a Luxtensor transaction using secp256k1 (matching Rust implementation).

    Args:
        tx: Unsigned transaction
        private_key: Private key (hex string with or without 0x)

    Returns:
        Signed transaction with v, r, s set
    """
    # Remove 0x prefix if present
    if private_key.startswith('0x'):
        private_key = private_key[2:]

    # Convert private key to bytes
    private_key_bytes = bytes.fromhex(private_key)

    # Create signing key using secp256k1 (same as Luxtensor Rust)
    signing_key = SigningKey.from_string(private_key_bytes, curve=SECP256k1)

    # Get signing message and hash it with keccak256 (same as Luxtensor Rust)
    message = tx.get_signing_message()
    message_hash = keccak256(message)

    # Sign the message hash using deterministic k (RFC 6979)
    signature = signing_key.sign_digest_deterministic(
        message_hash,
        hashfunc=hashlib.sha256,
        sigencode=util.sigencode_string
    )

    # Extract r and s from signature (64 bytes total: 32 for r, 32 for s)
    r = signature[:32]
    s = signature[32:64]

    # Calculate recovery id (v)
    # Try both recovery ids to see which one recovers the correct public key
    v = _calculate_recovery_id(message_hash, r, s, signing_key.get_verifying_key())

    tx.v = v
    tx.r = r
    tx.s = s

    return tx


def _calculate_recovery_id(message_hash: bytes, r: bytes, s: bytes, verifying_key) -> int:
    """
    Calculate the recovery id (v) for ECDSA signature recovery.

    This allows recovering the public key from the signature.
    """

    # Get the original public key bytes
    original_pubkey = verifying_key.to_string()

    # Try both possible recovery ids (0 and 1)
    for recovery_id in [0, 1]:
        try:
            # Attempt to recover public key with this recovery id
            recovered_pubkey = _recover_public_key(message_hash, r + s, recovery_id)
            if recovered_pubkey and recovered_pubkey == original_pubkey:
                return recovery_id + 27  # Add 27 for Ethereum-style v
        except Exception:
            continue

    # Default to 27 if recovery fails
    return 27


def _recover_public_key(message_hash: bytes, signature: bytes, recovery_id: int) -> Optional[bytes]:
    """
    Recover public key from ECDSA signature using coincurve (libsecp256k1 binding).

    Args:
        message_hash: 32-byte hash of the signed message
        signature: 64-byte signature (r + s)
        recovery_id: Recovery ID (0 or 1)

    Returns:
        64-byte uncompressed public key (without 0x04 prefix), or None on failure
    """
    try:
        from coincurve import PublicKey as CoincurvePublicKey
        # coincurve expects: recovery_id (1 byte) + r (32 bytes) + s (32 bytes)
        recoverable_sig = bytes([recovery_id]) + signature
        public_key = CoincurvePublicKey.from_signature_and_message(
            recoverable_sig, message_hash, hasher=None
        )
        # Return uncompressed key without the 0x04 prefix (64 bytes)
        return public_key.format(compressed=False)[1:]
    except Exception:
        return None


def verify_transaction_signature(tx: LuxtensorTransaction) -> bool:
    """
    Verify that a transaction's signature is valid (matching Luxtensor Rust implementation).

    Recovers the public key from the (v, r, s) signature components and verifies
    that the derived address matches tx.from_address.

    Args:
        tx: Signed transaction

    Returns:
        True if signature is valid, False otherwise
    """
    try:
        # Basic format checks
        if len(tx.r) != 32 or len(tx.s) != 32:
            return False

        # Get signing message and compute hash
        message = tx.get_signing_message()
        message_hash = keccak256(message)

        # Combine r and s into signature
        signature = tx.r + tx.s

        # Calculate recovery id from v (v = recovery_id + 27)
        recovery_id = tx.v - 27
        if recovery_id not in (0, 1):
            return False

        # Recover public key from signature
        recovered_pubkey = _recover_public_key(message_hash, signature, recovery_id)
        if recovered_pubkey is None:
            return False

        # Derive address from recovered public key using keccak256
        pubkey_hash = keccak256(recovered_pubkey)
        recovered_address = '0x' + pubkey_hash[12:].hex()

        # Compare with claimed from_address (case-insensitive)
        claimed_address = tx.from_address.lower()
        if not claimed_address.startswith('0x'):
            claimed_address = '0x' + claimed_address

        return recovered_address.lower() == claimed_address.lower()

    except Exception:
        return False


def encode_transaction_for_rpc(tx: LuxtensorTransaction) -> str:
    """
    Encode signed transaction as hex string for RPC submission.

    Args:
        tx: Signed transaction

    Returns:
        Hex-encoded transaction string (with 0x prefix)
    """
    # Serialize transaction matching Rust bincode format
    msg = tx.get_signing_message()

    # Add signature
    signature_bytes = bytes([tx.v]) + tx.r + tx.s

    # Combine and encode
    full_tx = msg + signature_bytes
    return '0x' + full_tx.hex()


# Helper functions for gas estimation
def estimate_gas_for_transfer() -> int:
    """Estimate gas for simple transfer."""
    return 21000


def estimate_gas_for_contract_call(data_size: int) -> int:
    """
    Estimate gas for contract call based on data size.

    Args:
        data_size: Size of transaction data in bytes

    Returns:
        Estimated gas limit
    """
    base_gas = 21000
    data_gas = data_size * 68  # 68 gas per byte (non-zero)
    return base_gas + data_gas


def calculate_transaction_fee(gas_used: int, gas_price: int) -> int:
    """
    Calculate transaction fee.

    Args:
        gas_used: Amount of gas used
        gas_price: Gas price per unit

    Returns:
        Total fee in base units
    """
    return gas_used * gas_price
