"""
Blockchain synchronization protocol for ModernTensor Layer 1.

This module implements the sync protocol for downloading and validating
blockchain data from peers.
"""

import asyncio
import logging
import time
from typing import Dict, List, Optional, Callable
from dataclasses import dataclass
import json

from sdk.blockchain.block import Block, BlockHeader
from sdk.blockchain.validation import BlockValidator
from sdk.network.p2p import P2PNode, Peer
from sdk.network.messages import (
    Message, MessageType, MessageCodec,
    GetBlocksMessage, GetHeadersMessage
)

logger = logging.getLogger(__name__)


@dataclass
class SyncStatus:
    """Track synchronization status"""
    
    syncing: bool = False
    start_height: int = 0
    current_height: int = 0
    target_height: int = 0
    start_time: float = 0.0
    blocks_downloaded: int = 0
    
    @property
    def progress(self) -> float:
        """Get sync progress as percentage"""
        if self.target_height == 0:
            return 100.0
        return (self.current_height / self.target_height) * 100.0
    
    @property
    def elapsed_time(self) -> float:
        """Get elapsed time in seconds"""
        if self.start_time == 0:
            return 0.0
        return time.time() - self.start_time
    
    @property
    def blocks_per_second(self) -> float:
        """Get sync speed in blocks per second"""
        if self.elapsed_time == 0:
            return 0.0
        return self.blocks_downloaded / self.elapsed_time


class SyncManager:
    """Manage blockchain synchronization"""
    
    def __init__(self,
                 p2p: P2PNode,
                 validator: BlockValidator,
                 blockchain_storage: Optional[object] = None):
        """
        Initialize sync manager.
        
        Args:
            p2p: P2P network node
            validator: Block validator
            blockchain_storage: Optional blockchain storage backend
        """
        self.p2p = p2p
        self.validator = validator
        self.storage = blockchain_storage
        
        # Sync status
        self.status = SyncStatus()
        
        # Downloaded blocks waiting to be processed
        self.block_queue: asyncio.Queue = asyncio.Queue(maxsize=1000)
        
        # Headers cache
        self.headers_cache: Dict[int, BlockHeader] = {}
        
        # Callbacks
        self.on_block_synced: Optional[Callable[[Block], None]] = None
        self.on_sync_complete: Optional[Callable[[], None]] = None
        
        # Register message handlers
        self.p2p.register_handler(MessageType.BLOCKS, self._handle_blocks_message)
        self.p2p.register_handler(MessageType.HEADERS, self._handle_headers_message)
        self.p2p.register_handler(MessageType.NEW_BLOCK_HASHES, self._handle_new_block_announcement)
        
        logger.info("Sync manager initialized")
    
    async def sync(self, target_height: Optional[int] = None):
        """
        Sync blockchain from peers.
        
        Args:
            target_height: Target height to sync to (None = sync to best peer)
        """
        if self.status.syncing:
            logger.warning("Already syncing")
            return
        
        # Find best peer
        best_peer = self.p2p.get_best_peer()
        if not best_peer:
            logger.error("No peers available for sync")
            return
        
        # Determine target height
        if target_height is None:
            target_height = best_peer.height
        
        # Get current height from validator's state
        current_height = self.validator.state.get_account(b'\x00' * 20).nonce  # Simplified
        
        if current_height >= target_height:
            logger.info(f"Already at height {current_height}, no sync needed")
            return
        
        # Start sync
        self.status = SyncStatus(
            syncing=True,
            start_height=current_height,
            current_height=current_height,
            target_height=target_height,
            start_time=time.time()
        )
        
        logger.info(f"Starting sync from height {current_height} to {target_height}")
        
        try:
            # Sync headers first (headers-first sync)
            await self._sync_headers(best_peer, current_height, target_height)
            
            # Then sync blocks
            await self._sync_blocks(best_peer, current_height, target_height)
            
            # Mark sync complete
            self.status.syncing = False
            
            logger.info(
                f"Sync complete! Synced {self.status.blocks_downloaded} blocks "
                f"in {self.status.elapsed_time:.2f}s "
                f"({self.status.blocks_per_second:.2f} blocks/s)"
            )
            
            if self.on_sync_complete:
                self.on_sync_complete()
                
        except Exception as e:
            logger.error(f"Sync failed: {e}")
            self.status.syncing = False
            raise
    
    async def fast_sync(self, snapshot_height: Optional[int] = None):
        """
        Fast sync using state snapshots.
        
        This downloads a recent state snapshot instead of processing all blocks,
        significantly speeding up initial sync.
        
        Args:
            snapshot_height: Height to snapshot from (None = latest available)
        """
        if self.status.syncing:
            logger.warning("Already syncing")
            return
        
        logger.info("Starting fast sync with state snapshots")
        
        # Find best peer
        best_peer = self.p2p.get_best_peer()
        if not best_peer:
            logger.error("No peers available for fast sync")
            return
        
        # Determine snapshot height
        if snapshot_height is None:
            # Use a recent checkpoint (e.g., 1000 blocks behind best)
            snapshot_height = max(0, best_peer.height - 1000)
        
        self.status = SyncStatus(
            syncing=True,
            start_height=0,
            current_height=0,
            target_height=snapshot_height,
            start_time=time.time()
        )
        
        try:
            # Request state snapshot
            await self._request_state_snapshot(best_peer, snapshot_height)
            
            # Sync remaining blocks normally
            await self._sync_blocks(best_peer, snapshot_height, best_peer.height)
            
            self.status.syncing = False
            
            logger.info(f"Fast sync complete to height {best_peer.height}")
            
            if self.on_sync_complete:
                self.on_sync_complete()
                
        except Exception as e:
            logger.error(f"Fast sync failed: {e}")
            self.status.syncing = False
            raise
    
    async def handle_new_block(self, block: Block, peer: Peer):
        """
        Handle new block announcement from peer.
        
        Args:
            block: New block received
            peer: Peer that sent the block
        """
        try:
            # Validate block
            if self.validator.validate_block(block):
                logger.info(f"Accepted new block at height {block.header.height}")
                
                # Update status
                self.status.current_height = block.header.height
                
                # Store block if storage available
                if self.storage:
                    await self._store_block(block)
                
                # Notify callback
                if self.on_block_synced:
                    self.on_block_synced(block)
            else:
                logger.warning(f"Invalid block from {peer.address}")
                
        except Exception as e:
            logger.error(f"Error handling new block: {e}")
    
    async def _sync_headers(self, peer: Peer, start_height: int, end_height: int):
        """Sync block headers from peer"""
        logger.info(f"Syncing headers from {start_height} to {end_height}")
        
        current = start_height
        batch_size = 192  # Standard batch size
        
        while current < end_height:
            # Request headers
            await self.p2p.request_headers(peer, current, batch_size)
            
            # Wait for headers (with timeout)
            try:
                await asyncio.wait_for(
                    self._wait_for_headers(current),
                    timeout=30.0
                )
                current += batch_size
                
            except asyncio.TimeoutError:
                logger.warning(f"Timeout waiting for headers at {current}")
                # Try with different peer
                peer = self.p2p.get_best_peer()
                if not peer:
                    raise Exception("No peers available")
        
        logger.info(f"Headers sync complete, downloaded {len(self.headers_cache)} headers")
    
    async def _sync_blocks(self, peer: Peer, start_height: int, end_height: int):
        """Sync blocks from peer"""
        logger.info(f"Syncing blocks from {start_height} to {end_height}")
        
        current = start_height
        batch_size = 128  # Smaller batch for blocks
        
        while current < end_height:
            # Request blocks
            await self.p2p.request_blocks(peer, current, min(current + batch_size, end_height))
            
            # Wait for blocks
            try:
                await asyncio.wait_for(
                    self._wait_for_blocks(current, batch_size),
                    timeout=60.0
                )
                current += batch_size
                self.status.current_height = current
                
                # Log progress
                if current % 1000 == 0:
                    logger.info(
                        f"Sync progress: {self.status.progress:.1f}% "
                        f"({current}/{end_height}) "
                        f"at {self.status.blocks_per_second:.2f} blocks/s"
                    )
                
            except asyncio.TimeoutError:
                logger.warning(f"Timeout waiting for blocks at {current}")
                # Try with different peer
                peer = self.p2p.get_best_peer()
                if not peer:
                    raise Exception("No peers available")
        
        logger.info("Block sync complete")
    
    async def _wait_for_headers(self, expected_height: int):
        """Wait for headers to arrive"""
        # This is a simplified implementation
        # In production, would track pending requests more carefully
        while expected_height not in self.headers_cache:
            await asyncio.sleep(0.1)
    
    async def _wait_for_blocks(self, start_height: int, count: int):
        """Wait for blocks to arrive"""
        # Process blocks from queue
        blocks_received = 0
        
        while blocks_received < count:
            try:
                block = await asyncio.wait_for(self.block_queue.get(), timeout=1.0)
                
                # Validate block
                if self.validator.validate_block(block):
                    # Store block
                    if self.storage:
                        await self._store_block(block)
                    
                    # Update counters
                    blocks_received += 1
                    self.status.blocks_downloaded += 1
                    
                    # Notify callback
                    if self.on_block_synced:
                        self.on_block_synced(block)
                else:
                    logger.warning(f"Invalid block at height {block.header.height}")
                    
            except asyncio.TimeoutError:
                # Check if we should continue waiting
                if blocks_received == 0:
                    raise
                else:
                    # Got some blocks, can continue
                    break
    
    async def _request_state_snapshot(self, peer: Peer, height: int):
        """Request state snapshot at given height"""
        logger.info(f"Requesting state snapshot at height {height}")
        
        # Create GET_STATE message
        state_request = json.dumps({
            'height': height
        }).encode('utf-8')
        
        msg = MessageCodec.encode(Message(MessageType.GET_STATE, state_request))
        await peer.send_message(msg)
        
        # Wait for state (simplified)
        # In production, would handle large state transfers more carefully
        await asyncio.sleep(5)
        
        logger.info("State snapshot received")
    
    async def _store_block(self, block: Block):
        """Store block to storage backend"""
        if self.storage and hasattr(self.storage, 'store_block'):
            await self.storage.store_block(block)
    
    async def _handle_blocks_message(self, peer: Peer, msg: Message):
        """Handle BLOCKS message from peer"""
        try:
            # Decode blocks
            data = json.loads(msg.payload.decode('utf-8'))
            blocks_data = data.get('blocks', [])
            
            logger.debug(f"Received {len(blocks_data)} blocks from {peer.address}")
            
            # Parse and queue blocks
            for block_data in blocks_data:
                # Simplified block parsing
                # In production, would use proper Block.deserialize()
                block = self._parse_block(block_data)
                if block:
                    await self.block_queue.put(block)
            
        except Exception as e:
            logger.error(f"Error handling BLOCKS message: {e}")
    
    async def _handle_headers_message(self, peer: Peer, msg: Message):
        """Handle HEADERS message from peer"""
        try:
            # Decode headers
            data = json.loads(msg.payload.decode('utf-8'))
            headers_data = data.get('headers', [])
            
            logger.debug(f"Received {len(headers_data)} headers from {peer.address}")
            
            # Parse and cache headers
            for header_data in headers_data:
                header = self._parse_header(header_data)
                if header:
                    self.headers_cache[header.height] = header
            
        except Exception as e:
            logger.error(f"Error handling HEADERS message: {e}")
    
    async def _handle_new_block_announcement(self, peer: Peer, msg: Message):
        """Handle NEW_BLOCK_HASHES message from peer"""
        try:
            # Decode block announcement
            data = json.loads(msg.payload.decode('utf-8'))
            
            height = data.get('height')
            block_hash = bytes.fromhex(data.get('hash'))
            
            logger.info(f"New block announced at height {height}")
            
            # If we're behind, request the block
            if height > self.status.current_height:
                await self.p2p.request_blocks(peer, height, height)
            
            # Update peer info
            if peer.peer_info:
                peer.peer_info.best_height = height
                peer.peer_info.best_hash = block_hash
            
        except Exception as e:
            logger.error(f"Error handling block announcement: {e}")
    
    def _parse_block(self, block_data: dict) -> Optional[Block]:
        """Parse block from JSON data"""
        try:
            # Simplified block parsing
            # In production, would use Block.from_dict() or similar
            header = BlockHeader(
                version=block_data.get('version', 1),
                height=block_data['height'],
                timestamp=block_data['timestamp'],
                previous_hash=bytes.fromhex(block_data['previous_hash']),
                state_root=bytes.fromhex(block_data.get('state_root', '00' * 32)),
                txs_root=bytes.fromhex(block_data.get('txs_root', '00' * 32)),
                receipts_root=bytes.fromhex(block_data.get('receipts_root', '00' * 32)),
                validator=bytes.fromhex(block_data.get('validator', '00' * 32)),
                signature=bytes.fromhex(block_data.get('signature', '00' * 64)),
                gas_used=block_data.get('gas_used', 0),
                gas_limit=block_data.get('gas_limit', 10000000)
            )
            
            # Parse transactions (simplified)
            transactions = []
            
            return Block(header=header, transactions=transactions)
            
        except Exception as e:
            logger.error(f"Error parsing block: {e}")
            return None
    
    def _parse_header(self, header_data: dict) -> Optional[BlockHeader]:
        """Parse block header from JSON data"""
        try:
            return BlockHeader(
                version=header_data.get('version', 1),
                height=header_data['height'],
                timestamp=header_data['timestamp'],
                previous_hash=bytes.fromhex(header_data['previous_hash']),
                state_root=bytes.fromhex(header_data.get('state_root', '00' * 32)),
                txs_root=bytes.fromhex(header_data.get('txs_root', '00' * 32)),
                receipts_root=bytes.fromhex(header_data.get('receipts_root', '00' * 32)),
                validator=bytes.fromhex(header_data.get('validator', '00' * 32)),
                signature=bytes.fromhex(header_data.get('signature', '00' * 64)),
                gas_used=header_data.get('gas_used', 0),
                gas_limit=header_data.get('gas_limit', 10000000)
            )
        except Exception as e:
            logger.error(f"Error parsing header: {e}")
            return None
