"""
JSON-RPC API server for ModernTensor Layer 1 blockchain.

Provides Ethereum-compatible JSON-RPC methods plus ModernTensor-specific
AI validation methods.
"""

import logging
from typing import Optional, Dict, Any, List, Union
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
import asyncio
from datetime import datetime

from sdk.blockchain.block import Block
from sdk.blockchain.transaction import Transaction, TransactionReceipt
from sdk.blockchain.state import StateDB
from sdk.storage.blockchain_db import BlockchainDB
from sdk.storage.indexer import Indexer, MemoryIndexer
from sdk.blockchain.validation import BlockValidator, ChainConfig

logger = logging.getLogger(__name__)


class JSONRPCRequest(BaseModel):
    """JSON-RPC 2.0 request format"""
    jsonrpc: str = "2.0"
    method: str
    params: List[Any] = Field(default_factory=list)
    id: Union[str, int, None] = None


class JSONRPCResponse(BaseModel):
    """JSON-RPC 2.0 response format"""
    jsonrpc: str = "2.0"
    result: Optional[Any] = None
    error: Optional[Dict[str, Any]] = None
    id: Union[str, int, None] = None


class JSONRPCError:
    """Standard JSON-RPC error codes"""
    PARSE_ERROR = {"code": -32700, "message": "Parse error"}
    INVALID_REQUEST = {"code": -32600, "message": "Invalid Request"}
    METHOD_NOT_FOUND = {"code": -32601, "message": "Method not found"}
    INVALID_PARAMS = {"code": -32602, "message": "Invalid params"}
    INTERNAL_ERROR = {"code": -32603, "message": "Internal error"}
    
    # Custom errors
    BLOCK_NOT_FOUND = {"code": -32001, "message": "Block not found"}
    TRANSACTION_NOT_FOUND = {"code": -32002, "message": "Transaction not found"}
    INVALID_ADDRESS = {"code": -32003, "message": "Invalid address"}


class JSONRPC:
    """
    JSON-RPC API server for ModernTensor blockchain.
    
    Provides both Ethereum-compatible RPC methods and ModernTensor-specific
    AI validation methods.
    """
    
    def __init__(
        self,
        blockchain_db: BlockchainDB,
        state_db: StateDB,
        indexer: Optional[Indexer] = None,
        validator: Optional[BlockValidator] = None,
        chain_config: Optional[ChainConfig] = None,
    ):
        """
        Initialize JSON-RPC server.
        
        Args:
            blockchain_db: Blockchain database instance
            state_db: State database instance
            indexer: Indexer for fast queries
            validator: Block validator instance
            chain_config: Chain configuration
        """
        self.blockchain_db = blockchain_db
        self.state_db = state_db
        self.indexer = indexer or MemoryIndexer()
        self.validator = validator
        self.chain_config = chain_config or ChainConfig()
        self.app = FastAPI(title="ModernTensor JSON-RPC API")
        
        # Transaction pool for pending transactions
        self.tx_pool: Dict[bytes, Transaction] = {}
        
        # AI task storage (in-memory for now)
        self.ai_tasks: Dict[str, Dict[str, Any]] = {}
        
        self._register_routes()
        
        logger.info("JSON-RPC server initialized")
    
    def _register_routes(self):
        """Register FastAPI routes"""
        
        @self.app.post("/")
        async def rpc_endpoint(request: JSONRPCRequest) -> JSONRPCResponse:
            """Main JSON-RPC endpoint"""
            try:
                # Map method name to handler
                handler = self._get_method_handler(request.method)
                
                if handler is None:
                    return JSONRPCResponse(
                        id=request.id,
                        error=JSONRPCError.METHOD_NOT_FOUND
                    )
                
                # Execute method
                result = await handler(*request.params)
                
                return JSONRPCResponse(
                    id=request.id,
                    result=result
                )
                
            except Exception as e:
                logger.error(f"RPC error: {e}")
                return JSONRPCResponse(
                    id=request.id,
                    error={
                        "code": JSONRPCError.INTERNAL_ERROR["code"],
                        "message": str(e)
                    }
                )
        
        @self.app.get("/health")
        async def health_check():
            """Health check endpoint"""
            return {"status": "healthy", "timestamp": datetime.now().isoformat()}
    
    def _get_method_handler(self, method: str):
        """Get handler function for RPC method"""
        handlers = {
            # Chain queries
            "eth_blockNumber": self.eth_blockNumber,
            "eth_getBlockByNumber": self.eth_getBlockByNumber,
            "eth_getBlockByHash": self.eth_getBlockByHash,
            "eth_chainId": self.eth_chainId,
            
            # Account queries
            "eth_getBalance": self.eth_getBalance,
            "eth_getTransactionCount": self.eth_getTransactionCount,
            "eth_getCode": self.eth_getCode,
            
            # Transaction operations
            "eth_sendRawTransaction": self.eth_sendRawTransaction,
            "eth_getTransactionByHash": self.eth_getTransactionByHash,
            "eth_getTransactionReceipt": self.eth_getTransactionReceipt,
            "eth_estimateGas": self.eth_estimateGas,
            
            # Gas price
            "eth_gasPrice": self.eth_gasPrice,
            
            # AI-specific methods
            "mt_submitAITask": self.mt_submitAITask,
            "mt_getAIResult": self.mt_getAIResult,
            "mt_listAITasks": self.mt_listAITasks,
            "mt_getValidatorInfo": self.mt_getValidatorInfo,
        }
        
        return handlers.get(method)
    
    # ===== Chain Query Methods =====
    
    async def eth_blockNumber(self) -> int:
        """
        Get current block number (height).
        
        Returns:
            int: Current block height
        """
        best_height = self.blockchain_db.get_best_height()
        return best_height if best_height is not None else 0
    
    async def eth_getBlockByNumber(
        self, 
        block_number: Union[int, str], 
        full_transactions: bool = False
    ) -> Optional[Dict[str, Any]]:
        """
        Get block by number.
        
        Args:
            block_number: Block number or "latest", "earliest", "pending"
            full_transactions: If True, return full tx objects, else just hashes
            
        Returns:
            Block data as dictionary or None if not found
        """
        # Handle special block identifiers
        if isinstance(block_number, str):
            if block_number == "latest":
                block_number = self.blockchain_db.get_best_height()
            elif block_number == "earliest":
                block_number = 0
            elif block_number == "pending":
                # Return None for pending block
                return None
            else:
                # Parse hex string
                block_number = int(block_number, 16)
        
        # Get block from database
        block = self.blockchain_db.get_block_by_height(block_number)
        
        if block is None:
            return None
        
        return self._format_block(block, full_transactions)
    
    async def eth_getBlockByHash(
        self, 
        block_hash: str, 
        full_transactions: bool = False
    ) -> Optional[Dict[str, Any]]:
        """
        Get block by hash.
        
        Args:
            block_hash: Block hash (hex string with 0x prefix)
            full_transactions: If True, return full tx objects, else just hashes
            
        Returns:
            Block data as dictionary or None if not found
        """
        # Parse hash
        block_hash_bytes = bytes.fromhex(block_hash.removeprefix("0x"))
        
        # Get block from database
        block = self.blockchain_db.get_block(block_hash_bytes)
        
        if block is None:
            return None
        
        return self._format_block(block, full_transactions)
    
    async def eth_chainId(self) -> int:
        """
        Get chain ID.
        
        Returns:
            int: Chain ID
        """
        return self.chain_config.chain_id
    
    # ===== Account Query Methods =====
    
    async def eth_getBalance(self, address: str, block: str = "latest") -> str:
        """
        Get account balance.
        
        Args:
            address: Address (hex string with 0x prefix)
            block: Block number or "latest"
            
        Returns:
            Balance as hex string
        """
        # Parse address
        address_bytes = bytes.fromhex(address.removeprefix("0x"))
        
        # Get account from state
        account = self.state_db.get_account(address_bytes)
        
        # Return balance as hex string
        return hex(account.balance)
    
    async def eth_getTransactionCount(
        self, 
        address: str, 
        block: str = "latest"
    ) -> str:
        """
        Get account nonce (transaction count).
        
        Args:
            address: Address (hex string with 0x prefix)
            block: Block number or "latest"
            
        Returns:
            Nonce as hex string
        """
        # Parse address
        address_bytes = bytes.fromhex(address.removeprefix("0x"))
        
        # Get account from state
        account = self.state_db.get_account(address_bytes)
        
        # Return nonce as hex string
        return hex(account.nonce)
    
    async def eth_getCode(self, address: str, block: str = "latest") -> str:
        """
        Get contract code.
        
        Args:
            address: Contract address (hex string with 0x prefix)
            block: Block number or "latest"
            
        Returns:
            Contract code as hex string
        """
        # Parse address
        address_bytes = bytes.fromhex(address.removeprefix("0x"))
        
        # Get account from state
        account = self.state_db.get_account(address_bytes)
        
        # Get code from storage
        code = self.state_db.get_code(account.code_hash)
        
        if code:
            return "0x" + code.hex()
        return "0x"
    
    # ===== Transaction Methods =====
    
    async def eth_sendRawTransaction(self, tx_hex: str) -> str:
        """
        Submit signed transaction.
        
        Args:
            tx_hex: Signed transaction data (hex string with 0x prefix)
            
        Returns:
            Transaction hash as hex string
        """
        # Parse transaction data
        tx_bytes = bytes.fromhex(tx_hex.removeprefix("0x"))
        
        # Deserialize transaction
        tx = Transaction.deserialize(tx_bytes)
        
        # Validate transaction
        if not tx.verify_signature():
            raise HTTPException(status_code=400, detail="Invalid signature")
        
        # Add to transaction pool
        tx_hash = tx.hash()
        self.tx_pool[tx_hash] = tx
        
        logger.info(f"Transaction added to pool: {tx_hash.hex()}")
        
        # Return transaction hash
        return "0x" + tx_hash.hex()
    
    async def eth_getTransactionByHash(self, tx_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get transaction by hash.
        
        Args:
            tx_hash: Transaction hash (hex string with 0x prefix)
            
        Returns:
            Transaction data as dictionary or None if not found
        """
        # Parse hash
        tx_hash_bytes = bytes.fromhex(tx_hash.removeprefix("0x"))
        
        # Check transaction pool first
        if tx_hash_bytes in self.tx_pool:
            tx = self.tx_pool[tx_hash_bytes]
            return self._format_transaction(tx, pending=True)
        
        # Get from database
        result = self.blockchain_db.get_transaction(tx_hash_bytes)
        
        if result is None:
            return None
        
        block_hash, tx = result
        return self._format_transaction(tx, block_hash=block_hash)
    
    async def eth_getTransactionReceipt(
        self, 
        tx_hash: str
    ) -> Optional[Dict[str, Any]]:
        """
        Get transaction receipt.
        
        Args:
            tx_hash: Transaction hash (hex string with 0x prefix)
            
        Returns:
            Receipt data as dictionary or None if not found
        """
        # Parse hash
        tx_hash_bytes = bytes.fromhex(tx_hash.removeprefix("0x"))
        
        # Get receipt from database
        receipt = self.blockchain_db.get_receipt(tx_hash_bytes)
        
        if receipt is None:
            return None
        
        return self._format_receipt(receipt)
    
    async def eth_estimateGas(self, tx_params: Dict[str, Any]) -> str:
        """
        Estimate gas for transaction.
        
        Args:
            tx_params: Transaction parameters
            
        Returns:
            Estimated gas as hex string
        """
        # For now, return a simple estimate
        # In production, this would simulate the transaction
        base_gas = 21000  # Base transaction cost
        
        # Add data cost
        if "data" in tx_params and tx_params["data"]:
            data = bytes.fromhex(tx_params["data"].removeprefix("0x"))
            data_gas = sum(68 if b != 0 else 4 for b in data)
            base_gas += data_gas
        
        return hex(base_gas)
    
    async def eth_gasPrice(self) -> str:
        """
        Get current gas price.
        
        Returns:
            Gas price as hex string
        """
        # Return configured minimum gas price
        return hex(self.chain_config.min_gas_price)
    
    # ===== AI-Specific Methods =====
    
    async def mt_submitAITask(self, task_params: Dict[str, Any]) -> str:
        """
        Submit AI task to the network.
        
        Args:
            task_params: Task parameters
                - model_hash: Hash of the model
                - input_data: Input data for inference
                - requester: Requester address
                - reward: Reward amount
                
        Returns:
            Task ID as hex string
        """
        import hashlib
        import time
        
        # Generate task ID
        task_id_input = (
            str(task_params.get("model_hash", "")) +
            str(task_params.get("input_data", "")) +
            str(time.time())
        )
        task_id = hashlib.sha256(task_id_input.encode()).hexdigest()
        
        # Store task
        self.ai_tasks[task_id] = {
            "task_id": task_id,
            "model_hash": task_params.get("model_hash"),
            "input_data": task_params.get("input_data"),
            "requester": task_params.get("requester"),
            "reward": task_params.get("reward", 0),
            "status": "pending",
            "submitted_at": datetime.now().isoformat(),
            "result": None,
        }
        
        logger.info(f"AI task submitted: {task_id}")
        
        return task_id
    
    async def mt_getAIResult(self, task_id: str) -> Optional[Dict[str, Any]]:
        """
        Get AI task result.
        
        Args:
            task_id: Task ID
            
        Returns:
            Task result or None if not found
        """
        return self.ai_tasks.get(task_id)
    
    async def mt_listAITasks(
        self, 
        status: Optional[str] = None,
        limit: int = 100
    ) -> List[Dict[str, Any]]:
        """
        List AI tasks.
        
        Args:
            status: Filter by status (pending, completed, failed)
            limit: Maximum number of tasks to return
            
        Returns:
            List of tasks
        """
        tasks = list(self.ai_tasks.values())
        
        # Filter by status
        if status:
            tasks = [t for t in tasks if t["status"] == status]
        
        # Limit results
        return tasks[:limit]
    
    async def mt_getValidatorInfo(self, address: str) -> Optional[Dict[str, Any]]:
        """
        Get validator information.
        
        Args:
            address: Validator address
            
        Returns:
            Validator info or None if not found
        """
        # This would query the consensus layer for validator info
        # For now, return placeholder data
        return {
            "address": address,
            "stake": "0",
            "active": False,
            "commission": "0",
        }
    
    # ===== Helper Methods =====
    
    def _format_block(
        self, 
        block: Block, 
        full_transactions: bool = False
    ) -> Dict[str, Any]:
        """Format block for JSON-RPC response"""
        block_dict = {
            "number": hex(block.header.height),
            "hash": "0x" + block.hash().hex(),
            "parentHash": "0x" + block.header.previous_hash.hex(),
            "timestamp": hex(block.header.timestamp),
            "stateRoot": "0x" + block.header.state_root.hex(),
            "transactionsRoot": "0x" + block.header.txs_root.hex(),
            "receiptsRoot": "0x" + block.header.receipts_root.hex(),
            "miner": "0x" + block.header.validator.hex(),
            "gasUsed": hex(block.header.gas_used),
            "gasLimit": hex(block.header.gas_limit),
            "extraData": "0x" + block.header.extra_data.hex(),
            "size": hex(len(block.serialize())),
        }
        
        # Add transactions
        if full_transactions:
            block_dict["transactions"] = [
                self._format_transaction(tx, block_hash=block.hash())
                for tx in block.transactions
            ]
        else:
            block_dict["transactions"] = [
                "0x" + tx.hash().hex()
                for tx in block.transactions
            ]
        
        return block_dict
    
    def _format_transaction(
        self, 
        tx: Transaction, 
        block_hash: Optional[bytes] = None,
        pending: bool = False
    ) -> Dict[str, Any]:
        """Format transaction for JSON-RPC response"""
        tx_dict = {
            "hash": "0x" + tx.hash().hex(),
            "nonce": hex(tx.nonce),
            "from": "0x" + tx.from_address.hex(),
            "to": "0x" + tx.to_address.hex() if tx.to_address else None,
            "value": hex(tx.value),
            "gasPrice": hex(tx.gas_price),
            "gas": hex(tx.gas_limit),
            "input": "0x" + tx.data.hex(),
            "v": hex(tx.v),
            "r": "0x" + tx.r.hex(),
            "s": "0x" + tx.s.hex(),
        }
        
        if not pending and block_hash:
            tx_dict["blockHash"] = "0x" + block_hash.hex()
        
        return tx_dict
    
    def _format_receipt(self, receipt: TransactionReceipt) -> Dict[str, Any]:
        """Format transaction receipt for JSON-RPC response"""
        return {
            "transactionHash": "0x" + receipt.transaction_hash.hex(),
            "blockHash": "0x" + receipt.block_hash.hex(),
            "blockNumber": hex(receipt.block_height),
            "gasUsed": hex(receipt.gas_used),
            "cumulativeGasUsed": hex(receipt.gas_used),  # Simplified for now
            "contractAddress": (
                "0x" + receipt.contract_address.hex()
                if receipt.contract_address else None
            ),
            "status": hex(receipt.status),
            "logs": receipt.logs,
        }
