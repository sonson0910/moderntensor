"""
Claim Manager for reward claiming with Merkle proofs.

This module manages reward claims using Merkle trees for efficient
verification and gas savings.
"""

import hashlib
from typing import Dict, List, Optional
from dataclasses import dataclass


@dataclass
class ClaimData:
    """
    Data for a claim epoch.

    Attributes:
        root: Merkle root for the epoch
        rewards: Dict mapping addresses to reward amounts
        claims: Set of (address, amount) that have been claimed
    """
    root: bytes
    rewards: Dict[str, int]
    claims: set


class MerkleTree:
    """
    Simple Merkle tree implementation for claim verification.
    """

    def __init__(self, leaves: List[bytes]):
        """
        Initialize Merkle tree.

        Args:
            leaves: List of leaf hashes
        """
        if not leaves:
            raise ValueError("Leaves list cannot be empty")

        self.leaves = leaves
        self.tree = self._build_tree(leaves)

    def _build_tree(self, nodes: List[bytes]) -> List[List[bytes]]:
        """
        Build Merkle tree from leaves.

        Uses canonical (sorted) ordering for deterministic hashing,
        consistent with verify_proof().

        Args:
            nodes: List of nodes at current level

        Returns:
            Tree as list of levels
        """
        tree = [nodes]

        while len(nodes) > 1:
            level = []
            for i in range(0, len(nodes), 2):
                if i + 1 < len(nodes):
                    # Hash pair using canonical (sorted) ordering
                    left, right = nodes[i], nodes[i + 1]
                    if left < right:
                        combined = left + right
                    else:
                        combined = right + left
                    parent = hashlib.sha256(combined).digest()
                else:
                    # Odd node, carry up
                    parent = nodes[i]
                level.append(parent)
            nodes = level
            tree.append(level)

        return tree

    def get_root(self) -> bytes:
        """
        Get Merkle root.

        Returns:
            Root hash
        """
        return self.tree[-1][0] if self.tree else b'\x00' * 32

    def get_proof(self, leaf_index: int) -> List[bytes]:
        """
        Get Merkle proof for a leaf.

        Args:
            leaf_index: Index of leaf in leaves list

        Returns:
            List of hashes forming the proof
        """
        if leaf_index < 0 or leaf_index >= len(self.leaves):
            raise ValueError(f"Invalid leaf index: {leaf_index}")

        proof = []
        index = leaf_index

        for level in self.tree[:-1]:  # Exclude root level
            if index % 2 == 0:
                # Right sibling
                if index + 1 < len(level):
                    proof.append(level[index + 1])
            else:
                # Left sibling
                proof.append(level[index - 1])

            index //= 2

        return proof

    def verify_proof(self, leaf: bytes, proof: List[bytes], root: bytes) -> bool:
        """
        Verify Merkle proof.

        Args:
            leaf: Leaf hash to verify
            proof: Merkle proof
            root: Expected root hash

        Returns:
            True if proof is valid
        """
        current = leaf

        for sibling in proof:
            # Consistent ordering: concatenate in sorted order for deterministic hashing
            if current < sibling:
                combined = current + sibling
            else:
                combined = sibling + current

            current = hashlib.sha256(combined).digest()

        return current == root


class ClaimManager:
    """
    Manages reward claims with Merkle proofs.

    This system allows efficient verification of claims without
    storing all reward data on-chain.
    """

    def __init__(self):
        """Initialize claim manager."""
        self.pending_claims: Dict[int, ClaimData] = {}

    def create_claim_tree(
        self,
        epoch: int,
        rewards: Dict[str, int]
    ) -> bytes:
        """
        Create Merkle tree for claims.

        Args:
            epoch: Epoch number
            rewards: Dict mapping addresses to reward amounts

        Returns:
            Merkle root
        """
        if not rewards:
            # Empty tree
            root = hashlib.sha256(b'empty').digest()
            self.pending_claims[epoch] = ClaimData(
                root=root,
                rewards={},
                claims=set()
            )
            return root

        # Create leaves (address, amount) pairs
        leaves = []
        for address, amount in sorted(rewards.items()):
            leaf = self._hash_claim(address, amount)
            leaves.append(leaf)

        # Build tree
        tree = MerkleTree(leaves)
        root = tree.get_root()

        # Store for later verification
        self.pending_claims[epoch] = ClaimData(
            root=root,
            rewards=rewards.copy(),
            claims=set()
        )

        return root

    def get_claim_proof(
        self,
        epoch: int,
        address: str
    ) -> Optional[List[bytes]]:
        """
        Get Merkle proof for a claim.

        Args:
            epoch: Epoch number
            address: Claimer address

        Returns:
            Merkle proof or None if not found
        """
        claim_data = self.pending_claims.get(epoch)
        if not claim_data or address not in claim_data.rewards:
            return None

        # Rebuild tree to get proof
        leaves = []
        addresses = sorted(claim_data.rewards.keys())
        for addr in addresses:
            amount = claim_data.rewards[addr]
            leaf = self._hash_claim(addr, amount)
            leaves.append(leaf)

        tree = MerkleTree(leaves)
        leaf_index = addresses.index(address)

        return tree.get_proof(leaf_index)

    def claim_reward(
        self,
        epoch: int,
        address: str,
        amount: int,
        proof: List[bytes]
    ) -> bool:
        """
        Claim reward with Merkle proof.

        Args:
            epoch: Epoch number
            address: Claimer address
            amount: Claimed amount
            proof: Merkle proof

        Returns:
            True if claim successful
        """
        # Check if epoch exists
        claim_data = self.pending_claims.get(epoch)
        if not claim_data:
            raise ValueError(f"Invalid epoch: {epoch}")

        # Check if already claimed
        if (address, amount) in claim_data.claims:
            raise ValueError(f"Already claimed for epoch {epoch}")

        # Verify amount matches
        expected_amount = claim_data.rewards.get(address)
        if expected_amount is None:
            raise ValueError(f"No reward for address {address} in epoch {epoch}")

        if amount != expected_amount:
            raise ValueError(f"Amount mismatch: expected {expected_amount}, got {amount}")

        # Verify proof
        leaf = self._hash_claim(address, amount)

        # Rebuild tree for verification
        tree = MerkleTree([
            self._hash_claim(addr, amt)
            for addr, amt in sorted(claim_data.rewards.items())
        ])

        if not tree.verify_proof(leaf, proof, claim_data.root):
            raise ValueError("Invalid proof")

        # Mark as claimed
        claim_data.claims.add((address, amount))

        return True

    def _hash_claim(self, address: str, amount: int) -> bytes:
        """
        Hash a claim for Merkle tree.

        Args:
            address: Claimer address
            amount: Reward amount

        Returns:
            Hash of claim
        """
        claim_bytes = address.encode() + amount.to_bytes(32, 'big')
        return hashlib.sha256(claim_bytes).digest()

    def get_claim_status(self, epoch: int, address: str) -> Dict:
        """
        Get claim status for an address.

        Args:
            epoch: Epoch number
            address: Address to check

        Returns:
            Dictionary with claim status
        """
        claim_data = self.pending_claims.get(epoch)
        if not claim_data:
            return {'exists': False}

        reward = claim_data.rewards.get(address)
        if reward is None:
            return {'exists': False}

        claimed = (address, reward) in claim_data.claims

        return {
            'exists': True,
            'reward': reward,
            'claimed': claimed
        }
