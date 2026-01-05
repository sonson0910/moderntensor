"""
Network optimization module.

Provides:
- Connection pooling
- Message compression
- Bandwidth optimization
"""
from typing import Dict, Any, List
import zlib
import time


class NetworkOptimizer:
    """
    Optimizes network layer for better performance.
    
    Features:
    1. Connection Pooling - Reuse connections
    2. Message Compression - Reduce bandwidth usage
    3. Bandwidth Optimization - Rate limiting and prioritization
    """
    
    def __init__(self, max_connections: int = 100, enable_compression: bool = True):
        """Initialize network optimizer."""
        self.max_connections = max_connections
        self.enable_compression = enable_compression
        self.connection_pool = {}
        self.stats = {
            'messages_compressed': 0,
            'bytes_saved': 0,
            'connections_reused': 0,
        }
    
    def compress_message(self, message: bytes) -> bytes:
        """
        Compress message data for network transmission.
        
        Args:
            message: Raw message bytes
            
        Returns:
            bytes: Compressed message
        """
        if not self.enable_compression or len(message) < 256:
            return message  # Don't compress small messages
        
        compressed = zlib.compress(message, level=6)
        
        # Only use compression if it saves space
        if len(compressed) < len(message):
            self.stats['messages_compressed'] += 1
            self.stats['bytes_saved'] += len(message) - len(compressed)
            return compressed
        
        return message
    
    def decompress_message(self, message: bytes) -> bytes:
        """Decompress received message."""
        try:
            return zlib.decompress(message)
        except:
            return message  # Not compressed
    
    def get_connection(self, peer_id: str) -> Any:
        """
        Get connection from pool or create new one.
        
        Args:
            peer_id: Peer identifier
            
        Returns:
            Connection object
        """
        if peer_id in self.connection_pool:
            self.stats['connections_reused'] += 1
            return self.connection_pool[peer_id]
        
        # Create new connection (placeholder)
        connection = None  # Would create actual connection
        self.connection_pool[peer_id] = connection
        return connection
    
    def optimize_bandwidth(self, messages: List[Dict]) -> List[Dict]:
        """
        Prioritize and batch messages for optimal bandwidth usage.
        
        Args:
            messages: List of messages to send
            
        Returns:
            List of prioritized messages
        """
        # Priority levels:
        # 1. Block announcements (critical)
        # 2. Transactions (high)
        # 3. Peer discovery (medium)
        # 4. Heartbeats (low)
        
        priority_map = {
            'block': 0,
            'transaction': 1,
            'peers': 2,
            'ping': 3,
        }
        
        # Sort by priority
        sorted_messages = sorted(messages, 
                                key=lambda m: priority_map.get(m.get('type', 'ping'), 99))
        
        return sorted_messages
    
    def get_stats(self) -> Dict[str, Any]:
        """Get network optimizer statistics."""
        return dict(self.stats)
