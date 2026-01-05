"""
Cryptography primitives for ModernTensor Layer 1 blockchain.

Provides key management, signing, verification, and Merkle tree functionality.
"""
import hashlib
import secrets
from typing import List, Optional, Tuple
from dataclasses import dataclass


class KeyPair:
    """
    Public/private key pair for blockchain accounts.
    
    Uses secp256k1 elliptic curve (same as Bitcoin/Ethereum).
    """
    
    def __init__(self, private_key: Optional[bytes] = None):
        """
        Initialize key pair.
        
        Args:
            private_key: 32-byte private key (generates new if None)
        """
        if private_key is None:
            self.private_key = self._generate()
        else:
            if len(private_key) != 32:
                raise ValueError("Private key must be 32 bytes")
            self.private_key = private_key
        
        self.public_key = self._derive_public()
    
    def _generate(self) -> bytes:
        """
        Generate a new random private key.
        
        Returns:
            bytes: 32-byte private key
        """
        return secrets.token_bytes(32)
    
    def _derive_public(self) -> bytes:
        """
        Derive public key from private key.
        
        Returns:
            bytes: 64-byte uncompressed public key (without 0x04 prefix)
        """
        # TODO: Implement proper secp256k1 key derivation
        # For now, use a placeholder (hash of private key)
        # Real implementation should use ecdsa library or pycryptodome
        h = hashlib.sha256(self.private_key).digest()
        return h + hashlib.sha256(h).digest()  # 64 bytes
    
    def sign(self, message: bytes) -> bytes:
        """
        Sign a message with the private key.
        
        Args:
            message: Message to sign
            
        Returns:
            bytes: 65-byte signature (r + s + v)
        """
        # TODO: Implement proper ECDSA signing with secp256k1
        # For now, use a placeholder
        msg_hash = hashlib.sha256(message).digest()
        sig_hash = hashlib.sha256(self.private_key + msg_hash).digest()
        # r (32 bytes) + s (32 bytes) + v (1 byte)
        r = sig_hash
        s = hashlib.sha256(sig_hash).digest()
        v = bytes([27])  # Recovery ID
        return r + s + v
    
    @staticmethod
    def verify(message: bytes, signature: bytes, public_key: bytes) -> bool:
        """
        Verify a signature against a public key.
        
        Args:
            message: Original message
            signature: Signature to verify (65 bytes)
            public_key: Public key (64 bytes)
            
        Returns:
            bool: True if signature is valid
        """
        # TODO: Implement proper ECDSA verification
        # For now, always return True as placeholder
        return len(signature) == 65 and len(public_key) == 64
    
    @staticmethod
    def recover_public_key(message: bytes, signature: bytes) -> Optional[bytes]:
        """
        Recover public key from message and signature.
        
        Args:
            message: Original message
            signature: Signature (65 bytes with recovery ID)
            
        Returns:
            Optional[bytes]: Recovered public key (64 bytes) or None if invalid
        """
        # TODO: Implement ECDSA public key recovery
        # This is critical for transaction verification
        if len(signature) != 65:
            return None
        # Placeholder: return dummy public key
        return b'\x00' * 64
    
    def address(self) -> bytes:
        """
        Derive address from public key.
        
        Uses Ethereum-style addressing: last 20 bytes of keccak256(pubkey).
        For now, uses SHA256 instead of keccak256.
        
        Returns:
            bytes: 20-byte address
        """
        # TODO: Use keccak256 instead of SHA256 for Ethereum compatibility
        pub_hash = hashlib.sha256(self.public_key).digest()
        return pub_hash[-20:]  # Last 20 bytes
    
    def export_private_key(self) -> str:
        """
        Export private key as hex string.
        
        Returns:
            str: Hex-encoded private key
        """
        return self.private_key.hex()
    
    @classmethod
    def from_private_key_hex(cls, hex_str: str) -> 'KeyPair':
        """
        Create KeyPair from hex-encoded private key.
        
        Args:
            hex_str: Hex string of private key
            
        Returns:
            KeyPair: New key pair instance
        """
        private_key = bytes.fromhex(hex_str)
        return cls(private_key)


class MerkleTree:
    """
    Merkle tree for transactions and state verification.
    
    Provides cryptographic proofs of inclusion.
    """
    
    def __init__(self, leaves: List[bytes]):
        """
        Initialize Merkle tree.
        
        Args:
            leaves: List of leaf hashes
        """
        if not leaves:
            self.leaves = [b'\x00' * 32]
        else:
            self.leaves = leaves
        
        self.tree = self._build_tree()
        # Root is the first element of the first level (topmost)
        self._root = self.tree[0][0] if self.tree and self.tree[0] else b'\x00' * 32
    
    def _build_tree(self) -> List[List[bytes]]:
        """
        Build the Merkle tree.
        
        Returns:
            List[List[bytes]]: Tree structure (levels from root to leaves)
        """
        if not self.leaves:
            return [[b'\x00' * 32]]
        
        # Start with leaves at bottom level
        current_level = list(self.leaves)
        tree = [current_level]
        
        # Build tree bottom-up
        while len(current_level) > 1:
            next_level = []
            
            # Process pairs
            for i in range(0, len(current_level), 2):
                left = current_level[i]
                # If odd number of nodes, duplicate the last one
                right = current_level[i + 1] if i + 1 < len(current_level) else left
                
                # Hash the pair
                parent = self._hash_pair(left, right)
                next_level.append(parent)
            
            tree.insert(0, next_level)  # Insert at beginning (root first)
            current_level = next_level
        
        return tree
    
    @staticmethod
    def _hash_pair(left: bytes, right: bytes) -> bytes:
        """
        Hash a pair of nodes.
        
        Args:
            left: Left node hash
            right: Right node hash
            
        Returns:
            bytes: Hash of concatenated nodes
        """
        return hashlib.sha256(left + right).digest()
    
    def root(self) -> bytes:
        """
        Get Merkle root.
        
        Returns:
            bytes: 32-byte root hash
        """
        return self._root
    
    def get_proof(self, leaf_index: int) -> List[Tuple[bytes, bool]]:
        """
        Get Merkle proof for a leaf.
        
        Args:
            leaf_index: Index of leaf to prove
            
        Returns:
            List[Tuple[bytes, bool]]: List of (hash, is_left) pairs for proof path
        """
        if leaf_index < 0 or leaf_index >= len(self.leaves):
            return []
        
        proof = []
        index = leaf_index
        
        # Traverse from leaf to root
        for level in reversed(range(1, len(self.tree))):
            level_nodes = self.tree[level]
            
            # Find sibling
            if index % 2 == 0:
                # Current node is left, sibling is right
                sibling_index = index + 1
                is_left = False
            else:
                # Current node is right, sibling is left
                sibling_index = index - 1
                is_left = True
            
            # Add sibling to proof (if it exists)
            if sibling_index < len(level_nodes):
                sibling = level_nodes[sibling_index]
                proof.append((sibling, is_left))
            
            # Move to parent level
            index = index // 2
        
        return proof
    
    @staticmethod
    def verify_proof(
        leaf: bytes,
        proof: List[Tuple[bytes, bool]],
        root: bytes
    ) -> bool:
        """
        Verify a Merkle proof.
        
        Args:
            leaf: Leaf hash to verify
            proof: Merkle proof (list of sibling hashes with positions)
            root: Expected root hash
            
        Returns:
            bool: True if proof is valid
        """
        current_hash = leaf
        
        # Apply each proof step
        for sibling, is_left in proof:
            if is_left:
                # Sibling is on the left
                current_hash = MerkleTree._hash_pair(sibling, current_hash)
            else:
                # Sibling is on the right
                current_hash = MerkleTree._hash_pair(current_hash, sibling)
        
        return current_hash == root
    
    @staticmethod
    def from_transactions(transactions: List) -> 'MerkleTree':
        """
        Create Merkle tree from list of transactions.
        
        Args:
            transactions: List of Transaction objects
            
        Returns:
            MerkleTree: Merkle tree of transaction hashes
        """
        leaves = [tx.hash() for tx in transactions]
        return MerkleTree(leaves)


def keccak256(data: bytes) -> bytes:
    """
    Keccak256 hash function (used by Ethereum).
    
    For now, uses SHA256 as placeholder.
    TODO: Implement proper Keccak256.
    
    Args:
        data: Data to hash
        
    Returns:
        bytes: 32-byte hash
    """
    # TODO: Use actual keccak256 (pip install pysha3 or pycryptodome)
    return hashlib.sha256(data).digest()


def sha256(data: bytes) -> bytes:
    """
    SHA256 hash function.
    
    Args:
        data: Data to hash
        
    Returns:
        bytes: 32-byte hash
    """
    return hashlib.sha256(data).digest()
