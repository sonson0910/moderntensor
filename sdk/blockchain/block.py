"""
Block structure for ModernTensor Layer 1 blockchain.

Implements the block data structure with header, body, and validation methods.
"""
import hashlib
import time
from dataclasses import dataclass, field
from typing import List, Optional
import json


@dataclass
class BlockHeader:
    """
    Block header containing metadata and hash links.
    
    Attributes:
        version: Protocol version number
        height: Block height (position in chain)
        timestamp: Unix timestamp when block was created
        previous_hash: Hash of the previous block (32 bytes)
        state_root: Merkle root of the state trie (32 bytes)
        txs_root: Merkle root of transactions (32 bytes)
        receipts_root: Merkle root of transaction receipts (32 bytes)
        validator: Public key of block producer (32 bytes)
        signature: Block signature by validator (64 bytes)
        gas_used: Total gas consumed by transactions
        gas_limit: Maximum gas allowed in block
        extra_data: Additional arbitrary data
    """
    version: int
    height: int
    timestamp: int
    previous_hash: bytes  # 32 bytes
    state_root: bytes     # 32 bytes - Merkle root
    txs_root: bytes       # 32 bytes - Merkle root
    receipts_root: bytes  # 32 bytes
    
    # Consensus fields
    validator: bytes      # 32 bytes - validator public key
    signature: bytes      # 64 bytes - block signature
    
    # Execution metadata
    gas_used: int
    gas_limit: int
    extra_data: bytes = field(default_factory=bytes)
    
    def hash(self) -> bytes:
        """
        Calculate the hash of this block header.
        
        Returns:
            bytes: SHA256 hash of the header
        """
        # Serialize header fields (excluding signature for hash calculation)
        header_data = {
            "version": self.version,
            "height": self.height,
            "timestamp": self.timestamp,
            "previous_hash": self.previous_hash.hex(),
            "state_root": self.state_root.hex(),
            "txs_root": self.txs_root.hex(),
            "receipts_root": self.receipts_root.hex(),
            "validator": self.validator.hex(),
            "gas_used": self.gas_used,
            "gas_limit": self.gas_limit,
            "extra_data": self.extra_data.hex(),
        }
        header_json = json.dumps(header_data, sort_keys=True, separators=(',', ':'))
        return hashlib.sha256(header_json.encode('utf-8')).digest()
    
    def verify_signature(self) -> bool:
        """
        Verify the block signature using the validator's public key.
        
        Returns:
            bool: True if signature is valid
        """
        from .crypto import KeyPair
        
        # Note: This requires validator's public key to be available
        # For now, we just check signature format
        # In production, should verify against actual validator public key
        return self.signature is not None and len(self.signature) == 65


@dataclass
class Block:
    """
    Complete block structure including header and body.
    
    Attributes:
        header: Block header with metadata
        transactions: List of transactions in this block
    """
    header: BlockHeader
    transactions: List['Transaction'] = field(default_factory=list)
    
    def hash(self) -> bytes:
        """
        Get the hash of this block (from header).
        
        Returns:
            bytes: Block hash
        """
        return self.header.hash()
    
    def validate_structure(self) -> bool:
        """
        Validate the block structure (not including consensus rules).
        
        Returns:
            bool: True if block structure is valid
        """
        # Check basic constraints
        if self.header.height < 0:
            return False
        if self.header.timestamp < 0:
            return False
        if len(self.header.previous_hash) != 32:
            return False
        if len(self.header.state_root) != 32:
            return False
        if len(self.header.txs_root) != 32:
            return False
        if len(self.header.receipts_root) != 32:
            return False
        if len(self.header.validator) != 32:
            return False
        if len(self.header.signature) != 64:
            return False
        if self.header.gas_used > self.header.gas_limit:
            return False
        
        return True
    
    def serialize(self) -> bytes:
        """
        Serialize block to bytes for storage or transmission.
        
        Returns:
            bytes: Serialized block data
        """
        # Simple JSON serialization for now
        # TODO: Implement more efficient binary serialization (e.g., RLP, Protobuf)
        block_data = {
            "header": {
                "version": self.header.version,
                "height": self.header.height,
                "timestamp": self.header.timestamp,
                "previous_hash": self.header.previous_hash.hex(),
                "state_root": self.header.state_root.hex(),
                "txs_root": self.header.txs_root.hex(),
                "receipts_root": self.header.receipts_root.hex(),
                "validator": self.header.validator.hex(),
                "signature": self.header.signature.hex(),
                "gas_used": self.header.gas_used,
                "gas_limit": self.header.gas_limit,
                "extra_data": self.header.extra_data.hex(),
            },
            "transactions": [tx.serialize().hex() for tx in self.transactions]
        }
        return json.dumps(block_data, separators=(',', ':')).encode('utf-8')
    
    @classmethod
    def deserialize(cls, data: bytes) -> 'Block':
        """
        Deserialize block from bytes.
        
        Args:
            data: Serialized block data
            
        Returns:
            Block: Deserialized block object
        """
        # Import here to avoid circular dependency
        from .transaction import Transaction
        
        block_data = json.loads(data.decode('utf-8'))
        header_data = block_data["header"]
        
        header = BlockHeader(
            version=header_data["version"],
            height=header_data["height"],
            timestamp=header_data["timestamp"],
            previous_hash=bytes.fromhex(header_data["previous_hash"]),
            state_root=bytes.fromhex(header_data["state_root"]),
            txs_root=bytes.fromhex(header_data["txs_root"]),
            receipts_root=bytes.fromhex(header_data["receipts_root"]),
            validator=bytes.fromhex(header_data["validator"]),
            signature=bytes.fromhex(header_data["signature"]),
            gas_used=header_data["gas_used"],
            gas_limit=header_data["gas_limit"],
            extra_data=bytes.fromhex(header_data["extra_data"]),
        )
        
        transactions = [
            Transaction.deserialize(bytes.fromhex(tx_hex))
            for tx_hex in block_data["transactions"]
        ]
        
        return cls(header=header, transactions=transactions)
    
    @staticmethod
    def create_genesis(
        chain_id: int = 1,
        timestamp: Optional[int] = None,
        validator: bytes = b'\x00' * 32,
    ) -> 'Block':
        """
        Create the genesis (first) block for the blockchain.
        
        Args:
            chain_id: Blockchain network ID
            timestamp: Genesis timestamp (defaults to current time)
            validator: Genesis validator public key
            
        Returns:
            Block: Genesis block
        """
        if timestamp is None:
            timestamp = int(time.time())
        
        # Genesis block has no previous hash (all zeros)
        previous_hash = b'\x00' * 32
        
        # Empty merkle roots for genesis
        state_root = b'\x00' * 32
        txs_root = b'\x00' * 32
        receipts_root = b'\x00' * 32
        
        # Create header
        header = BlockHeader(
            version=1,
            height=0,
            timestamp=timestamp,
            previous_hash=previous_hash,
            state_root=state_root,
            txs_root=txs_root,
            receipts_root=receipts_root,
            validator=validator,
            signature=b'\x00' * 64,  # Genesis doesn't need real signature
            gas_used=0,
            gas_limit=10_000_000,  # 10M gas limit
            extra_data=f"ModernTensor Genesis - Chain {chain_id}".encode('utf-8'),
        )
        
        return Block(header=header, transactions=[])
