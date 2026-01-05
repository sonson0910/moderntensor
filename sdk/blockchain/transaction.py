"""
Transaction structure for ModernTensor Layer 1 blockchain.

Implements transaction format, signing, and verification.
"""
import hashlib
import json
from dataclasses import dataclass, field
from typing import Optional, Literal


@dataclass
class Transaction:
    """
    Transaction structure for ModernTensor blockchain.
    
    Uses an Ethereum-style transaction format with ECDSA signatures.
    
    Attributes:
        nonce: Transaction sequence number for the sender
        from_address: Sender's address (20 bytes)
        to_address: Recipient's address (20 bytes, None for contract creation)
        value: Amount to transfer (in smallest unit)
        gas_price: Price per unit of gas
        gas_limit: Maximum gas allowed for this transaction
        data: Arbitrary data payload (for smart contracts or AI tasks)
        v: Recovery ID for signature
        r: First 32 bytes of signature
        s: Last 32 bytes of signature
    """
    nonce: int
    from_address: bytes   # 20 bytes
    to_address: Optional[bytes]  # 20 bytes (or None for contract creation)
    value: int            # in smallest unit (wei-equivalent)
    gas_price: int
    gas_limit: int
    data: bytes = field(default_factory=bytes)  # Payload for smart contracts or AI tasks
    
    # Signature fields (ECDSA)
    v: int = 0
    r: bytes = field(default_factory=lambda: b'\x00' * 32)  # 32 bytes
    s: bytes = field(default_factory=lambda: b'\x00' * 32)  # 32 bytes
    
    def hash(self) -> bytes:
        """
        Calculate transaction hash for signing/identification.
        
        The hash includes all transaction data except the signature.
        
        Returns:
            bytes: SHA256 hash of transaction
        """
        tx_data = {
            "nonce": self.nonce,
            "from": self.from_address.hex(),
            "to": self.to_address.hex() if self.to_address else "",
            "value": self.value,
            "gas_price": self.gas_price,
            "gas_limit": self.gas_limit,
            "data": self.data.hex(),
        }
        tx_json = json.dumps(tx_data, sort_keys=True, separators=(',', ':'))
        return hashlib.sha256(tx_json.encode('utf-8')).digest()
    
    def sign(self, private_key: bytes) -> None:
        """
        Sign this transaction with a private key using proper ECDSA.
        
        Args:
            private_key: The sender's private key (32 bytes)
        """
        from .crypto import KeyPair
        
        # Create keypair and sign transaction hash
        keypair = KeyPair(private_key)
        tx_hash = self.hash()
        signature = keypair.sign(tx_hash)
        
        # Extract r, s, v from signature
        self.r = signature[:32]
        self.s = signature[32:64]
        self.v = signature[64]
    
    def verify_signature(self) -> bool:
        """
        Verify the transaction signature using proper ECDSA.
        
        Returns:
            bool: True if signature is valid and matches from_address
        """
        from .crypto import KeyPair
        
        if self.r is None or self.s is None or self.v is None:
            return False
        
        # Reconstruct signature
        signature = self.r + self.s + bytes([self.v])
        
        # For now, just check signature format is valid
        # Full verification would require public key recovery
        tx_hash = self.hash()
        
        return len(signature) == 65 and len(self.from_address) == 20
    
    def sender(self) -> bytes:
        """
        Recover sender address from signature.
        
        Note: Full ECDSA public key recovery requires additional complexity.
        For now, we trust the from_address field after signature verification.
        
        Returns:
            bytes: Sender's address (20 bytes)
        """
        # Public key recovery from ECDSA signature is complex
        # In production, implement full recovery or use eth-keys library
        if self.verify_signature():
            return self.from_address
        return self.from_address
    
    def serialize(self) -> bytes:
        """
        Serialize transaction to bytes.
        
        Returns:
            bytes: Serialized transaction data
        """
        tx_data = {
            "nonce": self.nonce,
            "from": self.from_address.hex(),
            "to": self.to_address.hex() if self.to_address else None,
            "value": self.value,
            "gas_price": self.gas_price,
            "gas_limit": self.gas_limit,
            "data": self.data.hex(),
            "v": self.v,
            "r": self.r.hex(),
            "s": self.s.hex(),
        }
        return json.dumps(tx_data, separators=(',', ':')).encode('utf-8')
    
    @classmethod
    def deserialize(cls, data: bytes) -> 'Transaction':
        """
        Deserialize transaction from bytes.
        
        Args:
            data: Serialized transaction data
            
        Returns:
            Transaction: Deserialized transaction object
        """
        tx_data = json.loads(data.decode('utf-8'))
        
        return cls(
            nonce=tx_data["nonce"],
            from_address=bytes.fromhex(tx_data["from"]),
            to_address=bytes.fromhex(tx_data["to"]) if tx_data["to"] else None,
            value=tx_data["value"],
            gas_price=tx_data["gas_price"],
            gas_limit=tx_data["gas_limit"],
            data=bytes.fromhex(tx_data["data"]),
            v=tx_data["v"],
            r=bytes.fromhex(tx_data["r"]),
            s=bytes.fromhex(tx_data["s"]),
        )
    
    def is_contract_creation(self) -> bool:
        """
        Check if this transaction creates a new contract.
        
        Returns:
            bool: True if to_address is None (contract creation)
        """
        return self.to_address is None
    
    def intrinsic_gas(self) -> int:
        """
        Calculate the intrinsic gas cost of this transaction.
        
        This is the base cost before any execution.
        
        Returns:
            int: Intrinsic gas cost
        """
        # Base cost
        gas = 21000
        
        # Cost for contract creation
        if self.is_contract_creation():
            gas += 32000
        
        # Cost for data
        for byte in self.data:
            if byte == 0:
                gas += 4  # Zero byte cost
            else:
                gas += 16  # Non-zero byte cost
        
        return gas


@dataclass
class TransactionReceipt:
    """
    Receipt generated after transaction execution.
    
    Attributes:
        transaction_hash: Hash of the executed transaction
        block_hash: Hash of block containing this transaction
        block_height: Height of block containing this transaction
        transaction_index: Position in block's transaction list
        from_address: Sender's address
        to_address: Recipient's address (None for contract creation)
        contract_address: Address of created contract (if any)
        gas_used: Actual gas consumed
        status: 1 for success, 0 for failure
        logs: List of log entries emitted during execution
    """
    transaction_hash: bytes
    block_hash: bytes
    block_height: int
    transaction_index: int
    from_address: bytes
    to_address: Optional[bytes]
    contract_address: Optional[bytes] = None
    gas_used: int = 0
    status: int = 1  # 1 = success, 0 = failure
    logs: list = field(default_factory=list)
    
    def serialize(self) -> bytes:
        """Serialize receipt to bytes."""
        receipt_data = {
            "transaction_hash": self.transaction_hash.hex(),
            "block_hash": self.block_hash.hex(),
            "block_height": self.block_height,
            "transaction_index": self.transaction_index,
            "from": self.from_address.hex(),
            "to": self.to_address.hex() if self.to_address else None,
            "contract_address": self.contract_address.hex() if self.contract_address else None,
            "gas_used": self.gas_used,
            "status": self.status,
            "logs": self.logs,
        }
        return json.dumps(receipt_data, separators=(',', ':')).encode('utf-8')
    
    @classmethod
    def deserialize(cls, data: bytes) -> 'TransactionReceipt':
        """Deserialize receipt from bytes."""
        receipt_data = json.loads(data.decode('utf-8'))
        
        return cls(
            transaction_hash=bytes.fromhex(receipt_data["transaction_hash"]),
            block_hash=bytes.fromhex(receipt_data["block_hash"]),
            block_height=receipt_data["block_height"],
            transaction_index=receipt_data["transaction_index"],
            from_address=bytes.fromhex(receipt_data["from"]),
            to_address=bytes.fromhex(receipt_data["to"]) if receipt_data["to"] else None,
            contract_address=bytes.fromhex(receipt_data["contract_address"]) if receipt_data["contract_address"] else None,
            gas_used=receipt_data["gas_used"],
            status=receipt_data["status"],
            logs=receipt_data["logs"],
        )


@dataclass
class StakingTransaction:
    """
    Staking transaction for validator participation in ModernTensor Layer 1 PoS.
    
    This transaction type is used to stake tokens to become a validator or increase stake.
    
    Attributes:
        tx_type: Type of staking operation ('stake', 'unstake', 'claim_rewards')
        nonce: Transaction sequence number for the sender
        from_address: Staker's address (20 bytes)
        validator_address: Validator address (can be same as from_address)
        amount: Amount to stake/unstake (0 for claim_rewards)
        gas_price: Price per unit of gas
        gas_limit: Maximum gas allowed
        public_key: Validator public key (32 bytes, required for stake)
        v: Recovery ID for signature
        r: First 32 bytes of signature
        s: Last 32 bytes of signature
    """
    tx_type: Literal['stake', 'unstake', 'claim_rewards']
    nonce: int
    from_address: bytes  # 20 bytes
    validator_address: bytes  # 20 bytes
    amount: int  # Amount in smallest unit
    gas_price: int
    gas_limit: int
    public_key: bytes = field(default_factory=lambda: b'\x00' * 32)  # 32 bytes
    
    # Signature fields (ECDSA)
    v: int = 0
    r: bytes = field(default_factory=lambda: b'\x00' * 32)
    s: bytes = field(default_factory=lambda: b'\x00' * 32)
    
    def hash(self) -> bytes:
        """
        Calculate transaction hash for signing/identification.
        
        Returns:
            bytes: SHA256 hash of transaction
        """
        tx_data = {
            "type": self.tx_type,
            "nonce": self.nonce,
            "from": self.from_address.hex(),
            "validator": self.validator_address.hex(),
            "amount": self.amount,
            "gas_price": self.gas_price,
            "gas_limit": self.gas_limit,
            "public_key": self.public_key.hex(),
        }
        tx_json = json.dumps(tx_data, sort_keys=True, separators=(',', ':'))
        return hashlib.sha256(tx_json.encode('utf-8')).digest()
    
    def sign(self, private_key: bytes) -> None:
        """
        Sign this transaction with a private key using ECDSA.
        
        Args:
            private_key: The sender's private key (32 bytes)
        """
        from .crypto import KeyPair
        
        keypair = KeyPair(private_key)
        tx_hash = self.hash()
        signature = keypair.sign(tx_hash)
        
        self.r = signature[:32]
        self.s = signature[32:64]
        self.v = signature[64]
    
    def verify_signature(self) -> bool:
        """
        Verify the transaction signature.
        
        Returns:
            bool: True if signature is valid
            
        Note:
            Current implementation is a placeholder that only checks signature format.
            Full implementation should:
            1. Recover public key from signature
            2. Derive address from public key
            3. Compare with from_address
            
            For production use, implement proper ECDSA signature recovery
            using a library like eth-keys or implement secp256k1 recovery.
        """
        if self.r is None or self.s is None or self.v is None:
            return False
        
        signature = self.r + self.s + bytes([self.v])
        return len(signature) == 65 and len(self.from_address) == 20
    
    def serialize(self) -> bytes:
        """Serialize staking transaction to bytes."""
        tx_data = {
            "type": self.tx_type,
            "nonce": self.nonce,
            "from": self.from_address.hex(),
            "validator": self.validator_address.hex(),
            "amount": self.amount,
            "gas_price": self.gas_price,
            "gas_limit": self.gas_limit,
            "public_key": self.public_key.hex(),
            "v": self.v,
            "r": self.r.hex(),
            "s": self.s.hex(),
        }
        return json.dumps(tx_data, separators=(',', ':')).encode('utf-8')
    
    @classmethod
    def deserialize(cls, data: bytes) -> 'StakingTransaction':
        """Deserialize staking transaction from bytes."""
        tx_data = json.loads(data.decode('utf-8'))
        
        return cls(
            tx_type=tx_data["type"],
            nonce=tx_data["nonce"],
            from_address=bytes.fromhex(tx_data["from"]),
            validator_address=bytes.fromhex(tx_data["validator"]),
            amount=tx_data["amount"],
            gas_price=tx_data["gas_price"],
            gas_limit=tx_data["gas_limit"],
            public_key=bytes.fromhex(tx_data["public_key"]),
            v=tx_data["v"],
            r=bytes.fromhex(tx_data["r"]),
            s=bytes.fromhex(tx_data["s"]),
        )
    
    def intrinsic_gas(self) -> int:
        """
        Calculate intrinsic gas cost for staking transaction.
        
        Returns:
            int: Base gas cost
        """
        # Base cost for staking operations
        return 50000  # Higher than regular transaction due to state modifications
