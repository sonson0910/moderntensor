"""
Block and transaction validation for ModernTensor Layer 1 blockchain.

Implements validation rules and transaction execution.
"""
import logging
from typing import Optional, Dict, Any
from dataclasses import dataclass

from .block import Block
from .transaction import Transaction, TransactionReceipt
from .state import StateDB, Account
from .crypto import MerkleTree

logger = logging.getLogger(__name__)


@dataclass
class ChainConfig:
    """
    Blockchain configuration parameters.
    
    Attributes:
        chain_id: Network identifier
        block_gas_limit: Maximum gas per block
        block_time: Target time between blocks (seconds)
        min_gas_price: Minimum gas price accepted
    """
    chain_id: int = 1
    block_gas_limit: int = 10_000_000
    block_time: int = 12  # seconds
    min_gas_price: int = 1


class BlockValidator:
    """
    Validates blocks and transactions according to consensus rules.
    """
    
    def __init__(self, state_db: StateDB, config: ChainConfig):
        """
        Initialize block validator.
        
        Args:
            state_db: State database instance
            config: Chain configuration
        """
        self.state = state_db
        self.config = config
    
    def validate_block(self, block: Block, parent_block: Optional[Block] = None) -> bool:
        """
        Perform full block validation.
        
        Checks:
        1. Block structure
        2. Previous hash linkage
        3. Timestamp validity
        4. Validator signature
        5. Transaction validity
        6. State root correctness
        7. Gas usage
        
        Args:
            block: Block to validate
            parent_block: Previous block in chain (None for genesis)
            
        Returns:
            bool: True if block is valid
        """
        # 1. Check block structure
        if not block.validate_structure():
            logger.error(f"Block {block.header.height} failed structure validation")
            return False
        
        # 2. Verify previous hash (skip for genesis)
        if parent_block is not None:
            if block.header.previous_hash != parent_block.hash():
                logger.error(
                    f"Block {block.header.height} has invalid previous hash. "
                    f"Expected {parent_block.hash().hex()}, got {block.header.previous_hash.hex()}"
                )
                return False
            
            # Check height is sequential
            if block.header.height != parent_block.header.height + 1:
                logger.error(
                    f"Block height {block.header.height} is not sequential "
                    f"(parent height: {parent_block.header.height})"
                )
                return False
        
        # 3. Check timestamp (must be greater than parent, not too far in future)
        if parent_block is not None:
            if block.header.timestamp <= parent_block.header.timestamp:
                logger.error("Block timestamp is not greater than parent timestamp")
                return False
        
        # TODO: Check timestamp is not too far in the future
        
        # 4. Verify validator signature
        if not block.header.verify_signature():
            logger.error("Block signature verification failed")
            return False
        
        # 5. Validate all transactions
        total_gas_used = 0
        for i, tx in enumerate(block.transactions):
            if not self.validate_transaction(tx):
                logger.error(f"Transaction {i} in block {block.header.height} is invalid")
                return False
            
            # Accumulate gas
            total_gas_used += tx.intrinsic_gas()
        
        # 6. Verify gas usage
        if total_gas_used > block.header.gas_limit:
            logger.error(
                f"Block gas used ({total_gas_used}) exceeds gas limit ({block.header.gas_limit})"
            )
            return False
        
        if block.header.gas_used != total_gas_used:
            logger.error(
                f"Block header gas_used ({block.header.gas_used}) "
                f"doesn't match calculated ({total_gas_used})"
            )
            return False
        
        # 7. Verify transaction merkle root
        if block.transactions:
            tx_tree = MerkleTree.from_transactions(block.transactions)
            if tx_tree.root() != block.header.txs_root:
                logger.error("Transaction merkle root mismatch")
                return False
        
        # TODO: 8. Verify state root after executing all transactions
        
        logger.info(f"Block {block.header.height} validation passed")
        return True
    
    def validate_transaction(self, tx: Transaction) -> bool:
        """
        Validate a single transaction.
        
        Checks:
        1. Signature validity
        2. Nonce correctness
        3. Sufficient balance for value + gas
        4. Gas limit validity
        
        Args:
            tx: Transaction to validate
            
        Returns:
            bool: True if transaction is valid
        """
        # 1. Verify signature
        if not tx.verify_signature():
            logger.error(f"Transaction signature verification failed")
            return False
        
        # 2. Check sender matches signature
        recovered_sender = tx.sender()
        if recovered_sender != tx.from_address:
            logger.error("Transaction sender doesn't match signature")
            return False
        
        # 3. Check nonce
        account = self.state.get_account(tx.from_address)
        if tx.nonce != account.nonce:
            logger.error(
                f"Transaction nonce ({tx.nonce}) doesn't match account nonce ({account.nonce})"
            )
            return False
        
        # 4. Check balance for value + gas
        max_cost = tx.value + (tx.gas_limit * tx.gas_price)
        if account.balance < max_cost:
            logger.error(
                f"Insufficient balance. Required: {max_cost}, Available: {account.balance}"
            )
            return False
        
        # 5. Check gas limit
        intrinsic_gas = tx.intrinsic_gas()
        if tx.gas_limit < intrinsic_gas:
            logger.error(
                f"Gas limit ({tx.gas_limit}) less than intrinsic gas ({intrinsic_gas})"
            )
            return False
        
        if tx.gas_limit > self.config.block_gas_limit:
            logger.error(
                f"Gas limit ({tx.gas_limit}) exceeds block gas limit ({self.config.block_gas_limit})"
            )
            return False
        
        # 6. Check gas price
        if tx.gas_price < self.config.min_gas_price:
            logger.error(f"Gas price ({tx.gas_price}) below minimum ({self.config.min_gas_price})")
            return False
        
        return True
    
    def execute_transaction(
        self,
        tx: Transaction,
        block_height: int,
        block_hash: bytes,
        tx_index: int
    ) -> TransactionReceipt:
        """
        Execute a transaction and update state.
        
        Steps:
        1. Deduct gas cost upfront
        2. Transfer value
        3. Execute contract code (if any)
        4. Refund unused gas
        5. Update state
        6. Generate receipt
        
        Args:
            tx: Transaction to execute
            block_height: Current block height
            block_hash: Current block hash
            tx_index: Transaction index in block
            
        Returns:
            TransactionReceipt: Execution receipt
        """
        # Create receipt
        receipt = TransactionReceipt(
            transaction_hash=tx.hash(),
            block_hash=block_hash,
            block_height=block_height,
            transaction_index=tx_index,
            from_address=tx.from_address,
            to_address=tx.to_address,
        )
        
        try:
            # 1. Deduct max gas cost upfront
            max_gas_cost = tx.gas_limit * tx.gas_price
            if not self.state.sub_balance(tx.from_address, max_gas_cost):
                receipt.status = 0  # Failed
                receipt.gas_used = tx.intrinsic_gas()
                return receipt
            
            # 2. Increment nonce
            self.state.increment_nonce(tx.from_address)
            
            # 3. Calculate intrinsic gas
            intrinsic_gas = tx.intrinsic_gas()
            gas_remaining = tx.gas_limit - intrinsic_gas
            
            # 4. Transfer value (if any)
            if tx.value > 0:
                if not self.state.sub_balance(tx.from_address, tx.value):
                    # Revert: refund all gas
                    self.state.add_balance(tx.from_address, max_gas_cost)
                    receipt.status = 0
                    receipt.gas_used = tx.intrinsic_gas()
                    return receipt
                
                if tx.to_address:
                    self.state.add_balance(tx.to_address, tx.value)
            
            # 5. Handle contract creation or execution
            if tx.is_contract_creation():
                # Contract creation
                # TODO: Deploy contract code and execute constructor
                # For now, just create an account with code
                contract_address = self._create_contract_address(tx.from_address, tx.nonce - 1)
                receipt.contract_address = contract_address
                
                # Set code if provided in data
                if tx.data:
                    self.state.set_code(contract_address, tx.data)
                
                logger.info(f"Contract created at {contract_address.hex()}")
            elif tx.to_address and self.state.get_code(tx.to_address):
                # Contract call
                # TODO: Execute contract code in VM
                # For now, just log
                logger.info(f"Contract call to {tx.to_address.hex()}")
            
            # 6. Calculate actual gas used
            gas_used = intrinsic_gas  # + execution gas (TODO)
            receipt.gas_used = gas_used
            
            # 7. Refund unused gas
            gas_refund = (tx.gas_limit - gas_used) * tx.gas_price
            self.state.add_balance(tx.from_address, gas_refund)
            
            # Success
            receipt.status = 1
            logger.info(
                f"Transaction {tx.hash().hex()[:8]}... executed successfully. "
                f"Gas used: {gas_used}"
            )
            
        except Exception as e:
            logger.error(f"Transaction execution failed: {e}")
            receipt.status = 0
            receipt.gas_used = tx.intrinsic_gas()
            # Refund remaining gas
            self.state.add_balance(tx.from_address, (tx.gas_limit - receipt.gas_used) * tx.gas_price)
        
        return receipt
    
    def _create_contract_address(self, sender: bytes, nonce: int) -> bytes:
        """
        Generate contract address from sender and nonce.
        
        Args:
            sender: Sender address
            nonce: Sender's nonce
            
        Returns:
            bytes: 20-byte contract address
        """
        from .crypto import sha256
        # Simple address generation: hash(sender + nonce)
        data = sender + nonce.to_bytes(8, 'big')
        return sha256(data)[-20:]
    
    def execute_block(self, block: Block) -> Dict[str, Any]:
        """
        Execute all transactions in a block.
        
        Args:
            block: Block to execute
            
        Returns:
            Dict: Execution results with receipts and state root
        """
        receipts = []
        
        # Execute each transaction
        for i, tx in enumerate(block.transactions):
            receipt = self.execute_transaction(
                tx,
                block.header.height,
                block.hash(),
                i
            )
            receipts.append(receipt)
        
        # Commit state changes
        new_state_root = self.state.commit()
        
        return {
            "receipts": receipts,
            "state_root": new_state_root,
            "gas_used": sum(r.gas_used for r in receipts),
        }
