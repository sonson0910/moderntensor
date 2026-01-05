"""
State management for ModernTensor Layer 1 blockchain.

Implements account-based state model with Merkle tree for verification.
"""
import hashlib
import json
from dataclasses import dataclass, field
from typing import Dict, Optional
from collections import OrderedDict


@dataclass
class Account:
    """
    Account state in the blockchain.
    
    Uses an Ethereum-style account model.
    
    Attributes:
        nonce: Number of transactions sent from this account
        balance: Account balance in smallest unit
        storage_root: Merkle root of contract storage (for smart contracts)
        code_hash: Hash of contract code (for smart contracts)
    """
    nonce: int = 0
    balance: int = 0
    storage_root: bytes = field(default_factory=lambda: b'\x00' * 32)  # 32 bytes
    code_hash: bytes = field(default_factory=lambda: b'\x00' * 32)     # 32 bytes
    
    def is_empty(self) -> bool:
        """
        Check if account is empty (has no state).
        
        Returns:
            bool: True if account is empty
        """
        return (
            self.nonce == 0 and
            self.balance == 0 and
            self.storage_root == b'\x00' * 32 and
            self.code_hash == b'\x00' * 32
        )
    
    def serialize(self) -> bytes:
        """Serialize account to bytes."""
        account_data = {
            "nonce": self.nonce,
            "balance": self.balance,
            "storage_root": self.storage_root.hex(),
            "code_hash": self.code_hash.hex(),
        }
        return json.dumps(account_data, separators=(',', ':')).encode('utf-8')
    
    @classmethod
    def deserialize(cls, data: bytes) -> 'Account':
        """Deserialize account from bytes."""
        account_data = json.loads(data.decode('utf-8'))
        return cls(
            nonce=account_data["nonce"],
            balance=account_data["balance"],
            storage_root=bytes.fromhex(account_data["storage_root"]),
            code_hash=bytes.fromhex(account_data["code_hash"]),
        )


class StateDB:
    """
    Account-based state database with Merkle tree verification.
    
    Manages the global state of all accounts in the blockchain.
    Uses an in-memory cache with persistence to disk.
    """
    
    def __init__(self, storage_path: Optional[str] = None):
        """
        Initialize state database.
        
        Args:
            storage_path: Path for persistent storage (None for in-memory only)
        """
        self.storage_path = storage_path
        self.accounts: Dict[bytes, Account] = {}  # address -> Account
        self.cache: Dict[bytes, Account] = {}     # Cache for pending changes
        self.dirty: set = set()                   # Set of modified addresses
        
        # Contract storage: address -> (key -> value)
        self.contract_storage: Dict[bytes, Dict[bytes, bytes]] = {}
        
        # If storage path provided, load existing state
        if storage_path:
            self._load_from_disk()
    
    def get_account(self, address: bytes) -> Account:
        """
        Get account state.
        
        Args:
            address: Account address (20 bytes)
            
        Returns:
            Account: Account object (returns empty account if doesn't exist)
        """
        # Check cache first
        if address in self.cache:
            return self.cache[address]
        
        # Check main storage
        if address in self.accounts:
            account = self.accounts[address]
            self.cache[address] = account
            return account
        
        # Return empty account if not found
        return Account()
    
    def set_account(self, address: bytes, account: Account) -> None:
        """
        Update account state.
        
        Args:
            address: Account address (20 bytes)
            account: Account object with new state
        """
        self.cache[address] = account
        self.dirty.add(address)
    
    def add_balance(self, address: bytes, amount: int) -> None:
        """
        Add to account balance.
        
        Args:
            address: Account address
            amount: Amount to add (in smallest unit)
        """
        account = self.get_account(address)
        account.balance += amount
        self.set_account(address, account)
    
    def sub_balance(self, address: bytes, amount: int) -> bool:
        """
        Subtract from account balance.
        
        Args:
            address: Account address
            amount: Amount to subtract
            
        Returns:
            bool: True if successful, False if insufficient balance
        """
        account = self.get_account(address)
        if account.balance < amount:
            return False
        account.balance -= amount
        self.set_account(address, account)
        return True
    
    def get_balance(self, address: bytes) -> int:
        """Get account balance."""
        return self.get_account(address).balance
    
    def get_nonce(self, address: bytes) -> int:
        """Get account nonce."""
        return self.get_account(address).nonce
    
    def set_nonce(self, address: bytes, nonce: int) -> None:
        """Set account nonce."""
        account = self.get_account(address)
        account.nonce = nonce
        self.set_account(address, account)
    
    def increment_nonce(self, address: bytes) -> None:
        """Increment account nonce by 1."""
        account = self.get_account(address)
        account.nonce += 1
        self.set_account(address, account)
    
    def get_code(self, address: bytes) -> bytes:
        """
        Get contract code for an address.
        
        Args:
            address: Contract address
            
        Returns:
            bytes: Contract code (empty if not a contract)
        """
        # TODO: Implement contract code storage
        return b''
    
    def set_code(self, address: bytes, code: bytes) -> None:
        """
        Set contract code for an address.
        
        Args:
            address: Contract address
            code: Contract bytecode
        """
        # TODO: Implement contract code storage
        account = self.get_account(address)
        account.code_hash = hashlib.sha256(code).digest()
        self.set_account(address, account)
    
    def get_state_root(self) -> bytes:
        """
        Calculate Merkle root of current state.
        
        Returns:
            bytes: 32-byte Merkle root hash
        """
        # Simple implementation: hash all account states
        # TODO: Implement proper Merkle Patricia Trie
        
        if not self.accounts and not self.cache:
            return b'\x00' * 32
        
        # Merge accounts and cache
        all_accounts = dict(self.accounts)
        all_accounts.update(self.cache)
        
        # Sort addresses for deterministic hashing
        sorted_addresses = sorted(all_accounts.keys())
        
        # Hash each account state
        state_data = []
        for address in sorted_addresses:
            account = all_accounts[address]
            if not account.is_empty():
                entry = {
                    "address": address.hex(),
                    "nonce": account.nonce,
                    "balance": account.balance,
                    "storage_root": account.storage_root.hex(),
                    "code_hash": account.code_hash.hex(),
                }
                state_data.append(entry)
        
        # Calculate root hash
        state_json = json.dumps(state_data, sort_keys=True, separators=(',', ':'))
        return hashlib.sha256(state_json.encode('utf-8')).digest()
    
    def commit(self) -> bytes:
        """
        Commit pending changes to main state.
        
        Returns:
            bytes: New state root hash
        """
        # Apply cached changes to main storage
        for address in self.dirty:
            if address in self.cache:
                self.accounts[address] = self.cache[address]
        
        # Clear cache and dirty set
        self.cache.clear()
        self.dirty.clear()
        
        # Persist to disk if storage path provided
        if self.storage_path:
            self._save_to_disk()
        
        return self.get_state_root()
    
    def rollback(self) -> None:
        """
        Rollback uncommitted changes.
        """
        self.cache.clear()
        self.dirty.clear()
    
    def snapshot(self) -> int:
        """
        Create a snapshot of current state.
        
        Returns:
            int: Snapshot ID
        """
        # TODO: Implement snapshot mechanism for complex state transitions
        return 0
    
    def revert_to_snapshot(self, snapshot_id: int) -> None:
        """
        Revert state to a previous snapshot.
        
        Args:
            snapshot_id: ID of snapshot to revert to
        """
        # TODO: Implement snapshot revert
        pass
    
    def _load_from_disk(self) -> None:
        """Load state from persistent storage."""
        # TODO: Implement disk persistence
        pass
    
    def _save_to_disk(self) -> None:
        """Save state to persistent storage."""
        # TODO: Implement disk persistence
        pass
    
    def exists(self, address: bytes) -> bool:
        """
        Check if account exists (is not empty).
        
        Args:
            address: Account address
            
        Returns:
            bool: True if account exists and is not empty
        """
        account = self.get_account(address)
        return not account.is_empty()
    
    def delete_account(self, address: bytes) -> None:
        """
        Delete an account (set to empty state).
        
        Args:
            address: Account address to delete
        """
        self.set_account(address, Account())
    
    def transfer(self, from_addr: bytes, to_addr: bytes, amount: int) -> bool:
        """
        Transfer balance between accounts.
        
        Args:
            from_addr: Source address
            to_addr: Destination address
            amount: Amount to transfer
            
        Returns:
            bool: True if successful, False if insufficient balance
        """
        if not self.sub_balance(from_addr, amount):
            return False
        self.add_balance(to_addr, amount)
        return True
