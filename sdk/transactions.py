"""
Luxtensor Transaction Module

Provides functionality for creating and signing transactions for Luxtensor blockchain.
This module implements the Luxtensor transaction format compatible with the Rust implementation.
"""

from typing import Dict, Any, Optional, Tuple
from dataclasses import dataclass
import struct
from eth_account import Account
from Crypto.Hash import keccak
import binascii


@dataclass
class LuxtensorTransaction:
    """
    Luxtensor transaction structure matching the Rust implementation.
    
    Corresponds to the Transaction struct in luxtensor-core/src/transaction.rs
    """
    nonce: int
    from_address: str  # Ethereum-style address (0x...)
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
        - nonce (8 bytes, little endian)
        - from address (20 bytes)
        - to address (20 bytes if present)
        - value (16 bytes, little endian, u128)
        - gas_price (8 bytes, little endian)
        - gas_limit (8 bytes, little endian)
        - data
        """
        msg = bytearray()
        
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
        
        # Keccak256 hash
        k = keccak.new(digest_bits=256)
        k.update(msg)
        return k.digest()
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert transaction to dictionary for RPC submission."""
        return {
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
    data: bytes = b''
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
    
    Returns:
        Signed transaction dictionary ready for submission via RPC
    
    Example:
        >>> tx = create_transfer_transaction(
        ...     from_address="0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
        ...     to_address="0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3",
        ...     amount=1000000000,  # 1 MDT
        ...     nonce=0,
        ...     private_key="0x..."
        ... )
        >>> client.submit_transaction(tx)
    """
    # Create unsigned transaction
    tx = LuxtensorTransaction(
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
    Sign a Luxtensor transaction using secp256k1.
    
    Args:
        tx: Unsigned transaction
        private_key: Private key (hex string with or without 0x)
    
    Returns:
        Signed transaction with v, r, s set
    """
    # Remove 0x prefix if present
    if private_key.startswith('0x'):
        private_key = private_key[2:]
    
    # Create account from private key
    account = Account.from_key(private_key)
    
    # Get signing message and hash it
    message = tx.get_signing_message()
    k = keccak.new(digest_bits=256)
    k.update(message)
    message_hash = k.digest()
    
    # Sign the message hash
    signed_message = account.signHash(message_hash)
    
    # Extract signature components
    # eth_account uses (v, r, s) format where v is recovery id
    tx.v = signed_message.v
    tx.r = signed_message.r.to_bytes(32, byteorder='big')
    tx.s = signed_message.s.to_bytes(32, byteorder='big')
    
    return tx


def verify_transaction_signature(tx: LuxtensorTransaction) -> bool:
    """
    Verify that a transaction's signature is valid.
    
    Args:
        tx: Signed transaction
    
    Returns:
        True if signature is valid, False otherwise
    """
    from eth_account.messages import encode_defunct
    from eth_utils import to_checksum_address
    
    # Get signing message and hash
    message = tx.get_signing_message()
    k = keccak.new(digest_bits=256)
    k.update(message)
    message_hash = k.digest()
    
    # Recover address from signature
    try:
        # Combine r and s
        r_int = int.from_bytes(tx.r, byteorder='big')
        s_int = int.from_bytes(tx.s, byteorder='big')
        
        # Create signature
        signature = (tx.v, r_int, s_int)
        
        # Recover signer address
        from eth_account._utils.signing import sign_message_hash, recover_message_signer_from_vrs
        recovered_address = Account.recover_message(
            encode_defunct(primitive=message_hash),
            vrs=signature
        )
        
        # Compare with from_address
        return to_checksum_address(recovered_address) == to_checksum_address(tx.from_address)
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
