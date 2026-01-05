"""
Example demonstrating Phase 3 Network Layer usage.

This script shows how to:
1. Create a P2P node
2. Connect to peers
3. Broadcast transactions and blocks
4. Sync blockchain from peers
"""

import asyncio
import logging
from sdk.network.p2p import P2PNode
from sdk.network.sync import SyncManager
from sdk.network.messages import MessageType
from sdk.blockchain.block import Block, BlockHeader
from sdk.blockchain.transaction import Transaction
from sdk.blockchain.state import StateDB
from sdk.blockchain.validation import BlockValidator, ChainConfig

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


async def example_p2p_node():
    """Example: Create and start a P2P node"""
    
    logger.info("=== Example 1: P2P Node Setup ===")
    
    # Create a node
    node = P2PNode(
        listen_port=30303,
        bootstrap_nodes=['127.0.0.1:30304', '127.0.0.1:30305'],
        node_id=b'\x01' * 32,
        protocol_version=1,
        network_id=1,
        max_peers=50
    )
    
    # Set genesis hash (normally from blockchain)
    node.genesis_hash = b'\x00' * 32
    node.best_height = 0
    node.best_hash = b'\x00' * 32
    
    logger.info(f"Node created on port {node.listen_port}")
    logger.info(f"Max peers: {node.max_peers}")
    
    # Register message handlers
    async def handle_ping(peer, msg):
        logger.info(f"Received PING from {peer.address}")
    
    node.register_handler(MessageType.PING, handle_ping)
    
    # Start the node (would normally run indefinitely)
    # await node.start()
    # ... node would connect to bootstrap nodes and maintain peers
    # await node.stop()
    
    logger.info("P2P node example complete")


async def example_transaction_broadcast():
    """Example: Broadcast a transaction to peers"""
    
    logger.info("=== Example 2: Transaction Broadcasting ===")
    
    # Create a P2P node
    node = P2PNode(
        listen_port=30303,
        bootstrap_nodes=[],
        node_id=b'\x01' * 32
    )
    
    # Create a sample transaction
    tx = Transaction(
        nonce=1,
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
    
    logger.info(f"Created transaction: {tx.hash().hex()[:16]}...")
    
    # Broadcast to all connected peers
    # await node.broadcast_transaction(tx)
    
    logger.info("Transaction would be broadcasted to all peers")


async def example_block_sync():
    """Example: Sync blockchain from peers"""
    
    logger.info("=== Example 3: Blockchain Synchronization ===")
    
    # Setup state and validator
    state_db = StateDB("/tmp/example_state")
    config = ChainConfig(
        chain_id=1,
        block_time=12,
        block_gas_limit=10_000_000
    )
    validator = BlockValidator(state_db, config)
    
    # Create P2P node
    p2p_node = P2PNode(
        listen_port=30303,
        bootstrap_nodes=['127.0.0.1:30304'],
        node_id=b'\x01' * 32
    )
    
    # Create sync manager
    sync_manager = SyncManager(p2p_node, validator)
    
    # Setup callbacks
    def on_block_synced(block: Block):
        logger.info(f"Synced block {block.header.height}")
    
    def on_sync_complete():
        logger.info("Blockchain sync complete!")
    
    sync_manager.on_block_synced = on_block_synced
    sync_manager.on_sync_complete = on_sync_complete
    
    # Sync to target height
    # await sync_manager.sync(target_height=1000)
    
    logger.info("Sync would download blocks 0-1000 from peers")
    logger.info(f"Sync status: {sync_manager.status.progress:.1f}% complete")


async def example_peer_discovery():
    """Example: Discover and connect to peers"""
    
    logger.info("=== Example 4: Peer Discovery ===")
    
    # Create node
    node = P2PNode(
        listen_port=30303,
        bootstrap_nodes=['127.0.0.1:30304'],
        node_id=b'\x01' * 32
    )
    
    # Connect to a specific peer
    # peer = await node.connect_peer('127.0.0.1', 30304)
    # if peer:
    #     logger.info(f"Connected to {peer.address}")
    #     logger.info(f"Peer height: {peer.height}")
    
    # Get best peer (highest height)
    # best_peer = node.get_best_peer()
    # if best_peer:
    #     logger.info(f"Best peer: {best_peer.address} at height {best_peer.height}")
    
    # Get random peer for load balancing
    # random_peer = node.get_random_peer()
    
    logger.info("Peer discovery allows dynamic network topology")


async def example_message_protocol():
    """Example: Use message protocol"""
    
    logger.info("=== Example 5: Message Protocol ===")
    
    from sdk.network.messages import (
        MessageCodec, HelloMessage, GetBlocksMessage
    )
    
    # Create a HELLO message
    hello = HelloMessage(
        protocol_version=1,
        network_id=1,
        genesis_hash=b'\x00' * 32,
        best_height=100,
        best_hash=b'\xff' * 32,
        listen_port=30303,
        node_id=b'\xaa' * 32,
        capabilities=['sync', 'relay']
    )
    
    # Encode to bytes
    encoded = MessageCodec.encode_hello(hello)
    logger.info(f"Encoded HELLO message: {len(encoded)} bytes")
    
    # Decode back
    msg = MessageCodec.decode(encoded)
    decoded_hello = MessageCodec.decode_hello(msg)
    logger.info(f"Decoded: protocol v{decoded_hello.protocol_version}, "
                f"height {decoded_hello.best_height}")
    
    # Create GET_BLOCKS message
    get_blocks = GetBlocksMessage(
        start_height=100,
        end_height=200,
        max_blocks=50
    )
    
    encoded = MessageCodec.encode_get_blocks(get_blocks)
    logger.info(f"Encoded GET_BLOCKS message: {len(encoded)} bytes")


async def main():
    """Run all examples"""
    
    logger.info("ModernTensor Layer 1 - Phase 3 Network Layer Examples")
    logger.info("=" * 60)
    
    await example_p2p_node()
    print()
    
    await example_transaction_broadcast()
    print()
    
    await example_block_sync()
    print()
    
    await example_peer_discovery()
    print()
    
    await example_message_protocol()
    print()
    
    logger.info("=" * 60)
    logger.info("All examples complete!")
    logger.info("\nKey Features Demonstrated:")
    logger.info("  ✓ P2P node creation and configuration")
    logger.info("  ✓ Transaction broadcasting")
    logger.info("  ✓ Blockchain synchronization")
    logger.info("  ✓ Peer discovery and management")
    logger.info("  ✓ Message protocol encoding/decoding")


if __name__ == '__main__':
    asyncio.run(main())
