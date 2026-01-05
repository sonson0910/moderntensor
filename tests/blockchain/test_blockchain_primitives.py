"""
Tests for core blockchain primitives.
"""
import pytest
from sdk.blockchain.block import Block, BlockHeader
from sdk.blockchain.transaction import Transaction, TransactionReceipt
from sdk.blockchain.state import StateDB, Account
from sdk.blockchain.crypto import KeyPair, MerkleTree
from sdk.blockchain.validation import BlockValidator, ChainConfig


class TestBlock:
    """Test block creation and validation."""
    
    def test_genesis_block_creation(self):
        """Test creating a genesis block."""
        genesis = Block.create_genesis(chain_id=1)
        
        assert genesis.header.height == 0
        assert genesis.header.previous_hash == b'\x00' * 32
        assert len(genesis.transactions) == 0
        assert genesis.validate_structure()
    
    def test_block_hash(self):
        """Test block hashing."""
        genesis = Block.create_genesis()
        hash1 = genesis.hash()
        hash2 = genesis.hash()
        
        # Hash should be deterministic
        assert hash1 == hash2
        assert len(hash1) == 32
    
    def test_block_serialization(self):
        """Test block serialization and deserialization."""
        genesis = Block.create_genesis()
        serialized = genesis.serialize()
        
        # Deserialize
        deserialized = Block.deserialize(serialized)
        
        assert deserialized.header.height == genesis.header.height
        assert deserialized.hash() == genesis.hash()


class TestTransaction:
    """Test transaction creation and validation."""
    
    def test_transaction_creation(self):
        """Test creating a basic transaction."""
        tx = Transaction(
            nonce=0,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=1,
            gas_limit=21000,
        )
        
        assert tx.nonce == 0
        assert tx.value == 1000
        assert not tx.is_contract_creation()
    
    def test_contract_creation_transaction(self):
        """Test contract creation transaction."""
        tx = Transaction(
            nonce=0,
            from_address=b'\x01' * 20,
            to_address=None,  # None means contract creation
            value=0,
            gas_price=1,
            gas_limit=100000,
            data=b'contract_bytecode',
        )
        
        assert tx.is_contract_creation()
    
    def test_transaction_hash(self):
        """Test transaction hashing."""
        tx = Transaction(
            nonce=0,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=1,
            gas_limit=21000,
        )
        
        hash1 = tx.hash()
        hash2 = tx.hash()
        
        assert hash1 == hash2
        assert len(hash1) == 32
    
    def test_intrinsic_gas(self):
        """Test intrinsic gas calculation."""
        tx = Transaction(
            nonce=0,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=1,
            gas_limit=21000,
            data=b'\x00' * 10 + b'\x01' * 10,  # 10 zero bytes + 10 non-zero bytes
        )
        
        intrinsic = tx.intrinsic_gas()
        # Base: 21000 + zero bytes: 10*4 + non-zero bytes: 10*16
        assert intrinsic == 21000 + 40 + 160


class TestState:
    """Test state management."""
    
    def test_account_creation(self):
        """Test creating an account."""
        account = Account(nonce=0, balance=1000)
        
        assert account.nonce == 0
        assert account.balance == 1000
        assert not account.is_empty()
    
    def test_empty_account(self):
        """Test empty account detection."""
        account = Account()
        assert account.is_empty()
    
    def test_state_db_basic_operations(self):
        """Test basic state database operations."""
        state = StateDB()
        address = b'\x01' * 20
        
        # Get non-existent account
        account = state.get_account(address)
        assert account.is_empty()
        
        # Add balance
        state.add_balance(address, 1000)
        assert state.get_balance(address) == 1000
        
        # Subtract balance
        success = state.sub_balance(address, 500)
        assert success
        assert state.get_balance(address) == 500
        
        # Commit changes
        state.commit()
        assert state.get_balance(address) == 500
    
    def test_nonce_management(self):
        """Test nonce management."""
        state = StateDB()
        address = b'\x01' * 20
        
        assert state.get_nonce(address) == 0
        
        state.increment_nonce(address)
        assert state.get_nonce(address) == 1
        
        state.set_nonce(address, 5)
        assert state.get_nonce(address) == 5
    
    def test_transfer(self):
        """Test balance transfer between accounts."""
        state = StateDB()
        from_addr = b'\x01' * 20
        to_addr = b'\x02' * 20
        
        # Add initial balance
        state.add_balance(from_addr, 1000)
        
        # Transfer
        success = state.transfer(from_addr, to_addr, 600)
        assert success
        assert state.get_balance(from_addr) == 400
        assert state.get_balance(to_addr) == 600
        
        # Insufficient balance
        success = state.transfer(from_addr, to_addr, 500)
        assert not success


class TestCrypto:
    """Test cryptography primitives."""
    
    def test_keypair_generation(self):
        """Test key pair generation."""
        keypair = KeyPair()
        
        assert len(keypair.private_key) == 32
        assert len(keypair.public_key) == 64
        assert len(keypair.address()) == 20
    
    def test_keypair_from_private_key(self):
        """Test creating keypair from existing private key."""
        private_key = b'\x01' * 32
        keypair = KeyPair(private_key)
        
        assert keypair.private_key == private_key
        
        # Should derive same public key consistently
        keypair2 = KeyPair(private_key)
        assert keypair.public_key == keypair2.public_key
        assert keypair.address() == keypair2.address()
    
    def test_merkle_tree_empty(self):
        """Test Merkle tree with empty leaves."""
        tree = MerkleTree([])
        root = tree.root()
        
        assert len(root) == 32
    
    def test_merkle_tree_single_leaf(self):
        """Test Merkle tree with single leaf."""
        leaf = b'\x01' * 32
        tree = MerkleTree([leaf])
        
        assert tree.root() == leaf
    
    def test_merkle_tree_multiple_leaves(self):
        """Test Merkle tree with multiple leaves."""
        leaves = [b'\x01' * 32, b'\x02' * 32, b'\x03' * 32, b'\x04' * 32]
        tree = MerkleTree(leaves)
        
        root = tree.root()
        assert len(root) == 32
        
        # Test proof for first leaf
        proof = tree.get_proof(0)
        assert MerkleTree.verify_proof(leaves[0], proof, root)
        
        # Test proof for last leaf
        proof = tree.get_proof(3)
        assert MerkleTree.verify_proof(leaves[3], proof, root)


class TestValidation:
    """Test block and transaction validation."""
    
    def test_genesis_block_validation(self):
        """Test validating genesis block."""
        state = StateDB()
        config = ChainConfig()
        validator = BlockValidator(state, config)
        
        genesis = Block.create_genesis()
        assert validator.validate_block(genesis, parent_block=None)
    
    def test_transaction_validation_basic(self):
        """Test basic transaction validation."""
        state = StateDB()
        config = ChainConfig()
        validator = BlockValidator(state, config)
        
        # Create account with balance
        from_addr = b'\x01' * 20
        state.add_balance(from_addr, 100000)
        state.commit()
        
        # Create valid transaction
        tx = Transaction(
            nonce=0,
            from_address=from_addr,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=1,
            gas_limit=21000,
        )
        
        # Note: Signature validation is placeholder, so this will pass
        assert validator.validate_transaction(tx)
    
    def test_transaction_execution(self):
        """Test transaction execution."""
        state = StateDB()
        config = ChainConfig()
        validator = BlockValidator(state, config)
        
        # Setup accounts
        from_addr = b'\x01' * 20
        to_addr = b'\x02' * 20
        state.add_balance(from_addr, 100000)
        state.commit()
        
        # Create and execute transaction
        tx = Transaction(
            nonce=0,
            from_address=from_addr,
            to_address=to_addr,
            value=1000,
            gas_price=1,
            gas_limit=21000,
        )
        
        receipt = validator.execute_transaction(
            tx,
            block_height=1,
            block_hash=b'\x00' * 32,
            tx_index=0,
        )
        
        assert receipt.status == 1  # Success
        assert receipt.gas_used == tx.intrinsic_gas()
        
        # Check balances after commit
        state.commit()
        # From address should have: 100000 - 1000 (value) - gas_used * gas_price
        expected_from_balance = 100000 - 1000 - receipt.gas_used
        assert state.get_balance(from_addr) == expected_from_balance
        assert state.get_balance(to_addr) == 1000


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
