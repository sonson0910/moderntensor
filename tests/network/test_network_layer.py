"""
Tests for Phase 3: Network Layer

Tests for P2P protocol, sync manager, and message protocol.
"""

import pytest
import asyncio
import time
from unittest.mock import Mock, MagicMock, AsyncMock

from sdk.network.messages import (
    Message, MessageType, MessageCodec,
    HelloMessage, GetBlocksMessage, GetHeadersMessage,
    GetPeersMessage, PeersMessage
)
from sdk.network.p2p import P2PNode, Peer, PeerInfo
from sdk.network.sync import SyncManager, SyncStatus
from sdk.blockchain.block import Block, BlockHeader
from sdk.blockchain.state import StateDB
from sdk.blockchain.validation import BlockValidator


class TestMessageProtocol:
    """Test message encoding and decoding"""
    
    def test_message_encode_decode(self):
        """Test basic message encoding and decoding"""
        # Create a message
        original = Message(MessageType.PING, b'test payload')
        
        # Encode
        encoded = MessageCodec.encode(original)
        
        # Check format
        assert len(encoded) >= MessageCodec.HEADER_SIZE
        
        # Decode
        decoded = MessageCodec.decode(encoded)
        
        # Verify
        assert decoded.type == MessageType.PING
        assert decoded.payload == b'test payload'
    
    def test_hello_message_encode_decode(self):
        """Test HelloMessage encoding and decoding"""
        # Create hello message
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
        
        # Encode
        encoded = MessageCodec.encode_hello(hello)
        
        # Decode
        msg = MessageCodec.decode(encoded)
        decoded_hello = MessageCodec.decode_hello(msg)
        
        # Verify
        assert decoded_hello.protocol_version == 1
        assert decoded_hello.network_id == 1
        assert decoded_hello.genesis_hash == b'\x00' * 32
        assert decoded_hello.best_height == 100
        assert decoded_hello.listen_port == 30303
        assert decoded_hello.capabilities == ['sync', 'relay']
    
    def test_get_blocks_message_encode_decode(self):
        """Test GetBlocksMessage encoding and decoding"""
        # Create message
        get_blocks = GetBlocksMessage(
            start_height=100,
            end_height=200,
            max_blocks=50
        )
        
        # Encode
        encoded = MessageCodec.encode_get_blocks(get_blocks)
        
        # Decode
        msg = MessageCodec.decode(encoded)
        decoded = MessageCodec.decode_get_blocks(msg)
        
        # Verify
        assert decoded.start_height == 100
        assert decoded.end_height == 200
        assert decoded.max_blocks == 50
    
    def test_get_headers_message_encode_decode(self):
        """Test GetHeadersMessage encoding and decoding"""
        # Create message
        get_headers = GetHeadersMessage(
            start_height=50,
            max_headers=100
        )
        
        # Encode
        encoded = MessageCodec.encode_get_headers(get_headers)
        
        # Decode
        msg = MessageCodec.decode(encoded)
        decoded = MessageCodec.decode_get_headers(msg)
        
        # Verify
        assert decoded.start_height == 50
        assert decoded.max_headers == 100
    
    def test_peers_message_encode_decode(self):
        """Test PeersMessage encoding and decoding"""
        # Create message
        peers = PeersMessage(
            peers=[
                {'address': '127.0.0.1', 'port': 30303, 'node_id': 'abc123'},
                {'address': '127.0.0.2', 'port': 30304, 'node_id': 'def456'}
            ]
        )
        
        # Encode
        encoded = MessageCodec.encode_peers(peers)
        
        # Decode
        msg = MessageCodec.decode(encoded)
        decoded = MessageCodec.decode_peers(msg)
        
        # Verify
        assert len(decoded.peers) == 2
        assert decoded.peers[0]['address'] == '127.0.0.1'
        assert decoded.peers[1]['port'] == 30304
    
    def test_ping_pong_messages(self):
        """Test PING and PONG messages"""
        # Encode ping
        ping = MessageCodec.encode_ping()
        
        # Decode
        msg = MessageCodec.decode(ping)
        assert msg.type == MessageType.PING
        assert msg.payload == b''
        
        # Encode pong
        pong = MessageCodec.encode_pong()
        
        # Decode
        msg = MessageCodec.decode(pong)
        assert msg.type == MessageType.PONG
        assert msg.payload == b''
    
    def test_disconnect_message(self):
        """Test DISCONNECT message"""
        # Encode
        reason = "Test disconnect"
        encoded = MessageCodec.encode_disconnect(reason)
        
        # Decode
        msg = MessageCodec.decode(encoded)
        assert msg.type == MessageType.DISCONNECT
        assert msg.payload.decode('utf-8') == reason
    
    def test_invalid_message(self):
        """Test handling of invalid messages"""
        with pytest.raises(ValueError):
            # Too short
            MessageCodec.decode(b'abc')
        
        with pytest.raises(ValueError):
            # Invalid type
            invalid = MessageCodec.encode(Message(MessageType(0xFF), b'test'))
            # Manually corrupt the type byte
            corrupted = invalid[:4] + b'\xFF' + invalid[5:]
            MessageCodec.decode(corrupted)


class TestP2PNode:
    """Test P2P node functionality"""
    
    @pytest.fixture
    def node_id(self):
        """Generate a test node ID"""
        return b'\x01' * 32
    
    @pytest.fixture
    def p2p_node(self, node_id):
        """Create a P2P node for testing"""
        return P2PNode(
            listen_port=30303,
            bootstrap_nodes=[],
            node_id=node_id,
            protocol_version=1,
            network_id=1,
            max_peers=10
        )
    
    def test_p2p_node_initialization(self, p2p_node):
        """Test P2P node initialization"""
        assert p2p_node.listen_port == 30303
        assert p2p_node.node_id == b'\x01' * 32
        assert p2p_node.max_peers == 10
        assert len(p2p_node.peers) == 0
        assert not p2p_node.running
    
    def test_peer_info(self):
        """Test PeerInfo dataclass"""
        peer_info = PeerInfo(
            address="127.0.0.1",
            port=30303,
            node_id=b'\x02' * 32,
            best_height=100,
            best_hash=b'\xff' * 32,
            capabilities=['sync']
        )
        
        assert peer_info.address == "127.0.0.1"
        assert peer_info.port == 30303
        assert peer_info.best_height == 100
        assert 'sync' in peer_info.capabilities
    
    def test_register_handler(self, p2p_node):
        """Test registering message handlers"""
        # Define a handler
        async def test_handler(peer, msg):
            pass
        
        # Register
        p2p_node.register_handler(MessageType.PING, test_handler)
        
        # Verify
        assert MessageType.PING in p2p_node.message_handlers
        assert p2p_node.message_handlers[MessageType.PING] == test_handler
    
    def test_get_best_peer(self, p2p_node):
        """Test getting best peer"""
        # No peers
        assert p2p_node.get_best_peer() is None
        
        # Add mock peers with different heights
        peer1 = Mock()
        peer1.height = 100
        peer1.peer_info = PeerInfo("127.0.0.1", 30303, b'\x01' * 32, best_height=100)
        
        peer2 = Mock()
        peer2.height = 200
        peer2.peer_info = PeerInfo("127.0.0.2", 30303, b'\x02' * 32, best_height=200)
        
        p2p_node.peers = {
            "127.0.0.1:30303": peer1,
            "127.0.0.2:30303": peer2
        }
        
        # Get best
        best = p2p_node.get_best_peer()
        assert best == peer2
        assert best.height == 200
    
    def test_get_random_peer(self, p2p_node):
        """Test getting random peer"""
        # No peers
        assert p2p_node.get_random_peer() is None
        
        # Add mock peer
        peer = Mock()
        p2p_node.peers = {"127.0.0.1:30303": peer}
        
        # Get random
        random_peer = p2p_node.get_random_peer()
        assert random_peer == peer


class TestSyncManager:
    """Test sync manager functionality"""
    
    @pytest.fixture
    def state_db(self, tmp_path):
        """Create a temporary state database"""
        return StateDB(str(tmp_path / "state"))
    
    @pytest.fixture
    def validator(self, state_db):
        """Create a block validator"""
        from sdk.blockchain.validation import ChainConfig
        config = ChainConfig(
            chain_id=1,
            block_time=12,
            block_gas_limit=10000000,
            min_gas_price=1
        )
        return BlockValidator(state_db, config)
    
    @pytest.fixture
    def p2p_node(self):
        """Create a mock P2P node"""
        node = Mock(spec=P2PNode)
        node.peers = {}
        node.register_handler = Mock()
        return node
    
    @pytest.fixture
    def sync_manager(self, p2p_node, validator):
        """Create a sync manager"""
        return SyncManager(p2p_node, validator)
    
    def test_sync_manager_initialization(self, sync_manager):
        """Test sync manager initialization"""
        assert not sync_manager.status.syncing
        assert sync_manager.status.current_height == 0
        assert sync_manager.status.target_height == 0
    
    def test_sync_status(self):
        """Test sync status tracking"""
        status = SyncStatus(
            syncing=True,
            start_height=0,
            current_height=500,
            target_height=1000,
            start_time=time.time() - 10,
            blocks_downloaded=500
        )
        
        # Check progress
        assert status.progress == 50.0
        
        # Check speed
        assert status.blocks_per_second > 0
        
        # Check elapsed time
        assert status.elapsed_time >= 10.0
    
    @pytest.mark.asyncio
    async def test_handle_new_block(self, sync_manager):
        """Test handling new block"""
        # Create a mock block
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
            gas_used=0,
            gas_limit=10000000
        )
        block = Block(header=header, transactions=[])
        
        # Mock peer
        peer = Mock()
        peer.address = "127.0.0.1:30303"
        
        # Mock validator to accept block
        sync_manager.validator.validate_block = Mock(return_value=True)
        
        # Callback to track
        blocks_synced = []
        sync_manager.on_block_synced = lambda b: blocks_synced.append(b)
        
        # Handle block
        await sync_manager.handle_new_block(block, peer)
        
        # Verify
        assert sync_manager.status.current_height == 1
        assert len(blocks_synced) == 1
        assert blocks_synced[0] == block


class TestIntegration:
    """Integration tests for network layer"""
    
    @pytest.mark.asyncio
    async def test_message_round_trip(self):
        """Test full message encode/decode round trip"""
        # Test all message types
        messages = [
            MessageCodec.encode_ping(),
            MessageCodec.encode_pong(),
            MessageCodec.encode_disconnect("test"),
        ]
        
        for encoded in messages:
            decoded = MessageCodec.decode(encoded)
            assert decoded is not None
            assert isinstance(decoded.type, MessageType)
    
    def test_peer_discovery_flow(self):
        """Test peer discovery message flow"""
        # Node requests peers
        get_peers = GetPeersMessage(max_peers=10)
        encoded_request = MessageCodec.encode_get_peers(get_peers)
        
        # Decode request
        msg = MessageCodec.decode(encoded_request)
        decoded_request = MessageCodec.decode_get_peers(msg)
        assert decoded_request.max_peers == 10
        
        # Node responds with peers
        peers = PeersMessage(peers=[
            {'address': '10.0.0.1', 'port': 30303, 'node_id': 'abc'},
            {'address': '10.0.0.2', 'port': 30304, 'node_id': 'def'}
        ])
        encoded_response = MessageCodec.encode_peers(peers)
        
        # Decode response
        msg = MessageCodec.decode(encoded_response)
        decoded_response = MessageCodec.decode_peers(msg)
        assert len(decoded_response.peers) == 2


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
