"""
Tests for JSON-RPC and GraphQL APIs.
"""
import pytest
import tempfile
import shutil
import time
from pathlib import Path

from sdk.blockchain.block import Block, BlockHeader
from sdk.blockchain.transaction import Transaction
from sdk.blockchain.state import StateDB
from sdk.blockchain.crypto import KeyPair
from sdk.storage.blockchain_db import BlockchainDB
from sdk.storage.indexer import MemoryIndexer
from sdk.blockchain.validation import ChainConfig
from sdk.api.rpc import JSONRPC, JSONRPCRequest
from sdk.api.graphql_api import GraphQLAPI, STRAWBERRY_AVAILABLE


@pytest.fixture
def temp_db_dir():
    """Create temporary database directory"""
    temp_dir = tempfile.mkdtemp()
    yield temp_dir
    shutil.rmtree(temp_dir, ignore_errors=True)


@pytest.fixture
def blockchain_db(temp_db_dir):
    """Create blockchain database"""
    try:
        db = BlockchainDB(temp_db_dir)
        yield db
        db.close()
    except ImportError:
        # LevelDB not available, skip
        pytest.skip("LevelDB not available")


@pytest.fixture
def state_db():
    """Create state database"""
    return StateDB()


@pytest.fixture
def sample_blocks(blockchain_db, state_db):
    """Create sample blocks for testing"""
    blocks = []
    
    # Genesis block
    genesis = Block.create_genesis(chain_id=1)
    blockchain_db.store_block(genesis)
    blocks.append(genesis)
    
    # Create a few more blocks manually
    for i in range(1, 5):
        # Create transactions
        keypair = KeyPair()
        tx = Transaction(
            nonce=0,
            from_address=keypair.address(),
            to_address=b'\x02' * 20,
            value=1000 * i,
            gas_price=1,
            gas_limit=21000,
        )
        tx.sign(keypair.private_key)
        
        # Create block header
        header = BlockHeader(
            version=1,
            height=i,
            timestamp=int(time.time()),
            previous_hash=blocks[-1].hash(),
            state_root=state_db.get_state_root(),
            txs_root=b'\x00' * 32,  # Simplified
            receipts_root=b'\x00' * 32,
            validator=keypair.public_key,
            signature=b'\x00' * 64,  # Simplified
            gas_used=21000 * len([tx]),
            gas_limit=1000000,
        )
        
        # Create block
        block = Block(header=header, transactions=[tx])
        
        blockchain_db.store_block(block)
        blocks.append(block)
    
    return blocks


class TestJSONRPC:
    """Test JSON-RPC API"""
    
    @pytest.fixture
    def rpc_server(self, blockchain_db, state_db):
        """Create JSON-RPC server"""
        indexer = MemoryIndexer()
        config = ChainConfig(chain_id=1)
        return JSONRPC(blockchain_db, state_db, indexer, chain_config=config)
    
    @pytest.mark.asyncio
    async def test_eth_blockNumber(self, rpc_server, sample_blocks):
        """Test eth_blockNumber method"""
        block_number = await rpc_server.eth_blockNumber()
        assert block_number == 4  # Genesis + 4 blocks
    
    @pytest.mark.asyncio
    async def test_eth_chainId(self, rpc_server):
        """Test eth_chainId method"""
        chain_id = await rpc_server.eth_chainId()
        assert chain_id == 1
    
    @pytest.mark.asyncio
    async def test_eth_getBlockByNumber(self, rpc_server, sample_blocks):
        """Test eth_getBlockByNumber method"""
        # Get genesis block
        block = await rpc_server.eth_getBlockByNumber(0, full_transactions=False)
        
        assert block is not None
        assert block["number"] == hex(0)
        # Hash will vary, so just check it's a valid hex string
        assert block["hash"].startswith("0x")
        assert len(block["hash"]) == 66  # 0x + 64 hex chars
        assert isinstance(block["transactions"], list)
    
    @pytest.mark.asyncio
    async def test_eth_getBlockByNumber_latest(self, rpc_server, sample_blocks):
        """Test eth_getBlockByNumber with 'latest'"""
        block = await rpc_server.eth_getBlockByNumber("latest", full_transactions=False)
        
        assert block is not None
        assert block["number"] == hex(4)
    
    @pytest.mark.asyncio
    async def test_eth_getBlockByHash(self, rpc_server, sample_blocks):
        """Test eth_getBlockByHash method"""
        block_hash = "0x" + sample_blocks[1].hash().hex()
        block = await rpc_server.eth_getBlockByHash(block_hash, full_transactions=False)
        
        assert block is not None
        assert block["hash"] == block_hash
        assert block["number"] == hex(1)
    
    @pytest.mark.asyncio
    async def test_eth_getBlockByNumber_full_transactions(
        self, rpc_server, sample_blocks
    ):
        """Test eth_getBlockByNumber with full transactions"""
        block = await rpc_server.eth_getBlockByNumber(1, full_transactions=True)
        
        assert block is not None
        assert len(block["transactions"]) == 1
        assert isinstance(block["transactions"][0], dict)
        assert "hash" in block["transactions"][0]
        assert "from" in block["transactions"][0]
    
    @pytest.mark.asyncio
    async def test_eth_getBalance(self, rpc_server, state_db):
        """Test eth_getBalance method"""
        # Create account with balance
        address = b'\x01' * 20
        state_db.add_balance(address, 1000000)
        
        balance = await rpc_server.eth_getBalance("0x" + address.hex())
        assert int(balance, 16) == 1000000
    
    @pytest.mark.asyncio
    async def test_eth_getTransactionCount(self, rpc_server, state_db):
        """Test eth_getTransactionCount method"""
        # Create account with nonce
        address = b'\x01' * 20
        state_db.set_nonce(address, 2)
        
        nonce = await rpc_server.eth_getTransactionCount("0x" + address.hex())
        assert int(nonce, 16) == 2
    
    @pytest.mark.asyncio
    async def test_eth_sendRawTransaction(self, rpc_server):
        """Test eth_sendRawTransaction method"""
        # Create and sign transaction
        keypair = KeyPair()
        tx = Transaction(
            nonce=0,
            from_address=keypair.address(),
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=1,
            gas_limit=21000,
        )
        tx.sign(keypair.private_key)
        
        # Serialize and send
        tx_hex = "0x" + tx.serialize().hex()
        tx_hash = await rpc_server.eth_sendRawTransaction(tx_hex)
        
        assert tx_hash.startswith("0x")
        assert len(tx_hash) == 66  # 0x + 64 hex chars
    
    @pytest.mark.asyncio
    async def test_eth_getTransactionByHash_pending(self, rpc_server):
        """Test eth_getTransactionByHash for pending transaction"""
        # Create and send transaction
        keypair = KeyPair()
        tx = Transaction(
            nonce=0,
            from_address=keypair.address(),
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=1,
            gas_limit=21000,
        )
        tx.sign(keypair.private_key)
        
        tx_hex = "0x" + tx.serialize().hex()
        tx_hash = await rpc_server.eth_sendRawTransaction(tx_hex)
        
        # Get transaction
        retrieved_tx = await rpc_server.eth_getTransactionByHash(tx_hash)
        
        assert retrieved_tx is not None
        assert retrieved_tx["hash"] == tx_hash
        assert retrieved_tx["value"] == hex(1000)
    
    @pytest.mark.asyncio
    async def test_eth_estimateGas(self, rpc_server):
        """Test eth_estimateGas method"""
        tx_params = {
            "from": "0x" + (b'\x01' * 20).hex(),
            "to": "0x" + (b'\x02' * 20).hex(),
            "value": hex(1000),
        }
        
        gas_estimate = await rpc_server.eth_estimateGas(tx_params)
        assert int(gas_estimate, 16) >= 21000  # Minimum gas
    
    @pytest.mark.asyncio
    async def test_eth_gasPrice(self, rpc_server):
        """Test eth_gasPrice method"""
        gas_price = await rpc_server.eth_gasPrice()
        assert int(gas_price, 16) > 0
    
    @pytest.mark.asyncio
    async def test_mt_submitAITask(self, rpc_server):
        """Test mt_submitAITask method"""
        task_params = {
            "model_hash": "0x1234567890abcdef",
            "input_data": "test_input",
            "requester": "0x" + (b'\x01' * 20).hex(),
            "reward": 1000,
        }
        
        task_id = await rpc_server.mt_submitAITask(task_params)
        
        assert task_id is not None
        assert len(task_id) == 64  # SHA256 hash
    
    @pytest.mark.asyncio
    async def test_mt_getAIResult(self, rpc_server):
        """Test mt_getAIResult method"""
        # Submit task
        task_params = {
            "model_hash": "0x1234567890abcdef",
            "input_data": "test_input",
            "requester": "0x" + (b'\x01' * 20).hex(),
            "reward": 1000,
        }
        task_id = await rpc_server.mt_submitAITask(task_params)
        
        # Get result
        result = await rpc_server.mt_getAIResult(task_id)
        
        assert result is not None
        assert result["task_id"] == task_id
        assert result["status"] == "pending"
    
    @pytest.mark.asyncio
    async def test_mt_listAITasks(self, rpc_server):
        """Test mt_listAITasks method"""
        # Submit multiple tasks
        for i in range(3):
            task_params = {
                "model_hash": f"0x{i:064x}",
                "input_data": f"test_input_{i}",
                "requester": "0x" + (b'\x01' * 20).hex(),
                "reward": 1000 * (i + 1),
            }
            await rpc_server.mt_submitAITask(task_params)
        
        # List tasks
        tasks = await rpc_server.mt_listAITasks()
        
        assert len(tasks) == 3
        assert all(t["status"] == "pending" for t in tasks)
    
    @pytest.mark.asyncio
    async def test_mt_listAITasks_with_filter(self, rpc_server):
        """Test mt_listAITasks with status filter"""
        # Submit tasks
        task_params = {
            "model_hash": "0x1234567890abcdef",
            "input_data": "test_input",
            "requester": "0x" + (b'\x01' * 20).hex(),
            "reward": 1000,
        }
        task_id = await rpc_server.mt_submitAITask(task_params)
        
        # Update task status
        rpc_server.ai_tasks[task_id]["status"] = "completed"
        
        # List completed tasks
        tasks = await rpc_server.mt_listAITasks(status="completed")
        
        assert len(tasks) == 1
        assert tasks[0]["status"] == "completed"
    
    @pytest.mark.asyncio
    async def test_mt_getValidatorInfo(self, rpc_server):
        """Test mt_getValidatorInfo method"""
        address = "0x" + (b'\x01' * 20).hex()
        validator_info = await rpc_server.mt_getValidatorInfo(address)
        
        assert validator_info is not None
        assert validator_info["address"] == address


@pytest.mark.skipif(not STRAWBERRY_AVAILABLE, reason="Strawberry GraphQL not installed")
class TestGraphQLAPI:
    """Test GraphQL API"""
    
    @pytest.fixture
    def graphql_api(self, blockchain_db, state_db):
        """Create GraphQL API"""
        indexer = MemoryIndexer()
        return GraphQLAPI(blockchain_db, state_db, indexer)
    
    def test_graphql_api_initialization(self, graphql_api):
        """Test GraphQL API initialization"""
        assert graphql_api.router is not None
        assert graphql_api.schema is not None
    
    @pytest.mark.asyncio
    async def test_chain_info_query(self, graphql_api, sample_blocks):
        """Test chain info query"""
        query = """
        query {
            chainInfo {
                chainId
                bestHeight
                bestHash
            }
        }
        """
        
        result = await graphql_api.schema.execute(query)
        
        assert result.errors is None
        assert result.data["chainInfo"]["bestHeight"] == 4
    
    @pytest.mark.asyncio
    async def test_block_by_height_query(self, graphql_api, sample_blocks):
        """Test block by height query"""
        query = """
        query {
            block(height: 0) {
                hash
                height
                transactionCount
            }
        }
        """
        
        result = await graphql_api.schema.execute(query)
        
        assert result.errors is None
        assert result.data["block"]["height"] == 0
        assert result.data["block"]["hash"].startswith("0x")
    
    @pytest.mark.asyncio
    async def test_block_by_hash_query(self, graphql_api, sample_blocks):
        """Test block by hash query"""
        block_hash = "0x" + sample_blocks[1].hash().hex()
        query = f"""
        query {{
            block(hash: "{block_hash}") {{
                hash
                height
                validator
            }}
        }}
        """
        
        result = await graphql_api.schema.execute(query)
        
        assert result.errors is None
        assert result.data["block"]["hash"] == block_hash
        assert result.data["block"]["height"] == 1
    
    @pytest.mark.asyncio
    async def test_blocks_range_query(self, graphql_api, sample_blocks):
        """Test blocks range query"""
        query = """
        query {
            blocks(fromHeight: 0, toHeight: 2) {
                hash
                height
            }
        }
        """
        
        result = await graphql_api.schema.execute(query)
        
        assert result.errors is None
        assert len(result.data["blocks"]) >= 2
    
    @pytest.mark.asyncio
    async def test_account_query(self, graphql_api, state_db):
        """Test account query"""
        # Create account
        address = b'\x01' * 20
        state_db.create_account(address)
        state_db.add_balance(address, 5000000)
        
        address_hex = "0x" + address.hex()
        query = f"""
        query {{
            account(address: "{address_hex}") {{
                address
                balance
                nonce
            }}
        }}
        """
        
        result = await graphql_api.schema.execute(query)
        
        assert result.errors is None
        assert result.data["account"]["address"] == address_hex
        assert int(result.data["account"]["balance"]) == 5000000


class TestAPIIntegration:
    """Integration tests for API layer"""
    
    @pytest.fixture
    def full_setup(self, blockchain_db, state_db):
        """Setup full API stack"""
        indexer = MemoryIndexer()
        config = ChainConfig(chain_id=1)
        
        rpc = JSONRPC(blockchain_db, state_db, indexer, chain_config=config)
        graphql = GraphQLAPI(blockchain_db, state_db, indexer)
        
        return rpc, graphql, blockchain_db, state_db
    
    @pytest.mark.asyncio
    async def test_end_to_end_transaction_flow(self, full_setup, sample_blocks):
        """Test complete transaction flow through API"""
        rpc, graphql, blockchain_db, state_db = full_setup
        
        # Create and send transaction
        keypair = KeyPair()
        tx = Transaction(
            nonce=0,
            from_address=keypair.address(),
            to_address=b'\x02' * 20,
            value=5000,
            gas_price=1,
            gas_limit=21000,
        )
        tx.sign(keypair.private_key)
        
        # Send via RPC
        tx_hex = "0x" + tx.serialize().hex()
        tx_hash = await rpc.eth_sendRawTransaction(tx_hex)
        
        # Verify transaction is in pool
        assert tx_hash.startswith("0x")
        
        # Get transaction via RPC
        retrieved_tx = await rpc.eth_getTransactionByHash(tx_hash)
        assert retrieved_tx is not None
        assert retrieved_tx["value"] == hex(5000)
    
    @pytest.mark.asyncio
    async def test_block_retrieval_consistency(self, full_setup, sample_blocks):
        """Test block retrieval consistency across APIs"""
        rpc, graphql, blockchain_db, state_db = full_setup
        
        # Get block via RPC
        rpc_block = await rpc.eth_getBlockByNumber(1, full_transactions=False)
        
        assert rpc_block is not None
        assert rpc_block["number"] == hex(1)
        
        # Verify block exists in database
        db_block = blockchain_db.get_block_by_height(1)
        assert db_block is not None
        assert "0x" + db_block.hash().hex() == rpc_block["hash"]
