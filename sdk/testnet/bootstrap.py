"""
Bootstrap Node

This module provides bootstrap node functionality for testnet deployment.
Bootstrap nodes help new nodes discover and connect to the network.
"""

import asyncio
import json
from dataclasses import dataclass, field
from typing import Dict, List, Set, Optional
from pathlib import Path
import time


@dataclass
class PeerInfo:
    """Information about a peer"""
    node_id: str
    address: str
    port: int
    last_seen: float
    version: str = "1.0.0"
    chain_id: int = 9999
    
    def to_dict(self) -> Dict:
        return {
            'node_id': self.node_id,
            'address': self.address,
            'port': self.port,
            'last_seen': self.last_seen,
            'version': self.version,
            'chain_id': self.chain_id
        }
    
    @classmethod
    def from_dict(cls, data: Dict) -> 'PeerInfo':
        return cls(**data)


@dataclass
class BootstrapConfig:
    """Configuration for bootstrap node"""
    listen_address: str = "0.0.0.0"
    listen_port: int = 30303
    max_peers: int = 1000
    peer_timeout: int = 3600  # 1 hour
    announce_interval: int = 300  # 5 minutes
    
    # Network info
    chain_id: int = 9999
    network_name: str = "moderntensor-testnet"
    
    # Bootstrap node identity
    node_id: Optional[str] = None
    public_address: Optional[str] = None


class BootstrapNode:
    """
    Bootstrap node for ModernTensor testnet
    
    Helps new nodes discover and connect to the network by maintaining
    a list of active peers and providing peer discovery services.
    """
    
    def __init__(self, config: Optional[BootstrapConfig] = None):
        self.config = config or BootstrapConfig()
        self.peers: Dict[str, PeerInfo] = {}
        self.running = False
        self.stats = {
            'total_peers_seen': 0,
            'active_peers': 0,
            'discovery_requests': 0,
            'announcements': 0,
            'uptime_start': time.time()
        }
    
    async def start(self):
        """Start the bootstrap node"""
        self.running = True
        print(f"🚀 Bootstrap node starting...")
        print(f"   Listen: {self.config.listen_address}:{self.config.listen_port}")
        print(f"   Network: {self.config.network_name} (Chain ID: {self.config.chain_id})")
        print(f"   Max peers: {self.config.max_peers}")
        
        # Start background tasks
        asyncio.create_task(self._cleanup_stale_peers())
        asyncio.create_task(self._periodic_stats())
        
        print(f"✅ Bootstrap node running")
    
    async def stop(self):
        """Stop the bootstrap node"""
        self.running = False
        print("🛑 Bootstrap node stopped")
    
    def register_peer(
        self,
        node_id: str,
        address: str,
        port: int,
        version: str = "1.0.0"
    ) -> bool:
        """
        Register a peer with the bootstrap node
        
        Args:
            node_id: Unique node identifier
            address: Node's IP address
            port: Node's P2P port
            version: Node's software version
        
        Returns:
            bool: True if registration successful
        """
        # Check if we've reached max peers
        if len(self.peers) >= self.config.max_peers and node_id not in self.peers:
            return False
        
        # Check if this is a new peer
        is_new = node_id not in self.peers
        
        peer = PeerInfo(
            node_id=node_id,
            address=address,
            port=port,
            last_seen=time.time(),
            version=version,
            chain_id=self.config.chain_id
        )
        
        self.peers[node_id] = peer
        self.stats['announcements'] += 1
        
        if is_new:
            self.stats['total_peers_seen'] += 1
            print(f"📡 New peer registered: {node_id[:8]}... from {address}:{port}")
        
        self._update_active_peers_count()
        
        return True
    
    def get_peers(
        self,
        max_count: int = 10,
        exclude: Optional[Set[str]] = None
    ) -> List[PeerInfo]:
        """
        Get a list of active peers for discovery
        
        Args:
            max_count: Maximum number of peers to return
            exclude: Set of node IDs to exclude
        
        Returns:
            List of PeerInfo objects
        """
        self.stats['discovery_requests'] += 1
        
        exclude = exclude or set()
        current_time = time.time()
        
        # Get active peers
        active_peers = [
            peer for peer in self.peers.values()
            if (current_time - peer.last_seen < self.config.peer_timeout
                and peer.node_id not in exclude)
        ]
        
        # Sort by last seen (most recent first)
        active_peers.sort(key=lambda p: p.last_seen, reverse=True)
        
        return active_peers[:max_count]
    
    def get_peer(self, node_id: str) -> Optional[PeerInfo]:
        """Get information about a specific peer"""
        return self.peers.get(node_id)
    
    def remove_peer(self, node_id: str):
        """Remove a peer from the registry"""
        if node_id in self.peers:
            del self.peers[node_id]
            self._update_active_peers_count()
            print(f"🗑️  Peer removed: {node_id[:8]}...")
    
    async def _cleanup_stale_peers(self):
        """Background task to clean up stale peers"""
        while self.running:
            current_time = time.time()
            stale_peers = [
                node_id for node_id, peer in self.peers.items()
                if current_time - peer.last_seen > self.config.peer_timeout
            ]
            
            for node_id in stale_peers:
                self.remove_peer(node_id)
            
            if stale_peers:
                print(f"🧹 Cleaned up {len(stale_peers)} stale peers")
            
            # Run cleanup every 5 minutes
            await asyncio.sleep(300)
    
    async def _periodic_stats(self):
        """Background task to log periodic statistics"""
        while self.running:
            await asyncio.sleep(600)  # Every 10 minutes
            stats = self.get_stats()
            print(f"📊 Stats: {stats['active_peers']} active peers, "
                  f"{stats['total_peers_seen']} total seen, "
                  f"{stats['discovery_requests']} discovery requests")
    
    def _update_active_peers_count(self):
        """Update the count of active peers"""
        current_time = time.time()
        self.stats['active_peers'] = sum(
            1 for peer in self.peers.values()
            if current_time - peer.last_seen < self.config.peer_timeout
        )
    
    def get_stats(self) -> Dict:
        """Get bootstrap node statistics"""
        self._update_active_peers_count()
        uptime = time.time() - self.stats['uptime_start']
        
        return {
            **self.stats,
            'uptime_seconds': int(uptime),
            'uptime_hours': round(uptime / 3600, 2)
        }
    
    def get_all_peers(self) -> List[Dict]:
        """Get all registered peers as dictionaries"""
        return [peer.to_dict() for peer in self.peers.values()]
    
    def save_peer_list(self, filepath: Path):
        """Save peer list to file"""
        data = {
            'network_name': self.config.network_name,
            'chain_id': self.config.chain_id,
            'timestamp': time.time(),
            'peers': self.get_all_peers()
        }
        
        with open(filepath, 'w') as f:
            json.dump(data, f, indent=2)
        
        print(f"💾 Saved {len(self.peers)} peers to {filepath}")
    
    def load_peer_list(self, filepath: Path):
        """Load peer list from file"""
        with open(filepath, 'r') as f:
            data = json.load(f)
        
        for peer_data in data['peers']:
            peer = PeerInfo.from_dict(peer_data)
            self.peers[peer.node_id] = peer
        
        self._update_active_peers_count()
        print(f"📂 Loaded {len(self.peers)} peers from {filepath}")
    
    def get_bootstrap_endpoints(self) -> List[str]:
        """Get list of bootstrap node endpoints"""
        endpoints = []
        
        if self.config.public_address:
            endpoints.append(
                f"{self.config.public_address}:{self.config.listen_port}"
            )
        
        return endpoints


class BootstrapNodeAPI:
    """
    API interface for bootstrap node
    """
    
    def __init__(self, node: BootstrapNode):
        self.node = node
    
    async def handle_announce(
        self,
        node_id: str,
        address: str,
        port: int,
        version: str = "1.0.0"
    ) -> Dict:
        """Handle peer announcement"""
        success = self.node.register_peer(node_id, address, port, version)
        
        return {
            'success': success,
            'message': 'Peer registered' if success else 'Registration failed'
        }
    
    async def handle_discovery(
        self,
        node_id: str,
        max_count: int = 10
    ) -> Dict:
        """Handle peer discovery request"""
        peers = self.node.get_peers(max_count=max_count, exclude={node_id})
        
        return {
            'peers': [peer.to_dict() for peer in peers],
            'count': len(peers)
        }
    
    def get_stats(self) -> Dict:
        """Get node statistics"""
        return self.node.get_stats()
    
    def get_network_info(self) -> Dict:
        """Get network information"""
        return {
            'chain_id': self.node.config.chain_id,
            'network_name': self.node.config.network_name,
            'bootstrap_endpoints': self.node.get_bootstrap_endpoints(),
            'active_peers': self.node.stats['active_peers']
        }
