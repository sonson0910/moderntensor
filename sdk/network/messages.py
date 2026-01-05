"""
Network message protocol for ModernTensor Layer 1 blockchain.

This module defines the message types and serialization/deserialization
for communication between nodes in the P2P network.
"""

from dataclasses import dataclass
from enum import IntEnum
from typing import List, Optional
import json
import struct


class MessageType(IntEnum):
    """Message types for P2P communication"""
    
    # Handshake and ping
    HELLO = 0x00
    PING = 0x01
    PONG = 0x02
    DISCONNECT = 0x03
    
    # Blockchain sync
    GET_BLOCKS = 0x10
    BLOCKS = 0x11
    GET_HEADERS = 0x12
    HEADERS = 0x13
    GET_BLOCK_BODIES = 0x14
    BLOCK_BODIES = 0x15
    
    # Transaction/Block propagation
    NEW_TRANSACTION = 0x20
    NEW_BLOCK = 0x21
    NEW_BLOCK_HASHES = 0x22
    GET_TRANSACTIONS = 0x23
    TRANSACTIONS = 0x24
    
    # State sync
    GET_STATE = 0x30
    STATE = 0x31
    GET_STATE_NODES = 0x32
    STATE_NODES = 0x33
    
    # Peer discovery
    GET_PEERS = 0x40
    PEERS = 0x41


@dataclass
class Message:
    """Base message class for P2P communication"""
    
    type: MessageType
    payload: bytes
    
    def __post_init__(self):
        """Validate message after initialization"""
        if not isinstance(self.type, MessageType):
            self.type = MessageType(self.type)


@dataclass
class HelloMessage:
    """Handshake message sent when connecting to a peer"""
    
    protocol_version: int
    network_id: int
    genesis_hash: bytes
    best_height: int
    best_hash: bytes
    listen_port: int
    node_id: bytes
    capabilities: List[str]
    
    def to_dict(self) -> dict:
        """Convert to dictionary for serialization"""
        return {
            'protocol_version': self.protocol_version,
            'network_id': self.network_id,
            'genesis_hash': self.genesis_hash.hex(),
            'best_height': self.best_height,
            'best_hash': self.best_hash.hex(),
            'listen_port': self.listen_port,
            'node_id': self.node_id.hex(),
            'capabilities': self.capabilities
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> 'HelloMessage':
        """Create from dictionary"""
        return cls(
            protocol_version=data['protocol_version'],
            network_id=data['network_id'],
            genesis_hash=bytes.fromhex(data['genesis_hash']),
            best_height=data['best_height'],
            best_hash=bytes.fromhex(data['best_hash']),
            listen_port=data['listen_port'],
            node_id=bytes.fromhex(data['node_id']),
            capabilities=data['capabilities']
        )


@dataclass
class GetBlocksMessage:
    """Request blocks from a peer"""
    
    start_height: int
    end_height: int
    max_blocks: int = 128
    
    def to_dict(self) -> dict:
        """Convert to dictionary for serialization"""
        return {
            'start_height': self.start_height,
            'end_height': self.end_height,
            'max_blocks': self.max_blocks
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> 'GetBlocksMessage':
        """Create from dictionary"""
        return cls(
            start_height=data['start_height'],
            end_height=data['end_height'],
            max_blocks=data.get('max_blocks', 128)
        )


@dataclass
class GetHeadersMessage:
    """Request block headers from a peer"""
    
    start_height: int
    max_headers: int = 192
    
    def to_dict(self) -> dict:
        """Convert to dictionary for serialization"""
        return {
            'start_height': self.start_height,
            'max_headers': self.max_headers
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> 'GetHeadersMessage':
        """Create from dictionary"""
        return cls(
            start_height=data['start_height'],
            max_headers=data.get('max_headers', 192)
        )


@dataclass
class GetPeersMessage:
    """Request peer list from a peer"""
    
    max_peers: int = 50
    
    def to_dict(self) -> dict:
        """Convert to dictionary for serialization"""
        return {
            'max_peers': self.max_peers
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> 'GetPeersMessage':
        """Create from dictionary"""
        return cls(
            max_peers=data.get('max_peers', 50)
        )


@dataclass
class PeersMessage:
    """Response with list of known peers"""
    
    peers: List[dict]  # Each dict contains: {'address': str, 'port': int, 'node_id': str}
    
    def to_dict(self) -> dict:
        """Convert to dictionary for serialization"""
        return {
            'peers': self.peers
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> 'PeersMessage':
        """Create from dictionary"""
        return cls(
            peers=data['peers']
        )


class MessageCodec:
    """Encode/decode messages for network transmission"""
    
    # Message format: [length: 4 bytes][type: 1 byte][payload: N bytes]
    HEADER_SIZE = 5
    MAX_MESSAGE_SIZE = 10 * 1024 * 1024  # 10 MB
    
    @staticmethod
    def encode(msg: Message) -> bytes:
        """
        Serialize message to bytes.
        
        Format: [length: 4 bytes][type: 1 byte][payload: N bytes]
        
        Args:
            msg: Message to encode
            
        Returns:
            Encoded message as bytes
        """
        # Create header: length (4 bytes) + type (1 byte)
        msg_type = int(msg.type)
        payload = msg.payload
        
        # Calculate total length (type + payload)
        total_length = 1 + len(payload)
        
        # Pack header
        header = struct.pack('!IB', total_length, msg_type)
        
        # Combine header and payload
        return header + payload
    
    @staticmethod
    def decode(data: bytes) -> Message:
        """
        Deserialize message from bytes.
        
        Args:
            data: Raw bytes to decode
            
        Returns:
            Decoded Message object
            
        Raises:
            ValueError: If data is invalid or corrupted
        """
        if len(data) < MessageCodec.HEADER_SIZE:
            raise ValueError(f"Message too short: {len(data)} bytes")
        
        # Unpack header
        total_length, msg_type = struct.unpack('!IB', data[:MessageCodec.HEADER_SIZE])
        
        # Validate length
        if len(data) != MessageCodec.HEADER_SIZE + total_length - 1:
            raise ValueError(
                f"Message length mismatch: expected {total_length}, "
                f"got {len(data) - MessageCodec.HEADER_SIZE + 1}"
            )
        
        # Check max size
        if total_length > MessageCodec.MAX_MESSAGE_SIZE:
            raise ValueError(f"Message too large: {total_length} bytes")
        
        # Extract payload
        payload = data[MessageCodec.HEADER_SIZE:]
        
        try:
            message_type = MessageType(msg_type)
        except ValueError:
            raise ValueError(f"Unknown message type: {msg_type}")
        
        return Message(type=message_type, payload=payload)
    
    @staticmethod
    def encode_hello(hello: HelloMessage) -> bytes:
        """Encode HelloMessage"""
        payload = json.dumps(hello.to_dict()).encode('utf-8')
        return MessageCodec.encode(Message(MessageType.HELLO, payload))
    
    @staticmethod
    def decode_hello(msg: Message) -> HelloMessage:
        """Decode HelloMessage"""
        if msg.type != MessageType.HELLO:
            raise ValueError(f"Expected HELLO message, got {msg.type}")
        data = json.loads(msg.payload.decode('utf-8'))
        return HelloMessage.from_dict(data)
    
    @staticmethod
    def encode_get_blocks(get_blocks: GetBlocksMessage) -> bytes:
        """Encode GetBlocksMessage"""
        payload = json.dumps(get_blocks.to_dict()).encode('utf-8')
        return MessageCodec.encode(Message(MessageType.GET_BLOCKS, payload))
    
    @staticmethod
    def decode_get_blocks(msg: Message) -> GetBlocksMessage:
        """Decode GetBlocksMessage"""
        if msg.type != MessageType.GET_BLOCKS:
            raise ValueError(f"Expected GET_BLOCKS message, got {msg.type}")
        data = json.loads(msg.payload.decode('utf-8'))
        return GetBlocksMessage.from_dict(data)
    
    @staticmethod
    def encode_get_headers(get_headers: GetHeadersMessage) -> bytes:
        """Encode GetHeadersMessage"""
        payload = json.dumps(get_headers.to_dict()).encode('utf-8')
        return MessageCodec.encode(Message(MessageType.GET_HEADERS, payload))
    
    @staticmethod
    def decode_get_headers(msg: Message) -> GetHeadersMessage:
        """Decode GetHeadersMessage"""
        if msg.type != MessageType.GET_HEADERS:
            raise ValueError(f"Expected GET_HEADERS message, got {msg.type}")
        data = json.loads(msg.payload.decode('utf-8'))
        return GetHeadersMessage.from_dict(data)
    
    @staticmethod
    def encode_get_peers(get_peers: GetPeersMessage) -> bytes:
        """Encode GetPeersMessage"""
        payload = json.dumps(get_peers.to_dict()).encode('utf-8')
        return MessageCodec.encode(Message(MessageType.GET_PEERS, payload))
    
    @staticmethod
    def decode_get_peers(msg: Message) -> GetPeersMessage:
        """Decode GetPeersMessage"""
        if msg.type != MessageType.GET_PEERS:
            raise ValueError(f"Expected GET_PEERS message, got {msg.type}")
        data = json.loads(msg.payload.decode('utf-8'))
        return GetPeersMessage.from_dict(data)
    
    @staticmethod
    def encode_peers(peers: PeersMessage) -> bytes:
        """Encode PeersMessage"""
        payload = json.dumps(peers.to_dict()).encode('utf-8')
        return MessageCodec.encode(Message(MessageType.PEERS, payload))
    
    @staticmethod
    def decode_peers(msg: Message) -> PeersMessage:
        """Decode PeersMessage"""
        if msg.type != MessageType.PEERS:
            raise ValueError(f"Expected PEERS message, got {msg.type}")
        data = json.loads(msg.payload.decode('utf-8'))
        return PeersMessage.from_dict(data)
    
    @staticmethod
    def encode_ping() -> bytes:
        """Encode PING message"""
        return MessageCodec.encode(Message(MessageType.PING, b''))
    
    @staticmethod
    def encode_pong() -> bytes:
        """Encode PONG message"""
        return MessageCodec.encode(Message(MessageType.PONG, b''))
    
    @staticmethod
    def encode_disconnect(reason: str = "") -> bytes:
        """Encode DISCONNECT message"""
        payload = reason.encode('utf-8')
        return MessageCodec.encode(Message(MessageType.DISCONNECT, payload))
