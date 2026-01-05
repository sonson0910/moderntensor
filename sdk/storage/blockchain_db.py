"""
Persistent blockchain database for ModernTensor Layer 1.

This module provides persistent storage for blocks, transactions,
and blockchain metadata using LevelDB.
"""

import os
import logging
import json
from typing import Optional, List, Dict, Any
from pathlib import Path

try:
    import plyvel as leveldb
except ImportError:
    leveldb = None

from sdk.blockchain.block import Block, BlockHeader
from sdk.blockchain.transaction import Transaction

logger = logging.getLogger(__name__)


class LevelDBWrapper:
    """Wrapper for LevelDB operations"""
    
    def __init__(self, path: str, create_if_missing: bool = True):
        """
        Initialize LevelDB wrapper.
        
        Args:
            path: Database path
            create_if_missing: Create database if it doesn't exist
        """
        if leveldb is None:
            raise ImportError(
                "plyvel not installed. Install with: pip install plyvel"
            )
        
        # Ensure directory exists
        os.makedirs(path, exist_ok=True)
        
        self.db = leveldb.DB(
            path,
            create_if_missing=create_if_missing,
            error_if_exists=False
        )
        self.path = path
        logger.info(f"LevelDB opened at {path}")
    
    def get(self, key: bytes) -> Optional[bytes]:
        """Get value by key"""
        try:
            return self.db.get(key)
        except Exception as e:
            logger.error(f"Error getting key {key.hex()[:8]}: {e}")
            return None
    
    def put(self, key: bytes, value: bytes):
        """Put key-value pair"""
        try:
            self.db.put(key, value)
        except Exception as e:
            logger.error(f"Error putting key {key.hex()[:8]}: {e}")
            raise
    
    def delete(self, key: bytes):
        """Delete key"""
        try:
            self.db.delete(key)
        except Exception as e:
            logger.error(f"Error deleting key {key.hex()[:8]}: {e}")
            raise
    
    def batch_write(self, operations: List[tuple]):
        """
        Batch write operations.
        
        Args:
            operations: List of (operation, key, value) tuples
                       operation is 'put' or 'delete'
        """
        with self.db.write_batch() as batch:
            for op, key, *args in operations:
                if op == 'put':
                    batch.put(key, args[0])
                elif op == 'delete':
                    batch.delete(key)
    
    def iterator(self, prefix: Optional[bytes] = None):
        """
        Create iterator.
        
        Args:
            prefix: Optional prefix to filter keys
            
        Returns:
            Iterator over key-value pairs
        """
        if prefix:
            return self.db.iterator(prefix=prefix)
        return self.db.iterator()
    
    def close(self):
        """Close database"""
        if self.db:
            self.db.close()
            logger.info(f"LevelDB closed at {self.path}")
    
    def __enter__(self):
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()


class BlockchainDB:
    """Persistent storage for blockchain data"""
    
    # Key prefixes for different data types
    PREFIX_BLOCK = b'b'          # block_hash -> block_data
    PREFIX_BLOCK_HEIGHT = b'h'   # height -> block_hash
    PREFIX_TX = b't'             # tx_hash -> (block_hash, tx_data)
    PREFIX_HEADER = b'H'         # block_hash -> header_data
    PREFIX_META = b'm'           # metadata keys
    
    # Metadata keys
    META_BEST_HEIGHT = b'best_height'
    META_BEST_HASH = b'best_hash'
    META_GENESIS_HASH = b'genesis_hash'
    META_TOTAL_TXS = b'total_txs'
    
    def __init__(self, data_dir: str):
        """
        Initialize blockchain database.
        
        Args:
            data_dir: Directory for database files
        """
        self.data_dir = Path(data_dir)
        self.data_dir.mkdir(parents=True, exist_ok=True)
        
        # Open databases
        self.blocks_db = LevelDBWrapper(str(self.data_dir / "blocks"))
        self.index_db = LevelDBWrapper(str(self.data_dir / "index"))
        
        logger.info(f"BlockchainDB initialized at {data_dir}")
    
    def store_block(self, block: Block):
        """
        Store block and update indices.
        
        Args:
            block: Block to store
        """
        block_hash = block.hash()
        height = block.header.height
        
        # Serialize block
        block_data = self._serialize_block(block)
        
        # Prepare batch operations
        operations = [
            # Store full block
            ('put', self._make_key(self.PREFIX_BLOCK, block_hash), block_data),
            
            # Store header separately for faster access
            ('put', self._make_key(self.PREFIX_HEADER, block_hash),
             self._serialize_header(block.header)),
            
            # Index by height
            ('put', self._make_key(self.PREFIX_BLOCK_HEIGHT, height.to_bytes(8, 'big')),
             block_hash),
        ]
        
        # Store transactions
        for tx in block.transactions:
            tx_hash = tx.hash()
            tx_data = self._serialize_transaction(tx, block_hash)
            operations.append(
                ('put', self._make_key(self.PREFIX_TX, tx_hash), tx_data)
            )
        
        # Write batch
        self.blocks_db.batch_write(operations[:3])  # Block and header
        self.index_db.batch_write(operations[3:])   # Indices
        
        # Update metadata
        self._update_best_block(height, block_hash)
        self._increment_total_txs(len(block.transactions))
        
        logger.debug(f"Stored block {height} with {len(block.transactions)} transactions")
    
    def get_block(self, block_hash: bytes) -> Optional[Block]:
        """
        Retrieve block by hash.
        
        Args:
            block_hash: Block hash
            
        Returns:
            Block or None if not found
        """
        key = self._make_key(self.PREFIX_BLOCK, block_hash)
        data = self.blocks_db.get(key)
        
        if data is None:
            return None
        
        return self._deserialize_block(data)
    
    def get_block_by_height(self, height: int) -> Optional[Block]:
        """
        Retrieve block by height.
        
        Args:
            height: Block height
            
        Returns:
            Block or None if not found
        """
        # Get block hash from index
        key = self._make_key(self.PREFIX_BLOCK_HEIGHT, height.to_bytes(8, 'big'))
        block_hash = self.index_db.get(key)
        
        if block_hash is None:
            return None
        
        return self.get_block(block_hash)
    
    def get_block_header(self, block_hash: bytes) -> Optional[BlockHeader]:
        """
        Retrieve block header by hash.
        
        Args:
            block_hash: Block hash
            
        Returns:
            BlockHeader or None if not found
        """
        key = self._make_key(self.PREFIX_HEADER, block_hash)
        data = self.blocks_db.get(key)
        
        if data is None:
            return None
        
        return self._deserialize_header(data)
    
    def store_transaction(self, tx: Transaction, block_hash: bytes):
        """
        Store transaction with block reference.
        
        Args:
            tx: Transaction to store
            block_hash: Hash of block containing transaction
        """
        tx_hash = tx.hash()
        tx_data = self._serialize_transaction(tx, block_hash)
        
        key = self._make_key(self.PREFIX_TX, tx_hash)
        self.index_db.put(key, tx_data)
        
        logger.debug(f"Stored transaction {tx_hash.hex()[:8]}")
    
    def get_transaction(self, tx_hash: bytes) -> Optional[tuple]:
        """
        Retrieve transaction by hash.
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Tuple of (Transaction, block_hash) or None if not found
        """
        key = self._make_key(self.PREFIX_TX, tx_hash)
        data = self.index_db.get(key)
        
        if data is None:
            return None
        
        return self._deserialize_transaction(data)
    
    def get_best_height(self) -> int:
        """Get current best block height"""
        key = self._make_key(self.PREFIX_META, self.META_BEST_HEIGHT)
        data = self.index_db.get(key)
        
        if data is None:
            return 0
        
        return int.from_bytes(data, 'big')
    
    def get_best_hash(self) -> Optional[bytes]:
        """Get current best block hash"""
        key = self._make_key(self.PREFIX_META, self.META_BEST_HASH)
        return self.index_db.get(key)
    
    def get_genesis_hash(self) -> Optional[bytes]:
        """Get genesis block hash"""
        key = self._make_key(self.PREFIX_META, self.META_GENESIS_HASH)
        return self.index_db.get(key)
    
    def set_genesis_hash(self, genesis_hash: bytes):
        """Set genesis block hash"""
        key = self._make_key(self.PREFIX_META, self.META_GENESIS_HASH)
        self.index_db.put(key, genesis_hash)
    
    def get_total_transactions(self) -> int:
        """Get total number of transactions"""
        key = self._make_key(self.PREFIX_META, self.META_TOTAL_TXS)
        data = self.index_db.get(key)
        
        if data is None:
            return 0
        
        return int.from_bytes(data, 'big')
    
    def get_blocks_in_range(self, start_height: int, end_height: int) -> List[Block]:
        """
        Get blocks in height range.
        
        Args:
            start_height: Start height (inclusive)
            end_height: End height (inclusive)
            
        Returns:
            List of blocks
        """
        blocks = []
        
        for height in range(start_height, end_height + 1):
            block = self.get_block_by_height(height)
            if block:
                blocks.append(block)
        
        return blocks
    
    def block_exists(self, block_hash: bytes) -> bool:
        """Check if block exists"""
        key = self._make_key(self.PREFIX_BLOCK, block_hash)
        return self.blocks_db.get(key) is not None
    
    def transaction_exists(self, tx_hash: bytes) -> bool:
        """Check if transaction exists"""
        key = self._make_key(self.PREFIX_TX, tx_hash)
        return self.index_db.get(key) is not None
    
    def get_statistics(self) -> Dict[str, Any]:
        """Get database statistics"""
        return {
            'best_height': self.get_best_height(),
            'best_hash': self.get_best_hash().hex() if self.get_best_hash() else None,
            'genesis_hash': self.get_genesis_hash().hex() if self.get_genesis_hash() else None,
            'total_transactions': self.get_total_transactions(),
        }
    
    def close(self):
        """Close all databases"""
        self.blocks_db.close()
        self.index_db.close()
        logger.info("BlockchainDB closed")
    
    def _update_best_block(self, height: int, block_hash: bytes):
        """Update best block metadata"""
        operations = [
            ('put', self._make_key(self.PREFIX_META, self.META_BEST_HEIGHT),
             height.to_bytes(8, 'big')),
            ('put', self._make_key(self.PREFIX_META, self.META_BEST_HASH),
             block_hash),
        ]
        self.index_db.batch_write(operations)
    
    def _increment_total_txs(self, count: int):
        """Increment total transaction count"""
        current = self.get_total_transactions()
        new_total = current + count
        
        key = self._make_key(self.PREFIX_META, self.META_TOTAL_TXS)
        self.index_db.put(key, new_total.to_bytes(8, 'big'))
    
    @staticmethod
    def _make_key(prefix: bytes, key: bytes) -> bytes:
        """Create prefixed key"""
        return prefix + key
    
    @staticmethod
    def _serialize_block(block: Block) -> bytes:
        """Serialize block to bytes"""
        block_dict = {
            'header': {
                'version': block.header.version,
                'height': block.header.height,
                'timestamp': block.header.timestamp,
                'previous_hash': block.header.previous_hash.hex(),
                'state_root': block.header.state_root.hex(),
                'txs_root': block.header.txs_root.hex(),
                'receipts_root': block.header.receipts_root.hex(),
                'validator': block.header.validator.hex(),
                'signature': block.header.signature.hex(),
                'gas_used': block.header.gas_used,
                'gas_limit': block.header.gas_limit,
            },
            'transactions': [
                {
                    'nonce': tx.nonce,
                    'from': tx.from_address.hex(),
                    'to': tx.to_address.hex() if tx.to_address else None,
                    'value': tx.value,
                    'gas_price': tx.gas_price,
                    'gas_limit': tx.gas_limit,
                    'data': tx.data.hex(),
                    'v': tx.v,
                    'r': tx.r.hex(),
                    's': tx.s.hex(),
                }
                for tx in block.transactions
            ]
        }
        return json.dumps(block_dict).encode('utf-8')
    
    @staticmethod
    def _deserialize_block(data: bytes) -> Block:
        """Deserialize block from bytes"""
        block_dict = json.loads(data.decode('utf-8'))
        
        header = BlockHeader(
            version=block_dict['header']['version'],
            height=block_dict['header']['height'],
            timestamp=block_dict['header']['timestamp'],
            previous_hash=bytes.fromhex(block_dict['header']['previous_hash']),
            state_root=bytes.fromhex(block_dict['header']['state_root']),
            txs_root=bytes.fromhex(block_dict['header']['txs_root']),
            receipts_root=bytes.fromhex(block_dict['header']['receipts_root']),
            validator=bytes.fromhex(block_dict['header']['validator']),
            signature=bytes.fromhex(block_dict['header']['signature']),
            gas_used=block_dict['header']['gas_used'],
            gas_limit=block_dict['header']['gas_limit'],
        )
        
        transactions = [
            Transaction(
                nonce=tx_dict['nonce'],
                from_address=bytes.fromhex(tx_dict['from']),
                to_address=bytes.fromhex(tx_dict['to']) if tx_dict['to'] else None,
                value=tx_dict['value'],
                gas_price=tx_dict['gas_price'],
                gas_limit=tx_dict['gas_limit'],
                data=bytes.fromhex(tx_dict['data']),
                v=tx_dict['v'],
                r=bytes.fromhex(tx_dict['r']),
                s=bytes.fromhex(tx_dict['s']),
            )
            for tx_dict in block_dict['transactions']
        ]
        
        return Block(header=header, transactions=transactions)
    
    @staticmethod
    def _serialize_header(header: BlockHeader) -> bytes:
        """Serialize block header to bytes"""
        header_dict = {
            'version': header.version,
            'height': header.height,
            'timestamp': header.timestamp,
            'previous_hash': header.previous_hash.hex(),
            'state_root': header.state_root.hex(),
            'txs_root': header.txs_root.hex(),
            'receipts_root': header.receipts_root.hex(),
            'validator': header.validator.hex(),
            'signature': header.signature.hex(),
            'gas_used': header.gas_used,
            'gas_limit': header.gas_limit,
        }
        return json.dumps(header_dict).encode('utf-8')
    
    @staticmethod
    def _deserialize_header(data: bytes) -> BlockHeader:
        """Deserialize block header from bytes"""
        header_dict = json.loads(data.decode('utf-8'))
        
        return BlockHeader(
            version=header_dict['version'],
            height=header_dict['height'],
            timestamp=header_dict['timestamp'],
            previous_hash=bytes.fromhex(header_dict['previous_hash']),
            state_root=bytes.fromhex(header_dict['state_root']),
            txs_root=bytes.fromhex(header_dict['txs_root']),
            receipts_root=bytes.fromhex(header_dict['receipts_root']),
            validator=bytes.fromhex(header_dict['validator']),
            signature=bytes.fromhex(header_dict['signature']),
            gas_used=header_dict['gas_used'],
            gas_limit=header_dict['gas_limit'],
        )
    
    @staticmethod
    def _serialize_transaction(tx: Transaction, block_hash: bytes) -> bytes:
        """Serialize transaction with block reference"""
        tx_dict = {
            'block_hash': block_hash.hex(),
            'nonce': tx.nonce,
            'from': tx.from_address.hex(),
            'to': tx.to_address.hex() if tx.to_address else None,
            'value': tx.value,
            'gas_price': tx.gas_price,
            'gas_limit': tx.gas_limit,
            'data': tx.data.hex(),
            'v': tx.v,
            'r': tx.r.hex(),
            's': tx.s.hex(),
        }
        return json.dumps(tx_dict).encode('utf-8')
    
    @staticmethod
    def _deserialize_transaction(data: bytes) -> tuple:
        """Deserialize transaction from bytes"""
        tx_dict = json.loads(data.decode('utf-8'))
        
        block_hash = bytes.fromhex(tx_dict['block_hash'])
        
        tx = Transaction(
            nonce=tx_dict['nonce'],
            from_address=bytes.fromhex(tx_dict['from']),
            to_address=bytes.fromhex(tx_dict['to']) if tx_dict['to'] else None,
            value=tx_dict['value'],
            gas_price=tx_dict['gas_price'],
            gas_limit=tx_dict['gas_limit'],
            data=bytes.fromhex(tx_dict['data']),
            v=tx_dict['v'],
            r=bytes.fromhex(tx_dict['r']),
            s=bytes.fromhex(tx_dict['s']),
        )
        
        return (tx, block_hash)
    
    def __enter__(self):
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
