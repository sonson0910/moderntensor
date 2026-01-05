"""
Peer-to-peer networking for ModernTensor Layer 1 blockchain.

This module implements the P2P protocol for node communication,
including peer management, transaction/block broadcasting, and peer discovery.
"""

import asyncio
import logging
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Set, Callable
import random

from sdk.blockchain.block import Block
from sdk.blockchain.transaction import Transaction
from sdk.network.messages import (
    Message, MessageType, MessageCodec,
    HelloMessage, GetBlocksMessage, GetHeadersMessage,
    GetPeersMessage, PeersMessage
)

logger = logging.getLogger(__name__)


@dataclass
class PeerInfo:
    """Information about a peer"""
    
    address: str
    port: int
    node_id: bytes
    best_height: int = 0
    best_hash: Optional[bytes] = None
    last_seen: float = field(default_factory=time.time)
    capabilities: List[str] = field(default_factory=list)
    
    def __hash__(self):
        return hash((self.address, self.port))
    
    def __eq__(self, other):
        if not isinstance(other, PeerInfo):
            return False
        return self.address == other.address and self.port == other.port


class Peer:
    """Represents a connected peer"""
    
    def __init__(self, 
                 reader: asyncio.StreamReader,
                 writer: asyncio.StreamWriter,
                 peer_info: Optional[PeerInfo] = None):
        self.reader = reader
        self.writer = writer
        self.peer_info = peer_info
        self.connected = True
        self.last_ping = time.time()
        self.latency = 0.0
        
    @property
    def address(self) -> str:
        """Get peer address"""
        if self.peer_info:
            return f"{self.peer_info.address}:{self.peer_info.port}"
        addr = self.writer.get_extra_info('peername')
        if addr:
            return f"{addr[0]}:{addr[1]}"
        return "unknown"
    
    @property
    def height(self) -> int:
        """Get peer's best block height"""
        return self.peer_info.best_height if self.peer_info else 0
    
    @property
    def best_hash(self) -> Optional[bytes]:
        """Get peer's best block hash"""
        return self.peer_info.best_hash if self.peer_info else None
    
    async def send_message(self, msg_bytes: bytes):
        """
        Send message to peer.
        
        Args:
            msg_bytes: Encoded message bytes
        """
        if not self.connected:
            raise ConnectionError("Peer is not connected")
        
        try:
            self.writer.write(msg_bytes)
            await self.writer.drain()
        except Exception as e:
            logger.error(f"Failed to send message to {self.address}: {e}")
            self.connected = False
            raise
    
    async def receive_message(self) -> Optional[Message]:
        """
        Receive message from peer.
        
        Returns:
            Decoded Message or None if connection closed
        """
        try:
            # Read header (5 bytes)
            header = await self.reader.readexactly(MessageCodec.HEADER_SIZE)
            if not header:
                return None
            
            # Decode header to get payload length
            import struct
            total_length, msg_type = struct.unpack('!IB', header)
            
            # Read payload
            payload = await self.reader.readexactly(total_length - 1)
            
            # Reconstruct and decode message
            full_data = header + payload
            return MessageCodec.decode(full_data)
            
        except asyncio.IncompleteReadError:
            logger.info(f"Peer {self.address} closed connection")
            self.connected = False
            return None
        except Exception as e:
            logger.error(f"Error receiving message from {self.address}: {e}")
            self.connected = False
            return None
    
    async def disconnect(self, reason: str = ""):
        """Disconnect from peer"""
        if self.connected:
            try:
                # Send disconnect message
                disconnect_msg = MessageCodec.encode_disconnect(reason)
                await self.send_message(disconnect_msg)
            except:
                pass
            
            self.connected = False
            self.writer.close()
            await self.writer.wait_closed()
            logger.info(f"Disconnected from {self.address}: {reason}")


class P2PNode:
    """Peer-to-peer network node"""
    
    def __init__(self,
                 listen_port: int,
                 bootstrap_nodes: List[str],
                 node_id: bytes,
                 protocol_version: int = 1,
                 network_id: int = 1,
                 max_peers: int = 50):
        """
        Initialize P2P node.
        
        Args:
            listen_port: Port to listen on
            bootstrap_nodes: List of bootstrap node addresses (host:port)
            node_id: Unique node identifier
            protocol_version: Protocol version
            network_id: Network ID (mainnet, testnet, etc.)
            max_peers: Maximum number of peers to connect to
        """
        self.listen_port = listen_port
        self.bootstrap_nodes = bootstrap_nodes
        self.node_id = node_id
        self.protocol_version = protocol_version
        self.network_id = network_id
        self.max_peers = max_peers
        
        # Connected peers
        self.peers: Dict[str, Peer] = {}
        
        # Known peers (for discovery)
        self.known_peers: Set[PeerInfo] = set()
        
        # Genesis hash (set by blockchain)
        self.genesis_hash: Optional[bytes] = None
        self.best_height: int = 0
        self.best_hash: Optional[bytes] = None
        
        # Message handlers
        self.message_handlers: Dict[MessageType, Callable] = {}
        
        # Server
        self.server: Optional[asyncio.Server] = None
        self.running = False
        
        # Tasks
        self.tasks: List[asyncio.Task] = []
        
        logger.info(f"P2P Node initialized on port {listen_port}")
    
    def register_handler(self, msg_type: MessageType, handler: Callable):
        """Register a handler for a message type"""
        self.message_handlers[msg_type] = handler
        logger.debug(f"Registered handler for {msg_type}")
    
    async def start(self):
        """Start P2P server and connect to bootstrap nodes"""
        if self.running:
            logger.warning("P2P node already running")
            return
        
        self.running = True
        
        # Start listening server
        self.server = await asyncio.start_server(
            self._handle_incoming_connection,
            '0.0.0.0',
            self.listen_port
        )
        
        logger.info(f"P2P server listening on port {self.listen_port}")
        
        # Connect to bootstrap nodes
        for node_addr in self.bootstrap_nodes:
            asyncio.create_task(self._connect_to_bootstrap(node_addr))
        
        # Start background tasks
        self.tasks.append(asyncio.create_task(self._peer_maintenance_loop()))
        self.tasks.append(asyncio.create_task(self._peer_discovery_loop()))
        
        logger.info("P2P node started")
    
    async def stop(self):
        """Stop P2P server and disconnect all peers"""
        if not self.running:
            return
        
        self.running = False
        
        # Stop server
        if self.server:
            self.server.close()
            await self.server.wait_closed()
        
        # Disconnect all peers
        for peer in list(self.peers.values()):
            await peer.disconnect("Node shutting down")
        
        # Cancel background tasks
        for task in self.tasks:
            task.cancel()
        
        logger.info("P2P node stopped")
    
    async def connect_peer(self, address: str, port: int) -> Optional[Peer]:
        """
        Connect to a peer.
        
        Args:
            address: Peer address
            port: Peer port
            
        Returns:
            Connected Peer object or None if failed
        """
        peer_addr = f"{address}:{port}"
        
        # Check if already connected
        if peer_addr in self.peers:
            logger.debug(f"Already connected to {peer_addr}")
            return self.peers[peer_addr]
        
        # Check max peers
        if len(self.peers) >= self.max_peers:
            logger.debug("Max peers reached, not connecting")
            return None
        
        try:
            # Connect
            reader, writer = await asyncio.wait_for(
                asyncio.open_connection(address, port),
                timeout=10.0
            )
            
            peer = Peer(reader, writer)
            
            # Send HELLO
            hello = HelloMessage(
                protocol_version=self.protocol_version,
                network_id=self.network_id,
                genesis_hash=self.genesis_hash or b'\x00' * 32,
                best_height=self.best_height,
                best_hash=self.best_hash or b'\x00' * 32,
                listen_port=self.listen_port,
                node_id=self.node_id,
                capabilities=['sync', 'relay']
            )
            
            hello_msg = MessageCodec.encode_hello(hello)
            await peer.send_message(hello_msg)
            
            # Wait for HELLO response
            response = await asyncio.wait_for(peer.receive_message(), timeout=10.0)
            
            if response and response.type == MessageType.HELLO:
                hello_response = MessageCodec.decode_hello(response)
                
                # Validate network compatibility
                if hello_response.network_id != self.network_id:
                    await peer.disconnect("Different network")
                    return None
                
                # Create peer info
                peer.peer_info = PeerInfo(
                    address=address,
                    port=hello_response.listen_port,
                    node_id=hello_response.node_id,
                    best_height=hello_response.best_height,
                    best_hash=hello_response.best_hash,
                    capabilities=hello_response.capabilities
                )
                
                # Add to peers
                self.peers[peer_addr] = peer
                self.known_peers.add(peer.peer_info)
                
                # Start message handler
                asyncio.create_task(self._handle_peer_messages(peer))
                
                logger.info(f"Connected to peer {peer_addr} (height: {peer.height})")
                return peer
            else:
                await peer.disconnect("Invalid handshake")
                return None
                
        except Exception as e:
            logger.error(f"Failed to connect to {peer_addr}: {e}")
            return None
    
    async def disconnect_peer(self, peer_addr: str, reason: str = ""):
        """Disconnect from a peer"""
        if peer_addr in self.peers:
            peer = self.peers[peer_addr]
            await peer.disconnect(reason)
            del self.peers[peer_addr]
    
    async def broadcast_transaction(self, tx: Transaction):
        """
        Broadcast transaction to all peers.
        
        Args:
            tx: Transaction to broadcast
        """
        import json
        
        # Serialize transaction
        tx_data = json.dumps({
            'nonce': tx.nonce,
            'from': tx.from_address.hex(),
            'to': tx.to_address.hex() if tx.to_address else None,
            'value': tx.value,
            'gas_price': tx.gas_price,
            'gas_limit': tx.gas_limit,
            'data': tx.data.hex(),
            'v': tx.v,
            'r': tx.r.hex(),
            's': tx.s.hex()
        }).encode('utf-8')
        
        msg = MessageCodec.encode(Message(MessageType.NEW_TRANSACTION, tx_data))
        
        # Broadcast to all connected peers
        for peer in self.peers.values():
            try:
                await peer.send_message(msg)
            except Exception as e:
                logger.error(f"Failed to broadcast tx to {peer.address}: {e}")
        
        logger.debug(f"Broadcasted transaction {tx.hash().hex()[:8]}...")
    
    async def broadcast_block(self, block: Block):
        """
        Broadcast new block to all peers.
        
        Args:
            block: Block to broadcast
        """
        import json
        
        # Serialize block header (lightweight announcement)
        block_data = json.dumps({
            'height': block.header.height,
            'hash': block.hash().hex(),
            'previous_hash': block.header.previous_hash.hex(),
            'timestamp': block.header.timestamp
        }).encode('utf-8')
        
        msg = MessageCodec.encode(Message(MessageType.NEW_BLOCK_HASHES, block_data))
        
        # Broadcast to all connected peers
        for peer in self.peers.values():
            try:
                await peer.send_message(msg)
            except Exception as e:
                logger.error(f"Failed to broadcast block to {peer.address}: {e}")
        
        logger.debug(f"Broadcasted block {block.header.height}")
    
    async def request_blocks(self, peer: Peer, start_height: int, end_height: int):
        """Request blocks from a peer"""
        get_blocks = GetBlocksMessage(
            start_height=start_height,
            end_height=end_height
        )
        msg = MessageCodec.encode_get_blocks(get_blocks)
        await peer.send_message(msg)
        logger.debug(f"Requested blocks {start_height}-{end_height} from {peer.address}")
    
    async def request_headers(self, peer: Peer, start_height: int, max_headers: int = 192):
        """Request block headers from a peer"""
        get_headers = GetHeadersMessage(
            start_height=start_height,
            max_headers=max_headers
        )
        msg = MessageCodec.encode_get_headers(get_headers)
        await peer.send_message(msg)
        logger.debug(f"Requested headers from {start_height} from {peer.address}")
    
    def get_best_peer(self) -> Optional[Peer]:
        """Get peer with highest block height"""
        if not self.peers:
            return None
        return max(self.peers.values(), key=lambda p: p.height)
    
    def get_random_peer(self) -> Optional[Peer]:
        """Get random connected peer"""
        if not self.peers:
            return None
        return random.choice(list(self.peers.values()))
    
    async def _handle_incoming_connection(self,
                                          reader: asyncio.StreamReader,
                                          writer: asyncio.StreamWriter):
        """Handle incoming peer connection"""
        addr = writer.get_extra_info('peername')
        logger.info(f"Incoming connection from {addr}")
        
        # Check max peers
        if len(self.peers) >= self.max_peers:
            logger.debug("Max peers reached, rejecting connection")
            writer.close()
            await writer.wait_closed()
            return
        
        peer = Peer(reader, writer)
        
        try:
            # Wait for HELLO
            hello_msg = await asyncio.wait_for(peer.receive_message(), timeout=10.0)
            
            if not hello_msg or hello_msg.type != MessageType.HELLO:
                await peer.disconnect("Expected HELLO")
                return
            
            hello = MessageCodec.decode_hello(hello_msg)
            
            # Validate network
            if hello.network_id != self.network_id:
                await peer.disconnect("Different network")
                return
            
            # Create peer info
            peer.peer_info = PeerInfo(
                address=addr[0],
                port=hello.listen_port,
                node_id=hello.node_id,
                best_height=hello.best_height,
                best_hash=hello.best_hash,
                capabilities=hello.capabilities
            )
            
            # Send HELLO response
            hello_response = HelloMessage(
                protocol_version=self.protocol_version,
                network_id=self.network_id,
                genesis_hash=self.genesis_hash or b'\x00' * 32,
                best_height=self.best_height,
                best_hash=self.best_hash or b'\x00' * 32,
                listen_port=self.listen_port,
                node_id=self.node_id,
                capabilities=['sync', 'relay']
            )
            
            response = MessageCodec.encode_hello(hello_response)
            await peer.send_message(response)
            
            # Add peer
            peer_addr = f"{addr[0]}:{addr[1]}"
            self.peers[peer_addr] = peer
            self.known_peers.add(peer.peer_info)
            
            # Start message handler
            asyncio.create_task(self._handle_peer_messages(peer))
            
            logger.info(f"Accepted connection from {peer_addr}")
            
        except Exception as e:
            logger.error(f"Error handling incoming connection: {e}")
            await peer.disconnect("Handshake failed")
    
    async def _handle_peer_messages(self, peer: Peer):
        """Handle messages from a peer"""
        while peer.connected and self.running:
            try:
                msg = await peer.receive_message()
                
                if msg is None:
                    break
                
                # Handle message
                if msg.type in self.message_handlers:
                    try:
                        await self.message_handlers[msg.type](peer, msg)
                    except Exception as e:
                        logger.error(f"Error in message handler for {msg.type}: {e}")
                elif msg.type == MessageType.PING:
                    # Respond to PING
                    pong = MessageCodec.encode_pong()
                    await peer.send_message(pong)
                elif msg.type == MessageType.PONG:
                    # Update latency
                    peer.latency = time.time() - peer.last_ping
                elif msg.type == MessageType.DISCONNECT:
                    reason = msg.payload.decode('utf-8') if msg.payload else ""
                    logger.info(f"Peer {peer.address} disconnected: {reason}")
                    break
                else:
                    logger.warning(f"Unhandled message type: {msg.type}")
                    
            except Exception as e:
                logger.error(f"Error handling message from {peer.address}: {e}")
                break
        
        # Clean up disconnected peer
        if peer.address in self.peers:
            del self.peers[peer.address]
        
        await peer.disconnect("Connection closed")
    
    async def _connect_to_bootstrap(self, node_addr: str):
        """Connect to a bootstrap node"""
        try:
            host, port = node_addr.split(':')
            port = int(port)
            await self.connect_peer(host, port)
        except Exception as e:
            logger.error(f"Failed to connect to bootstrap node {node_addr}: {e}")
    
    async def _peer_maintenance_loop(self):
        """Maintain peer connections"""
        while self.running:
            try:
                await asyncio.sleep(30)
                
                # Send PING to all peers
                for peer in list(self.peers.values()):
                    if peer.connected:
                        try:
                            ping = MessageCodec.encode_ping()
                            peer.last_ping = time.time()
                            await peer.send_message(ping)
                        except:
                            pass
                
                # Remove dead peers
                current_time = time.time()
                for peer_addr, peer in list(self.peers.items()):
                    if not peer.connected or current_time - peer.last_ping > 120:
                        logger.info(f"Removing dead peer {peer_addr}")
                        await self.disconnect_peer(peer_addr, "Timeout")
                
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in peer maintenance: {e}")
    
    async def _peer_discovery_loop(self):
        """Discover new peers"""
        while self.running:
            try:
                await asyncio.sleep(60)
                
                # Request peers from random peer
                peer = self.get_random_peer()
                if peer and peer.connected:
                    try:
                        get_peers = GetPeersMessage()
                        msg = MessageCodec.encode_get_peers(get_peers)
                        await peer.send_message(msg)
                    except:
                        pass
                
                # Try to connect to known peers if below max
                if len(self.peers) < self.max_peers and self.known_peers:
                    # Pick a random known peer not currently connected
                    available_peers = [
                        p for p in self.known_peers
                        if f"{p.address}:{p.port}" not in self.peers
                    ]
                    
                    if available_peers:
                        peer_info = random.choice(available_peers)
                        await self.connect_peer(peer_info.address, peer_info.port)
                
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in peer discovery: {e}")
