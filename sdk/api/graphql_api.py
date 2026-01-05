"""
GraphQL API for ModernTensor Layer 1 blockchain.

Provides flexible GraphQL queries for blockchain data.
"""

import logging
from typing import Optional, List, Any
from datetime import datetime

try:
    import strawberry
    from strawberry.fastapi import GraphQLRouter
    STRAWBERRY_AVAILABLE = True
except ImportError:
    STRAWBERRY_AVAILABLE = False
    strawberry = None
    GraphQLRouter = None

from sdk.blockchain.block import Block
from sdk.blockchain.transaction import Transaction
from sdk.storage.blockchain_db import BlockchainDB
from sdk.storage.indexer import Indexer, MemoryIndexer
from sdk.blockchain.state import StateDB

logger = logging.getLogger(__name__)


if STRAWBERRY_AVAILABLE:
    
    @strawberry.type
    class BlockType:
        """GraphQL Block type"""
        hash: str
        height: int
        timestamp: int
        parent_hash: str
        state_root: str
        transactions_root: str
        receipts_root: str
        validator: str
        gas_used: int
        gas_limit: int
        extra_data: str
        transaction_count: int
        
        @strawberry.field
        def transactions(self) -> List['TransactionType']:
            """Get transactions in this block"""
            # This would be populated by the resolver
            return []
    
    
    @strawberry.type
    class TransactionType:
        """GraphQL Transaction type"""
        hash: str
        nonce: int
        from_address: str
        to_address: Optional[str]
        value: str
        gas_price: int
        gas_limit: int
        data: str
        status: Optional[str]
        block_hash: Optional[str]
        block_number: Optional[int]
        
        @strawberry.field
        def block(self) -> Optional[BlockType]:
            """Get block containing this transaction"""
            return None
    
    
    @strawberry.type
    class AccountType:
        """GraphQL Account type"""
        address: str
        balance: str
        nonce: int
        code_hash: str
        storage_root: str
        is_contract: bool
        
        @strawberry.field
        def transactions(self, limit: int = 100) -> List[TransactionType]:
            """Get transactions for this account"""
            return []
    
    
    @strawberry.type
    class ValidatorType:
        """GraphQL Validator type"""
        address: str
        stake: str
        active: bool
        commission: str
        total_blocks: int
        
        @strawberry.field
        def blocks(self, limit: int = 100) -> List[BlockType]:
            """Get blocks produced by this validator"""
            return []
    
    
    @strawberry.type
    class AITaskType:
        """GraphQL AI Task type"""
        task_id: str
        model_hash: str
        input_data: str
        requester: str
        reward: int
        status: str
        submitted_at: str
        completed_at: Optional[str]
        result_data: Optional[str]
        worker: Optional[str]
    
    
    @strawberry.type
    class ChainInfoType:
        """GraphQL Chain Info type"""
        chain_id: int
        best_height: int
        best_hash: str
        total_transactions: int
        total_validators: int
        genesis_hash: str


class GraphQLAPI:
    """
    GraphQL API for ModernTensor blockchain.
    
    Provides flexible queries for blockchain data with relationship traversal.
    """
    
    def __init__(
        self,
        blockchain_db: BlockchainDB,
        state_db: StateDB,
        indexer: Optional[Indexer] = None,
    ):
        """
        Initialize GraphQL API.
        
        Args:
            blockchain_db: Blockchain database instance
            state_db: State database instance
            indexer: Indexer for fast queries
        """
        if not STRAWBERRY_AVAILABLE:
            logger.warning(
                "Strawberry GraphQL not installed. "
                "Install with: pip install 'strawberry-graphql[fastapi]'"
            )
            self.router = None
            return
        
        self.blockchain_db = blockchain_db
        self.state_db = state_db
        self.indexer = indexer or MemoryIndexer()
        
        # Create schema with resolvers
        self.schema = self._create_schema()
        
        # Create GraphQL router for FastAPI
        self.router = GraphQLRouter(self.schema)
        
        logger.info("GraphQL API initialized")
    
    def _create_schema(self):
        """Create GraphQL schema with resolvers"""
        
        @strawberry.type
        class Query:
            """Root Query type"""
            
            @strawberry.field
            def block(
                self,
                hash: Optional[str] = None,
                height: Optional[int] = None
            ) -> Optional[BlockType]:
                """
                Get block by hash or height.
                
                Args:
                    hash: Block hash (hex string)
                    height: Block height
                    
                Returns:
                    Block or None if not found
                """
                if hash:
                    block_hash_bytes = bytes.fromhex(hash.removeprefix("0x"))
                    block = self.blockchain_db.get_block(block_hash_bytes)
                elif height is not None:
                    block = self.blockchain_db.get_block_by_height(height)
                else:
                    return None
                
                if block is None:
                    return None
                
                return self._format_block(block)
            
            @strawberry.field
            def blocks(
                self,
                from_height: int = 0,
                to_height: Optional[int] = None,
                limit: int = 100
            ) -> List[BlockType]:
                """
                Get range of blocks.
                
                Args:
                    from_height: Starting height
                    to_height: Ending height (optional)
                    limit: Maximum number of blocks
                    
                Returns:
                    List of blocks
                """
                if to_height is None:
                    to_height = from_height + limit
                
                blocks = self.blockchain_db.get_blocks_range(from_height, to_height)
                return [self._format_block(b) for b in blocks[:limit]]
            
            @strawberry.field
            def transaction(self, hash: str) -> Optional[TransactionType]:
                """
                Get transaction by hash.
                
                Args:
                    hash: Transaction hash (hex string)
                    
                Returns:
                    Transaction or None if not found
                """
                tx_hash_bytes = bytes.fromhex(hash.removeprefix("0x"))
                result = self.blockchain_db.get_transaction(tx_hash_bytes)
                
                if result is None:
                    return None
                
                block_hash, tx = result
                return self._format_transaction(tx, block_hash)
            
            @strawberry.field
            def account(self, address: str) -> Optional[AccountType]:
                """
                Get account information.
                
                Args:
                    address: Account address (hex string)
                    
                Returns:
                    Account or None if not found
                """
                address_bytes = bytes.fromhex(address.removeprefix("0x"))
                account = self.state_db.get_account(address_bytes)
                
                if account is None:
                    return None
                
                return AccountType(
                    address="0x" + address_bytes.hex(),
                    balance=str(account.balance),
                    nonce=account.nonce,
                    code_hash="0x" + account.code_hash.hex(),
                    storage_root="0x" + account.storage_root.hex(),
                    is_contract=account.code_hash != b'\x00' * 32,
                )
            
            @strawberry.field
            def validator(self, address: str) -> Optional[ValidatorType]:
                """
                Get validator information.
                
                Args:
                    address: Validator address (hex string)
                    
                Returns:
                    Validator or None if not found
                """
                # This would query the consensus layer
                # For now, return placeholder
                return ValidatorType(
                    address=address,
                    stake="0",
                    active=False,
                    commission="0",
                    total_blocks=0,
                )
            
            @strawberry.field
            def ai_task(self, task_id: str) -> Optional[AITaskType]:
                """
                Get AI task by ID.
                
                Args:
                    task_id: Task ID
                    
                Returns:
                    AI task or None if not found
                """
                # This would query the AI task storage
                return None
            
            @strawberry.field
            def chain_info(self) -> ChainInfoType:
                """
                Get chain information.
                
                Returns:
                    Chain info
                """
                best_height = self.blockchain_db.get_best_height() or 0
                best_hash = self.blockchain_db.get_best_hash()
                genesis_hash = self.blockchain_db.get_genesis_hash()
                
                return ChainInfoType(
                    chain_id=1,  # Would come from config
                    best_height=best_height,
                    best_hash="0x" + best_hash.hex() if best_hash else "0x00",
                    total_transactions=0,
                    total_validators=0,
                    genesis_hash="0x" + genesis_hash.hex() if genesis_hash else "0x00",
                )
        
        # Bind resolvers to instance
        Query.block = lambda self_inner, **kwargs: Query.block(self, **kwargs)
        Query.blocks = lambda self_inner, **kwargs: Query.blocks(self, **kwargs)
        Query.transaction = lambda self_inner, **kwargs: Query.transaction(self, **kwargs)
        Query.account = lambda self_inner, **kwargs: Query.account(self, **kwargs)
        Query.validator = lambda self_inner, **kwargs: Query.validator(self, **kwargs)
        Query.ai_task = lambda self_inner, **kwargs: Query.ai_task(self, **kwargs)
        Query.chain_info = lambda self_inner, **kwargs: Query.chain_info(self, **kwargs)
        
        return strawberry.Schema(query=Query)
    
    def _format_block(self, block: Block) -> 'BlockType':
        """Format block for GraphQL response"""
        return BlockType(
            hash="0x" + block.hash().hex(),
            height=block.header.height,
            timestamp=block.header.timestamp,
            parent_hash="0x" + block.header.previous_hash.hex(),
            state_root="0x" + block.header.state_root.hex(),
            transactions_root="0x" + block.header.txs_root.hex(),
            receipts_root="0x" + block.header.receipts_root.hex(),
            validator="0x" + block.header.validator.hex(),
            gas_used=block.header.gas_used,
            gas_limit=block.header.gas_limit,
            extra_data="0x" + block.header.extra_data.hex(),
            transaction_count=len(block.transactions),
        )
    
    def _format_transaction(
        self, 
        tx: Transaction, 
        block_hash: Optional[bytes] = None
    ) -> 'TransactionType':
        """Format transaction for GraphQL response"""
        return TransactionType(
            hash="0x" + tx.hash().hex(),
            nonce=tx.nonce,
            from_address="0x" + tx.from_address.hex(),
            to_address="0x" + tx.to_address.hex() if tx.to_address else None,
            value=str(tx.value),
            gas_price=tx.gas_price,
            gas_limit=tx.gas_limit,
            data="0x" + tx.data.hex(),
            status=None,
            block_hash="0x" + block_hash.hex() if block_hash else None,
            block_number=None,
        )


# Fallback class when strawberry is not available
if not STRAWBERRY_AVAILABLE:
    class GraphQLAPI:
        """Fallback GraphQL API when strawberry is not installed"""
        
        def __init__(self, *args, **kwargs):
            logger.warning(
                "Strawberry GraphQL not installed. GraphQL API is disabled. "
                "Install with: pip install 'strawberry-graphql[fastapi]'"
            )
            self.router = None
