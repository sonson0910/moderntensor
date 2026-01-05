"""
Blockchain indexer for fast queries.

This module provides indexing for blocks, transactions, and addresses
to enable efficient querying of blockchain data.
"""

import logging
from typing import List, Optional, Dict, Set
from collections import defaultdict

from sdk.blockchain.block import Block
from sdk.blockchain.transaction import Transaction
from sdk.storage.blockchain_db import LevelDBWrapper

logger = logging.getLogger(__name__)


class Indexer:
    """Index blockchain data for fast queries"""
    
    # Key prefixes
    PREFIX_TX_BY_ADDRESS = b'a'    # address -> [tx_hashes]
    PREFIX_BALANCE = b'B'           # address -> balance
    PREFIX_NONCE = b'N'             # address -> nonce
    PREFIX_TX_COUNT = b'c'          # address -> tx_count
    
    def __init__(self, db_path: str):
        """
        Initialize indexer.
        
        Args:
            db_path: Path to index database
        """
        self.db = LevelDBWrapper(db_path)
        logger.info(f"Indexer initialized at {db_path}")
    
    def index_block(self, block: Block):
        """
        Index block data.
        
        Args:
            block: Block to index
        """
        # Index transactions
        for tx in block.transactions:
            self._index_transaction(tx, block)
        
        logger.debug(f"Indexed block {block.header.height} with {len(block.transactions)} transactions")
    
    def _index_transaction(self, tx: Transaction, block: Block):
        """Index a single transaction"""
        tx_hash = tx.hash()
        
        # Index by sender address
        self._add_transaction_to_address(tx.from_address, tx_hash)
        
        # Index by receiver address (if not contract creation)
        if tx.to_address:
            self._add_transaction_to_address(tx.to_address, tx_hash)
        
        # Update transaction counts
        self._increment_tx_count(tx.from_address)
        if tx.to_address:
            self._increment_tx_count(tx.to_address)
    
    def _add_transaction_to_address(self, address: bytes, tx_hash: bytes):
        """Add transaction hash to address index"""
        key = self._make_key(self.PREFIX_TX_BY_ADDRESS, address)
        
        # Get existing transactions
        existing = self.db.get(key)
        
        if existing:
            # Append new tx_hash (32 bytes per hash)
            new_data = existing + tx_hash
        else:
            new_data = tx_hash
        
        self.db.put(key, new_data)
    
    def get_transactions_by_address(self, address: bytes, limit: int = 100) -> List[bytes]:
        """
        Get transaction hashes for an address.
        
        Args:
            address: Address to query
            limit: Maximum number of transactions to return
            
        Returns:
            List of transaction hashes
        """
        key = self._make_key(self.PREFIX_TX_BY_ADDRESS, address)
        data = self.db.get(key)
        
        if not data:
            return []
        
        # Extract transaction hashes (32 bytes each)
        tx_hashes = []
        for i in range(0, len(data), 32):
            if len(tx_hashes) >= limit:
                break
            tx_hashes.append(data[i:i+32])
        
        return tx_hashes
    
    def get_transaction_count(self, address: bytes) -> int:
        """
        Get number of transactions for an address.
        
        Args:
            address: Address to query
            
        Returns:
            Number of transactions
        """
        key = self._make_key(self.PREFIX_TX_COUNT, address)
        data = self.db.get(key)
        
        if not data:
            return 0
        
        return int.from_bytes(data, 'big')
    
    def _increment_tx_count(self, address: bytes):
        """Increment transaction count for address"""
        current = self.get_transaction_count(address)
        new_count = current + 1
        
        key = self._make_key(self.PREFIX_TX_COUNT, address)
        self.db.put(key, new_count.to_bytes(8, 'big'))
    
    def update_balance(self, address: bytes, balance: int):
        """
        Update balance for an address.
        
        Args:
            address: Address
            balance: New balance
        """
        key = self._make_key(self.PREFIX_BALANCE, address)
        self.db.put(key, balance.to_bytes(32, 'big'))
    
    def get_balance(self, address: bytes) -> int:
        """
        Get balance for an address.
        
        Args:
            address: Address to query
            
        Returns:
            Balance (0 if not found)
        """
        key = self._make_key(self.PREFIX_BALANCE, address)
        data = self.db.get(key)
        
        if not data:
            return 0
        
        return int.from_bytes(data, 'big')
    
    def update_nonce(self, address: bytes, nonce: int):
        """
        Update nonce for an address.
        
        Args:
            address: Address
            nonce: New nonce
        """
        key = self._make_key(self.PREFIX_NONCE, address)
        self.db.put(key, nonce.to_bytes(8, 'big'))
    
    def get_nonce(self, address: bytes) -> int:
        """
        Get nonce for an address.
        
        Args:
            address: Address to query
            
        Returns:
            Nonce (0 if not found)
        """
        key = self._make_key(self.PREFIX_NONCE, address)
        data = self.db.get(key)
        
        if not data:
            return 0
        
        return int.from_bytes(data, 'big')
    
    def get_address_summary(self, address: bytes) -> Dict:
        """
        Get summary information for an address.
        
        Args:
            address: Address to query
            
        Returns:
            Dictionary with balance, nonce, and transaction count
        """
        return {
            'address': address.hex(),
            'balance': self.get_balance(address),
            'nonce': self.get_nonce(address),
            'transaction_count': self.get_transaction_count(address),
        }
    
    def close(self):
        """Close indexer database"""
        self.db.close()
        logger.info("Indexer closed")
    
    @staticmethod
    def _make_key(prefix: bytes, key: bytes) -> bytes:
        """Create prefixed key"""
        return prefix + key
    
    def __enter__(self):
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()


class MemoryIndexer:
    """In-memory indexer for testing and development"""
    
    def __init__(self):
        """Initialize memory indexer"""
        self.tx_by_address: Dict[bytes, List[bytes]] = defaultdict(list)
        self.balances: Dict[bytes, int] = {}
        self.nonces: Dict[bytes, int] = {}
        self.tx_counts: Dict[bytes, int] = defaultdict(int)
        
        logger.info("MemoryIndexer initialized")
    
    def index_block(self, block: Block):
        """Index block data"""
        for tx in block.transactions:
            self._index_transaction(tx, block)
        
        logger.debug(f"Indexed block {block.header.height} with {len(block.transactions)} transactions")
    
    def _index_transaction(self, tx: Transaction, block: Block):
        """Index a single transaction"""
        tx_hash = tx.hash()
        
        # Index by addresses
        self.tx_by_address[tx.from_address].append(tx_hash)
        if tx.to_address:
            self.tx_by_address[tx.to_address].append(tx_hash)
        
        # Update counts
        self.tx_counts[tx.from_address] += 1
        if tx.to_address:
            self.tx_counts[tx.to_address] += 1
    
    def get_transactions_by_address(self, address: bytes, limit: int = 100) -> List[bytes]:
        """Get transaction hashes for an address"""
        return self.tx_by_address.get(address, [])[:limit]
    
    def get_transaction_count(self, address: bytes) -> int:
        """Get number of transactions for an address"""
        return self.tx_counts.get(address, 0)
    
    def update_balance(self, address: bytes, balance: int):
        """Update balance for an address"""
        self.balances[address] = balance
    
    def get_balance(self, address: bytes) -> int:
        """Get balance for an address"""
        return self.balances.get(address, 0)
    
    def update_nonce(self, address: bytes, nonce: int):
        """Update nonce for an address"""
        self.nonces[address] = nonce
    
    def get_nonce(self, address: bytes) -> int:
        """Get nonce for an address"""
        return self.nonces.get(address, 0)
    
    def get_address_summary(self, address: bytes) -> Dict:
        """Get summary information for an address"""
        return {
            'address': address.hex(),
            'balance': self.get_balance(address),
            'nonce': self.get_nonce(address),
            'transaction_count': self.get_transaction_count(address),
        }
    
    def close(self):
        """Close indexer (no-op for memory)"""
        pass
    
    def __enter__(self):
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
