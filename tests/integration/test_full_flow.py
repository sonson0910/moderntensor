"""
End-to-end integration tests for ModernTensor blockchain.

These tests verify complete workflows including transaction submission,
block production, state updates, and network synchronization.
"""
import pytest
import asyncio
import tempfile
import shutil
from pathlib import Path

from sdk.blockchain.block import Block, BlockHeader
from sdk.blockchain.transaction import Transaction, TransactionReceipt
from sdk.blockchain.state import StateDB
from sdk.blockchain.crypto import KeyPair
from sdk.blockchain.validation import BlockValidator, ChainConfig
from sdk.storage.blockchain_db import BlockchainDB
from sdk.storage.indexer import MemoryIndexer
from sdk.api.rpc import JSONRPC


@pytest.fixture
def temp_dir():
    """Create temporary directory"""
    temp_dir = tempfile.mkdtemp()
    yield temp_dir
    shutil.rmtree(temp_dir, ignore_errors=True)


@pytest.fixture
def blockchain_components(temp_dir):
    """Setup complete blockchain components"""
    try:
        # Initialize storage
        blockchain_db = BlockchainDB(temp_dir)
        state_db = StateDB()
        indexer = MemoryIndexer()
        
        # Initialize config
        config = ChainConfig(chain_id=1)
        
        # Initialize validator
        validator = BlockValidator(state_db, config)
        
        # Initialize API
        rpc = JSONRPC(blockchain_db, state_db, indexer, validator, config)
        
        # Create genesis block
        genesis = Block.create_genesis(chain_id=1)
        blockchain_db.store_block(genesis)
        
        yield {
            'blockchain_db': blockchain_db,
            'state_db': state_db,
            'indexer': indexer,
            'validator': validator,
            'rpc': rpc,
            'config': config,
        }
        
        blockchain_db.close()
    except ImportError:
        pytest.skip("LevelDB not available")


class TestEndToEndTransactionFlow:
    """Test complete transaction lifecycle"""
    
    @pytest.mark.asyncio
    async def test_transaction_submission_and_execution(self, blockchain_components):
        """Test full transaction flow from submission to execution"""
        components = blockchain_components
        state_db = components['state_db']
        rpc = components['rpc']
        
        # Create sender and receiver accounts
        sender = KeyPair()
        receiver_address = b'\x02' * 20
        
        # Fund sender account
        initial_balance = 1000000
        state_db.add_balance(sender.address(), initial_balance)
        
        # Create and sign transaction
        tx = Transaction(
            nonce=0,
            from_address=sender.address(),
            to_address=receiver_address,
            value=50000,
            gas_price=1,
            gas_limit=21000,
        )
        tx.sign(sender.private_key)
        
        # Submit transaction via RPC
        tx_hex = "0x" + tx.serialize().hex()
        tx_hash = await rpc.eth_sendRawTransaction(tx_hex)
        
        assert tx_hash is not None
        assert tx_hash.startswith("0x")
        
        # Verify transaction is in pool
        retrieved_tx = await rpc.eth_getTransactionByHash(tx_hash)
        assert retrieved_tx is not None
        assert retrieved_tx["value"] == hex(50000)
        assert retrieved_tx["from"] == "0x" + sender.address().hex()
    
    @pytest.mark.asyncio
    async def test_block_production_workflow(self, blockchain_components):
        """Test block creation and validation workflow"""
        components = blockchain_components
        blockchain_db = components['blockchain_db']
        state_db = components['state_db']
        validator = components['validator']
        
        # Create validator keypair
        validator_kp = KeyPair()
        
        # Create some transactions
        transactions = []
        for i in range(3):
            sender = KeyPair()
            state_db.add_balance(sender.address(), 100000)
            
            tx = Transaction(
                nonce=0,
                from_address=sender.address(),
                to_address=b'\x03' * 20,
                value=1000 * (i + 1),
                gas_price=1,
                gas_limit=21000,
            )
            tx.sign(sender.private_key)
            transactions.append(tx)
        
        # Get previous block
        prev_block = blockchain_db.get_block_by_height(0)
        assert prev_block is not None
        
        # Create new block
        import time
        header = BlockHeader(
            version=1,
            height=1,
            timestamp=int(time.time()),
            previous_hash=prev_block.hash(),
            state_root=state_db.get_state_root(),
            txs_root=b'\x00' * 32,
            receipts_root=b'\x00' * 32,
            validator=validator_kp.public_key,
            signature=b'\x00' * 64,
            gas_used=21000 * len(transactions),
            gas_limit=1000000,
        )
        
        block = Block(header=header, transactions=transactions)
        
        # Validate block structure
        assert block.validate_structure()
        
        # Store block
        blockchain_db.store_block(block)
        
        # Verify block was stored
        retrieved_block = blockchain_db.get_block_by_height(1)
        assert retrieved_block is not None
        assert retrieved_block.header.height == 1
        assert len(retrieved_block.transactions) == 3
    
    @pytest.mark.asyncio
    async def test_state_consistency(self, blockchain_components):
        """Test state consistency across transactions"""
        components = blockchain_components
        state_db = components['state_db']
        
        # Create test account
        account_address = b'\x01' * 20
        initial_balance = 100000
        state_db.add_balance(account_address, initial_balance)
        
        # Verify initial state
        balance = state_db.get_balance(account_address)
        assert balance == initial_balance
        
        # Perform multiple balance operations
        state_db.sub_balance(account_address, 10000)
        balance = state_db.get_balance(account_address)
        assert balance == 90000
        
        state_db.add_balance(account_address, 5000)
        balance = state_db.get_balance(account_address)
        assert balance == 95000
        
        # Test nonce management
        nonce = state_db.get_nonce(account_address)
        assert nonce == 0
        
        state_db.increment_nonce(account_address)
        nonce = state_db.get_nonce(account_address)
        assert nonce == 1


class TestNetworkSync:
    """Test blockchain synchronization between nodes"""
    
    @pytest.mark.asyncio
    async def test_chain_sync_basic(self, blockchain_components):
        """Test basic blockchain sync workflow"""
        components = blockchain_components
        blockchain_db = components['blockchain_db']
        
        # Simulate second node with empty chain
        temp_dir2 = tempfile.mkdtemp()
        try:
            blockchain_db2 = BlockchainDB(temp_dir2)
            
            # Node 1 has blocks, Node 2 is empty
            node1_height = blockchain_db.get_best_height()
            node2_height = blockchain_db2.get_best_height()
            
            assert node1_height == 0  # Genesis
            assert node2_height == 0  # Also has genesis by default or 0 if empty
            
            # Simulate sync: copy blocks from node1 to node2
            genesis = blockchain_db.get_block_by_height(0)
            if genesis:
                blockchain_db2.store_block(genesis)
                
                # Verify sync
                synced_block = blockchain_db2.get_block_by_height(0)
                assert synced_block is not None
                assert synced_block.hash() == genesis.hash()
            
            blockchain_db2.close()
        finally:
            shutil.rmtree(temp_dir2, ignore_errors=True)
    
    @pytest.mark.asyncio
    async def test_block_propagation_simulation(self, blockchain_components):
        """Test block propagation between multiple nodes"""
        components = blockchain_components
        blockchain_db = components['blockchain_db']
        state_db = components['state_db']
        
        # Create a new block
        validator_kp = KeyPair()
        
        sender = KeyPair()
        state_db.add_balance(sender.address(), 100000)
        
        tx = Transaction(
            nonce=0,
            from_address=sender.address(),
            to_address=b'\x04' * 20,
            value=5000,
            gas_price=1,
            gas_limit=21000,
        )
        tx.sign(sender.private_key)
        
        # Create block
        import time
        prev_block = blockchain_db.get_block_by_height(0)
        
        header = BlockHeader(
            version=1,
            height=1,
            timestamp=int(time.time()),
            previous_hash=prev_block.hash(),
            state_root=state_db.get_state_root(),
            txs_root=b'\x00' * 32,
            receipts_root=b'\x00' * 32,
            validator=validator_kp.public_key,
            signature=b'\x00' * 64,
            gas_used=21000,
            gas_limit=1000000,
        )
        
        block = Block(header=header, transactions=[tx])
        
        # Simulate propagation to multiple nodes
        nodes = []
        for i in range(3):
            temp_dir = tempfile.mkdtemp()
            try:
                node_db = BlockchainDB(temp_dir)
                
                # Store genesis
                genesis = blockchain_db.get_block_by_height(0)
                if genesis:
                    node_db.store_block(genesis)
                
                # Propagate new block
                node_db.store_block(block)
                
                # Verify block received
                received_block = node_db.get_block_by_height(1)
                assert received_block is not None
                assert received_block.hash() == block.hash()
                
                nodes.append(node_db)
            except Exception as e:
                shutil.rmtree(temp_dir, ignore_errors=True)
                raise
        
        # Cleanup
        for node_db in nodes:
            temp_dir = node_db.data_dir
            node_db.close()
            shutil.rmtree(temp_dir, ignore_errors=True)


class TestAIValidationFlow:
    """Test AI task submission and validation workflow"""
    
    @pytest.mark.asyncio
    async def test_ai_task_submission(self, blockchain_components):
        """Test AI task submission through API"""
        components = blockchain_components
        rpc = components['rpc']
        
        # Submit AI task
        task_params = {
            "model_hash": "0x" + "a" * 64,
            "input_data": "test_image_data",
            "requester": "0x" + (b'\x01' * 20).hex(),
            "reward": 10000,
        }
        
        task_id = await rpc.mt_submitAITask(task_params)
        
        assert task_id is not None
        assert len(task_id) == 64
        
        # Retrieve task
        task = await rpc.mt_getAIResult(task_id)
        assert task is not None
        assert task["status"] == "pending"
        assert task["reward"] == 10000
    
    @pytest.mark.asyncio
    async def test_ai_task_lifecycle(self, blockchain_components):
        """Test complete AI task lifecycle"""
        components = blockchain_components
        rpc = components['rpc']
        
        # Submit task
        task_params = {
            "model_hash": "0x" + "b" * 64,
            "input_data": "inference_input",
            "requester": "0x" + (b'\x02' * 20).hex(),
            "reward": 5000,
        }
        
        task_id = await rpc.mt_submitAITask(task_params)
        
        # Simulate task processing (update status)
        rpc.ai_tasks[task_id]["status"] = "processing"
        rpc.ai_tasks[task_id]["worker"] = "0x" + (b'\x03' * 20).hex()
        
        # Check processing status
        task = await rpc.mt_getAIResult(task_id)
        assert task["status"] == "processing"
        
        # Complete task
        rpc.ai_tasks[task_id]["status"] = "completed"
        rpc.ai_tasks[task_id]["result"] = {"output": "inference_result"}
        
        # Verify completion
        task = await rpc.mt_getAIResult(task_id)
        assert task["status"] == "completed"
        assert "result" in task
    
    @pytest.mark.asyncio
    async def test_multiple_ai_tasks(self, blockchain_components):
        """Test handling multiple AI tasks concurrently"""
        components = blockchain_components
        rpc = components['rpc']
        
        # Submit multiple tasks
        task_ids = []
        for i in range(5):
            task_params = {
                "model_hash": f"0x{i:064x}",
                "input_data": f"input_{i}",
                "requester": "0x" + (b'\x04' * 20).hex(),
                "reward": 1000 * (i + 1),
            }
            task_id = await rpc.mt_submitAITask(task_params)
            task_ids.append(task_id)
        
        # Verify all tasks
        assert len(task_ids) == 5
        
        # List all tasks
        all_tasks = await rpc.mt_listAITasks()
        assert len(all_tasks) >= 5
        
        # List pending tasks
        pending_tasks = await rpc.mt_listAITasks(status="pending")
        assert len(pending_tasks) >= 5


class TestAPIIntegration:
    """Test API integration with blockchain components"""
    
    @pytest.mark.asyncio
    async def test_rpc_chain_queries(self, blockchain_components):
        """Test RPC chain query methods"""
        components = blockchain_components
        rpc = components['rpc']
        
        # Test eth_blockNumber
        block_number = await rpc.eth_blockNumber()
        assert block_number == 0  # Genesis block
        
        # Test eth_chainId
        chain_id = await rpc.eth_chainId()
        assert chain_id == 1
        
        # Test eth_gasPrice
        gas_price = await rpc.eth_gasPrice()
        assert int(gas_price, 16) > 0
    
    @pytest.mark.asyncio
    async def test_rpc_account_operations(self, blockchain_components):
        """Test RPC account operation methods"""
        components = blockchain_components
        rpc = components['rpc']
        state_db = components['state_db']
        
        # Create test account
        test_address = b'\x05' * 20
        test_balance = 999000
        
        state_db.add_balance(test_address, test_balance)
        state_db.set_nonce(test_address, 5)
        
        # Test eth_getBalance
        balance_hex = await rpc.eth_getBalance("0x" + test_address.hex())
        balance = int(balance_hex, 16)
        assert balance == test_balance
        
        # Test eth_getTransactionCount
        nonce_hex = await rpc.eth_getTransactionCount("0x" + test_address.hex())
        nonce = int(nonce_hex, 16)
        assert nonce == 5
