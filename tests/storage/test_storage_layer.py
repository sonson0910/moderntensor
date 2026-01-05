"""
Tests for Phase 4: Storage Layer

Tests for blockchain database, indexer, and persistent storage.
"""

import pytest
import tempfile
import shutil
import time
from pathlib import Path

# Try to import plyvel, skip tests if not available
try:
    import plyvel
    PLYVEL_AVAILABLE = True
except ImportError:
    PLYVEL_AVAILABLE = False

from sdk.storage.indexer import MemoryIndexer
from sdk.blockchain.block import Block, BlockHeader
from sdk.blockchain.transaction import Transaction

# Only import BlockchainDB if plyvel is available
if PLYVEL_AVAILABLE:
    from sdk.storage.blockchain_db import BlockchainDB, LevelDBWrapper


@pytest.mark.skipif(not PLYVEL_AVAILABLE, reason="plyvel not installed")
class TestLevelDBWrapper:
    """Test LevelDB wrapper"""
    
    @pytest.fixture
    def temp_db(self):
        """Create temporary database"""
        temp_dir = tempfile.mkdtemp()
        db = LevelDBWrapper(temp_dir)
        yield db
        db.close()
        shutil.rmtree(temp_dir)
    
    def test_put_get(self, temp_db):
        """Test basic put and get operations"""
        # Put data
        temp_db.put(b'key1', b'value1')
        temp_db.put(b'key2', b'value2')
        
        # Get data
        assert temp_db.get(b'key1') == b'value1'
        assert temp_db.get(b'key2') == b'value2'
        assert temp_db.get(b'key3') is None
    
    def test_delete(self, temp_db):
        """Test delete operation"""
        # Put and delete
        temp_db.put(b'key1', b'value1')
        assert temp_db.get(b'key1') == b'value1'
        
        temp_db.delete(b'key1')
        assert temp_db.get(b'key1') is None
    
    def test_batch_write(self, temp_db):
        """Test batch write operations"""
        operations = [
            ('put', b'key1', b'value1'),
            ('put', b'key2', b'value2'),
            ('put', b'key3', b'value3'),
        ]
        
        temp_db.batch_write(operations)
        
        assert temp_db.get(b'key1') == b'value1'
        assert temp_db.get(b'key2') == b'value2'
        assert temp_db.get(b'key3') == b'value3'


@pytest.mark.skipif(not PLYVEL_AVAILABLE, reason="plyvel not installed")
class TestBlockchainDB:
    """Test blockchain database"""
    
    @pytest.fixture
    def temp_blockchain_db(self):
        """Create temporary blockchain database"""
        temp_dir = tempfile.mkdtemp()
        db = BlockchainDB(temp_dir)
        yield db
        db.close()
        shutil.rmtree(temp_dir)
    
    @pytest.fixture
    def sample_block(self):
        """Create a sample block"""
        header = BlockHeader(
            version=1,
            height=1,
            timestamp=int(time.time()),
            previous_hash=b'\x00' * 32,
            state_root=b'\x00' * 32,
            txs_root=b'\x00' * 32,
            receipts_root=b'\x00' * 32,
            validator=b'\x00' * 32,
            signature=b'\x00' * 64,
            gas_used=21000,
            gas_limit=10000000
        )
        
        tx = Transaction(
            nonce=0,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=20,
            gas_limit=21000,
            data=b'',
            v=27,
            r=b'\x00' * 32,
            s=b'\x00' * 32
        )
        
        return Block(header=header, transactions=[tx])
    
    def test_store_and_get_block(self, temp_blockchain_db, sample_block):
        """Test storing and retrieving blocks"""
        # Store block
        temp_blockchain_db.store_block(sample_block)
        
        # Get by hash
        block_hash = sample_block.hash()
        retrieved = temp_blockchain_db.get_block(block_hash)
        
        assert retrieved is not None
        assert retrieved.header.height == sample_block.header.height
        assert retrieved.header.timestamp == sample_block.header.timestamp
        assert len(retrieved.transactions) == len(sample_block.transactions)
    
    def test_get_block_by_height(self, temp_blockchain_db, sample_block):
        """Test retrieving block by height"""
        # Store block
        temp_blockchain_db.store_block(sample_block)
        
        # Get by height
        retrieved = temp_blockchain_db.get_block_by_height(1)
        
        assert retrieved is not None
        assert retrieved.header.height == 1
    
    def test_get_block_header(self, temp_blockchain_db, sample_block):
        """Test retrieving block header"""
        # Store block
        temp_blockchain_db.store_block(sample_block)
        
        # Get header
        block_hash = sample_block.hash()
        header = temp_blockchain_db.get_block_header(block_hash)
        
        assert header is not None
        assert header.height == sample_block.header.height
        assert header.timestamp == sample_block.header.timestamp
    
    def test_store_and_get_transaction(self, temp_blockchain_db):
        """Test storing and retrieving transactions"""
        tx = Transaction(
            nonce=0,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=20,
            gas_limit=21000,
            data=b'test',
            v=27,
            r=b'\x00' * 32,
            s=b'\x00' * 32
        )
        
        block_hash = b'\xff' * 32
        
        # Store transaction
        temp_blockchain_db.store_transaction(tx, block_hash)
        
        # Get transaction
        tx_hash = tx.hash()
        result = temp_blockchain_db.get_transaction(tx_hash)
        
        assert result is not None
        retrieved_tx, retrieved_block_hash = result
        assert retrieved_tx.nonce == tx.nonce
        assert retrieved_tx.value == tx.value
        assert retrieved_block_hash == block_hash
    
    def test_best_height_and_hash(self, temp_blockchain_db, sample_block):
        """Test best height and hash tracking"""
        # Initially should be 0
        assert temp_blockchain_db.get_best_height() == 0
        
        # Store block
        temp_blockchain_db.store_block(sample_block)
        
        # Should be updated
        assert temp_blockchain_db.get_best_height() == 1
        assert temp_blockchain_db.get_best_hash() == sample_block.hash()
    
    def test_get_statistics(self, temp_blockchain_db, sample_block):
        """Test getting database statistics"""
        # Store block
        temp_blockchain_db.store_block(sample_block)
        temp_blockchain_db.set_genesis_hash(b'\xaa' * 32)
        
        # Get statistics
        stats = temp_blockchain_db.get_statistics()
        
        assert stats['best_height'] == 1
        assert stats['best_hash'] is not None
        assert stats['total_transactions'] == 1


class TestMemoryIndexer:
    """Test memory indexer functionality"""
    
    @pytest.fixture
    def memory_indexer(self):
        """Create memory indexer"""
        return MemoryIndexer()
    
    @pytest.fixture
    def sample_block_with_txs(self):
        """Create a sample block with transactions"""
        header = BlockHeader(
            version=1,
            height=1,
            timestamp=int(time.time()),
            previous_hash=b'\x00' * 32,
            state_root=b'\x00' * 32,
            txs_root=b'\x00' * 32,
            receipts_root=b'\x00' * 32,
            validator=b'\x00' * 32,
            signature=b'\x00' * 64,
            gas_used=42000,
            gas_limit=10000000
        )
        
        tx1 = Transaction(
            nonce=0,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=20,
            gas_limit=21000,
            data=b'',
            v=27,
            r=b'\x00' * 32,
            s=b'\x00' * 32
        )
        
        tx2 = Transaction(
            nonce=1,
            from_address=b'\x01' * 20,
            to_address=b'\x03' * 20,
            value=500,
            gas_price=20,
            gas_limit=21000,
            data=b'',
            v=27,
            r=b'\x00' * 32,
            s=b'\x00' * 32
        )
        
        return Block(header=header, transactions=[tx1, tx2])
    
    def test_index_block(self, memory_indexer, sample_block_with_txs):
        """Test indexing a block"""
        # Index block
        memory_indexer.index_block(sample_block_with_txs)
        
        # Check transaction counts
        from_addr = b'\x01' * 20
        assert memory_indexer.get_transaction_count(from_addr) == 2
        
        to_addr1 = b'\x02' * 20
        assert memory_indexer.get_transaction_count(to_addr1) == 1
        
        to_addr2 = b'\x03' * 20
        assert memory_indexer.get_transaction_count(to_addr2) == 1
    
    def test_get_transactions_by_address(self, memory_indexer, sample_block_with_txs):
        """Test getting transactions by address"""
        # Index block
        memory_indexer.index_block(sample_block_with_txs)
        
        # Get transactions for sender
        from_addr = b'\x01' * 20
        tx_hashes = memory_indexer.get_transactions_by_address(from_addr)
        
        assert len(tx_hashes) == 2
    
    def test_balance_tracking(self, memory_indexer):
        """Test balance tracking"""
        address = b'\x01' * 20
        
        # Initially 0
        assert memory_indexer.get_balance(address) == 0
        
        # Update balance
        memory_indexer.update_balance(address, 1000)
        
        # Check balance
        assert memory_indexer.get_balance(address) == 1000
    
    def test_nonce_tracking(self, memory_indexer):
        """Test nonce tracking"""
        address = b'\x01' * 20
        
        # Initially 0
        assert memory_indexer.get_nonce(address) == 0
        
        # Update nonce
        memory_indexer.update_nonce(address, 5)
        
        # Check nonce
        assert memory_indexer.get_nonce(address) == 5
    
    def test_get_address_summary(self, memory_indexer, sample_block_with_txs):
        """Test getting address summary"""
        # Setup
        address = b'\x01' * 20
        memory_indexer.index_block(sample_block_with_txs)
        memory_indexer.update_balance(address, 5000)
        memory_indexer.update_nonce(address, 2)
        
        # Get summary
        summary = memory_indexer.get_address_summary(address)
        
        assert summary['balance'] == 5000
        assert summary['nonce'] == 2
        assert summary['transaction_count'] == 2


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
