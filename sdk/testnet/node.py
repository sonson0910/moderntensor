"""
Integrated Node Implementation for ModernTensor Layer 1 Blockchain

This module ties together all the blockchain components:
- Blockchain primitives (Block, Transaction, State)
- Consensus (PoS, Validators)
- Network (P2P, Sync)
- Storage
- Testnet tools (Genesis, Faucet)
"""

import asyncio
import logging
from typing import Optional, List, Dict
from pathlib import Path

# Import all blockchain components
from ..blockchain import Block, BlockHeader, Transaction
from ..blockchain.state import StateDB, Account
# Import validation module for ChainConfig
from ..blockchain.validation import BlockValidator, ChainConfig
from ..blockchain.crypto import KeyPair

from ..consensus.pos import ProofOfStake, ValidatorSet, ConsensusConfig as PosConsensusConfig
from ..network.p2p import P2PNode
from ..network.sync import SyncManager

# Import testnet components
from .genesis import GenesisConfig, GenesisGenerator
from .faucet import Faucet, FaucetConfig
from .bootstrap import BootstrapNode, BootstrapConfig
from .monitoring import TestnetMonitor, NodeHealth

logger = logging.getLogger(__name__)


class L1Node:
    """
    Complete Layer 1 blockchain node integrating all components
    
    This is the main entry point that ties together:
    - Block production and validation
    - State management
    - Consensus mechanism (PoS)
    - P2P networking
    - Transaction pool
    """
    
    def __init__(
        self,
        node_id: str,
        data_dir: Path,
        genesis_config: GenesisConfig,
        is_validator: bool = False,
        validator_keypair: Optional[KeyPair] = None
    ):
        """
        Initialize a complete Layer 1 node
        
        Args:
            node_id: Unique identifier for this node
            data_dir: Directory for blockchain data storage
            genesis_config: Genesis configuration
            is_validator: Whether this node is a validator
            validator_keypair: Validator's keypair if is_validator=True
        """
        self.node_id = node_id
        self.data_dir = Path(data_dir)
        self.data_dir.mkdir(parents=True, exist_ok=True)
        self.genesis_config = genesis_config
        self.is_validator = is_validator
        self.validator_keypair = validator_keypair
        
        # Initialize blockchain components
        self.state_db = StateDB(str(self.data_dir / "state"))
        self.blockchain: List[Block] = []
        self.current_height = 0
        
        # Initialize consensus with state_db and config
        pos_config = genesis_config.consensus.to_pos_config()
        self.consensus = ProofOfStake(self.state_db, pos_config)
        
        # Initialize validator set from genesis
        self._initialize_validators()
        
        # Transaction pool (mempool)
        self.mempool: List[Transaction] = []
        
        # Block validator with chain config
        chain_config = ChainConfig(
            chain_id=genesis_config.chain_id,
            block_gas_limit=genesis_config.block_gas_limit,
            block_time=genesis_config.consensus.slot_duration,
            min_gas_price=genesis_config.min_gas_price
        )
        self.block_validator = BlockValidator(self.state_db, chain_config)
        
        # P2P network (will be initialized on start)
        self.p2p_node: Optional[P2PNode] = None
        self.sync_manager: Optional[SyncManager] = None
        
        # Monitoring
        self.monitor = TestnetMonitor()
        
        self.running = False
        
        logger.info(f"Initialized L1 Node: {node_id}")
        logger.info(f"  Data dir: {data_dir}")
        logger.info(f"  Validator: {is_validator}")
        logger.info(f"  Chain ID: {genesis_config.chain_id}")
    
    def _initialize_validators(self):
        """Initialize validator set from genesis configuration"""
        for validator_config in self.genesis_config.initial_validators:
            address = bytes.fromhex(validator_config.address[2:])
            public_key = bytes.fromhex(validator_config.public_key[2:])
            
            self.consensus.validator_set.add_validator(
                address=address,
                public_key=public_key,
                stake=validator_config.stake
            )
    
    def load_genesis(self):
        """
        Load and apply genesis block and state
        """
        logger.info("Loading genesis block...")
        
        # Generate genesis block
        generator = GenesisGenerator()
        generator.config = self.genesis_config
        genesis_block = generator.generate_genesis_block()
        
        # Initialize genesis state
        genesis_state = generator.initialize_genesis_state()
        
        # Copy genesis state to our state db
        for validator in self.genesis_config.initial_validators:
            address = bytes.fromhex(validator.address[2:])
            account = genesis_state.get_account(address)
            if account:
                self.state_db.set_account(address, account)
        
        for account_config in self.genesis_config.initial_accounts:
            address = bytes.fromhex(account_config.address[2:])
            account = genesis_state.get_account(address)
            if account:
                self.state_db.set_account(address, account)
        
        self.state_db.commit()
        
        # Add genesis block to blockchain
        self.blockchain.append(genesis_block)
        self.current_height = 0
        
        logger.info(f"✅ Genesis block loaded at height {self.current_height}")
        logger.info(f"   State root: {genesis_block.header.state_root.hex()[:16]}...")
    
    async def start(self):
        """Start the node"""
        self.running = True
        logger.info(f"🚀 Starting L1 Node {self.node_id}...")
        
        # Load genesis if blockchain is empty
        if len(self.blockchain) == 0:
            self.load_genesis()
        
        # Start monitoring
        await self.monitor.start()
        
        # Initialize P2P network
        node_id_bytes = self.node_id.encode('utf-8') if isinstance(self.node_id, str) else self.node_id
        self.p2p_node = P2PNode(
            listen_port=self.genesis_config.network.p2p_port,
            bootstrap_nodes=self.genesis_config.network.bootstrap_nodes,
            node_id=node_id_bytes,
            network_id=self.genesis_config.chain_id
        )
        
        # Start P2P node
        await self.p2p_node.start()
        
        # Initialize sync manager
        self.sync_manager = SyncManager(self)
        
        # Start block production if validator
        if self.is_validator:
            asyncio.create_task(self._block_production_loop())
        
        # Start transaction processing
        asyncio.create_task(self._process_transactions())
        
        logger.info(f"✅ L1 Node {self.node_id} running")
    
    async def stop(self):
        """Stop the node"""
        self.running = False
        
        if self.p2p_node:
            await self.p2p_node.stop()
        
        if self.monitor:
            await self.monitor.stop()
        
        logger.info(f"🛑 L1 Node {self.node_id} stopped")
    
    async def _block_production_loop(self):
        """
        Main block production loop for validators
        """
        logger.info("Starting block production loop...")
        
        while self.running:
            try:
                # Check if it's our turn to produce a block
                current_slot = self._get_current_slot()
                selected_validator = self.consensus.select_validator(current_slot)
                
                if selected_validator == self.validator_keypair.address():
                    # It's our turn - produce a block
                    await self._produce_block()
                
                # Wait for next slot
                await asyncio.sleep(self.genesis_config.consensus.slot_duration)
                
            except Exception as e:
                logger.error(f"Error in block production: {e}")
                await asyncio.sleep(1)
    
    async def _produce_block(self):
        """
        Produce a new block
        """
        logger.info(f"Producing block at height {self.current_height + 1}")
        
        # Get parent block
        parent_block = self.blockchain[-1] if self.blockchain else None
        if not parent_block:
            logger.error("No parent block found")
            return
        
        # Select transactions from mempool
        transactions = self._select_transactions()
        
        # Execute transactions and update state
        receipts = []
        for tx in transactions:
            try:
                receipt = self.block_validator.execute_transaction(tx)
                receipts.append(receipt)
            except Exception as e:
                logger.warning(f"Transaction execution failed: {e}")
        
        # Commit state changes
        new_state_root = self.state_db.get_state_root()
        
        # Create block header
        header = BlockHeader(
            version=1,
            height=self.current_height + 1,
            timestamp=int(time.time()),
            previous_hash=parent_block.hash(),
            state_root=new_state_root,
            txs_root=self._calculate_txs_root(transactions),
            receipts_root=self._calculate_receipts_root(receipts),
            validator=self.validator_keypair.address(),
            signature=b'\x00' * 64,  # Will be filled after signing
            gas_used=sum(r.gas_used for r in receipts),
            gas_limit=self.genesis_config.block_gas_limit,
            extra_data=b''
        )
        
        # Create block
        block = Block(header=header, transactions=transactions)
        
        # Sign block
        block_hash = block.hash()
        signature = self.validator_keypair.sign(block_hash)
        block.header.signature = signature
        
        # Add to blockchain
        self.blockchain.append(block)
        self.current_height += 1
        
        # Remove transactions from mempool
        for tx in transactions:
            if tx in self.mempool:
                self.mempool.remove(tx)
        
        # Broadcast block to network
        if self.p2p_node:
            await self.p2p_node.broadcast_block(block)
        
        logger.info(f"✅ Produced block {self.current_height}: {block_hash.hex()[:16]}...")
        
        # Update health monitoring
        await self._update_health()
    
    async def _process_transactions(self):
        """
        Process incoming transactions
        """
        while self.running:
            try:
                # In a real implementation, this would listen for transactions from network
                await asyncio.sleep(1)
            except Exception as e:
                logger.error(f"Error processing transactions: {e}")
    
    def add_transaction(self, tx: Transaction) -> bool:
        """
        Add a transaction to the mempool
        
        Args:
            tx: Transaction to add
        
        Returns:
            bool: True if added successfully
        """
        try:
            # Validate transaction
            if not self.block_validator.validate_transaction(tx):
                return False
            
            # Add to mempool
            self.mempool.append(tx)
            
            # Broadcast to network
            if self.p2p_node:
                asyncio.create_task(self.p2p_node.broadcast_transaction(tx))
            
            return True
        except Exception as e:
            logger.error(f"Failed to add transaction: {e}")
            return False
    
    def _select_transactions(self, max_count: int = 100) -> List[Transaction]:
        """Select transactions from mempool for next block"""
        return self.mempool[:max_count]
    
    def _get_current_slot(self) -> int:
        """Get current consensus slot"""
        import time
        genesis_time = int(datetime.fromisoformat(self.genesis_config.genesis_time).timestamp())
        current_time = int(time.time())
        elapsed = current_time - genesis_time
        return elapsed // self.genesis_config.consensus.slot_duration
    
    def _calculate_txs_root(self, transactions: List[Transaction]) -> bytes:
        """Calculate Merkle root of transactions"""
        # Simplified implementation
        import hashlib
        if not transactions:
            return b'\x00' * 32
        
        tx_hashes = [tx.hash() for tx in transactions]
        combined = b''.join(tx_hashes)
        return hashlib.sha256(combined).digest()
    
    def _calculate_receipts_root(self, receipts: List) -> bytes:
        """Calculate Merkle root of receipts"""
        # Simplified implementation
        import hashlib
        if not receipts:
            return b'\x00' * 32
        
        return hashlib.sha256(str(len(receipts)).encode()).digest()
    
    async def _update_health(self):
        """Update node health metrics"""
        health = NodeHealth(
            node_id=self.node_id,
            status="healthy" if self.running else "down",
            last_block_height=self.current_height,
            last_block_time=self.blockchain[-1].header.timestamp if self.blockchain else 0,
            peer_count=len(self.p2p_node.peers) if self.p2p_node else 0,
            sync_status="synced" if self.current_height > 0 else "syncing"
        )
        self.monitor.update_node_health(health)
    
    def get_block(self, height: int) -> Optional[Block]:
        """Get block by height"""
        if 0 <= height < len(self.blockchain):
            return self.blockchain[height]
        return None
    
    def get_latest_block(self) -> Optional[Block]:
        """Get the latest block"""
        return self.blockchain[-1] if self.blockchain else None
    
    def get_account(self, address: bytes) -> Optional[Account]:
        """Get account state"""
        return self.state_db.get_account(address)


# Import for convenience
import time
from datetime import datetime
