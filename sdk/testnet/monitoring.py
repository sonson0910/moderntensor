"""
Testnet Monitoring

This module provides monitoring tools for testnet deployment.
"""

import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional
from datetime import datetime
import asyncio


@dataclass
class NodeHealth:
    """Health status of a node"""
    node_id: str
    status: str  # "healthy", "degraded", "down"
    last_block_height: int
    last_block_time: float
    peer_count: int
    sync_status: str
    cpu_usage: float = 0.0
    memory_usage: float = 0.0
    disk_usage: float = 0.0
    timestamp: float = field(default_factory=time.time)
    
    def to_dict(self) -> Dict:
        return {
            'node_id': self.node_id,
            'status': self.status,
            'last_block_height': self.last_block_height,
            'last_block_time': self.last_block_time,
            'peer_count': self.peer_count,
            'sync_status': self.sync_status,
            'cpu_usage': self.cpu_usage,
            'memory_usage': self.memory_usage,
            'disk_usage': self.disk_usage,
            'timestamp': self.timestamp
        }


@dataclass
class NetworkMetrics:
    """Network-wide metrics"""
    total_nodes: int
    healthy_nodes: int
    total_validators: int
    active_validators: int
    current_height: int
    avg_block_time: float
    total_transactions: int
    tps: float  # Transactions per second
    timestamp: float = field(default_factory=time.time)
    
    def to_dict(self) -> Dict:
        return {
            'total_nodes': self.total_nodes,
            'healthy_nodes': self.healthy_nodes,
            'total_validators': self.total_validators,
            'active_validators': self.active_validators,
            'current_height': self.current_height,
            'avg_block_time': self.avg_block_time,
            'total_transactions': self.total_transactions,
            'tps': self.tps,
            'timestamp': self.timestamp
        }


class TestnetMonitor:
    """
    Monitor testnet health and performance
    """
    
    def __init__(self):
        self.nodes: Dict[str, NodeHealth] = {}
        self.metrics_history: List[NetworkMetrics] = []
        self.alerts: List[Dict] = []
        self.running = False
    
    async def start(self):
        """Start monitoring"""
        self.running = True
        print("📊 Testnet monitoring started")
        
        # Start background tasks
        asyncio.create_task(self._collect_metrics())
        asyncio.create_task(self._check_alerts())
    
    async def stop(self):
        """Stop monitoring"""
        self.running = False
        print("🛑 Testnet monitoring stopped")
    
    def update_node_health(self, health: NodeHealth):
        """Update health status for a node"""
        self.nodes[health.node_id] = health
    
    def get_node_health(self, node_id: str) -> Optional[NodeHealth]:
        """Get health status for a specific node"""
        return self.nodes.get(node_id)
    
    def get_all_nodes_health(self) -> List[NodeHealth]:
        """Get health status for all nodes"""
        return list(self.nodes.values())
    
    def calculate_network_metrics(self) -> NetworkMetrics:
        """Calculate network-wide metrics"""
        if not self.nodes:
            return NetworkMetrics(
                total_nodes=0,
                healthy_nodes=0,
                total_validators=0,
                active_validators=0,
                current_height=0,
                avg_block_time=0.0,
                total_transactions=0,
                tps=0.0
            )
        
        healthy_nodes = sum(1 for node in self.nodes.values() if node.status == "healthy")
        current_height = max(node.last_block_height for node in self.nodes.values())
        
        # Calculate average block time
        block_times = [node.last_block_time for node in self.nodes.values() if node.last_block_time > 0]
        avg_block_time = sum(block_times) / len(block_times) if block_times else 0.0
        
        metrics = NetworkMetrics(
            total_nodes=len(self.nodes),
            healthy_nodes=healthy_nodes,
            total_validators=0,  # Would be populated from consensus data
            active_validators=0,
            current_height=current_height,
            avg_block_time=avg_block_time,
            total_transactions=0,  # Would be queried from blockchain
            tps=0.0  # Would be calculated from recent blocks
        )
        
        self.metrics_history.append(metrics)
        
        # Keep only last 1000 metric snapshots
        if len(self.metrics_history) > 1000:
            self.metrics_history = self.metrics_history[-1000:]
        
        return metrics
    
    async def _collect_metrics(self):
        """Background task to collect metrics"""
        while self.running:
            self.calculate_network_metrics()
            await asyncio.sleep(60)  # Collect every minute
    
    async def _check_alerts(self):
        """Background task to check for alert conditions"""
        while self.running:
            current_time = time.time()
            
            # Check for unhealthy nodes
            for node in self.nodes.values():
                if node.status != "healthy":
                    self.alerts.append({
                        'type': 'node_unhealthy',
                        'node_id': node.node_id,
                        'status': node.status,
                        'timestamp': current_time
                    })
            
            # Check for stalled chain
            if self.nodes:
                latest_block_time = max(node.last_block_time for node in self.nodes.values())
                if current_time - latest_block_time > 300:  # 5 minutes
                    self.alerts.append({
                        'type': 'chain_stalled',
                        'message': 'No new blocks in 5 minutes',
                        'timestamp': current_time
                    })
            
            # Keep only last 100 alerts
            if len(self.alerts) > 100:
                self.alerts = self.alerts[-100:]
            
            await asyncio.sleep(300)  # Check every 5 minutes
    
    def get_recent_alerts(self, count: int = 10) -> List[Dict]:
        """Get recent alerts"""
        return self.alerts[-count:]
    
    def get_dashboard_data(self) -> Dict:
        """Get data for monitoring dashboard"""
        metrics = self.calculate_network_metrics()
        
        return {
            'network_metrics': metrics.to_dict(),
            'nodes': [node.to_dict() for node in self.nodes.values()],
            'recent_alerts': self.get_recent_alerts(),
            'timestamp': time.time()
        }


class TestnetExplorer:
    """
    Basic blockchain explorer for testnet
    """
    
    def __init__(self):
        self.blocks: List[Dict] = []
        self.transactions: List[Dict] = []
        self.accounts: Dict[str, Dict] = {}
    
    def add_block(self, block: Dict):
        """Add a block to the explorer"""
        self.blocks.append(block)
        
        # Keep only last 1000 blocks in memory
        if len(self.blocks) > 1000:
            self.blocks = self.blocks[-1000:]
    
    def add_transaction(self, tx: Dict):
        """Add a transaction to the explorer"""
        self.transactions.append(tx)
        
        # Update account info
        if 'from' in tx:
            if tx['from'] not in self.accounts:
                self.accounts[tx['from']] = {
                    'address': tx['from'],
                    'tx_count': 0,
                    'last_seen': 0
                }
            self.accounts[tx['from']]['tx_count'] += 1
            self.accounts[tx['from']]['last_seen'] = time.time()
        
        # Keep only last 10000 transactions in memory
        if len(self.transactions) > 10000:
            self.transactions = self.transactions[-10000:]
    
    def get_latest_blocks(self, count: int = 10) -> List[Dict]:
        """Get latest blocks"""
        return self.blocks[-count:]
    
    def get_block(self, height: int) -> Optional[Dict]:
        """Get block by height"""
        for block in reversed(self.blocks):
            if block.get('height') == height:
                return block
        return None
    
    def get_transaction(self, tx_hash: str) -> Optional[Dict]:
        """Get transaction by hash"""
        for tx in reversed(self.transactions):
            if tx.get('hash') == tx_hash:
                return tx
        return None
    
    def get_account(self, address: str) -> Optional[Dict]:
        """Get account information"""
        return self.accounts.get(address)
    
    def get_statistics(self) -> Dict:
        """Get explorer statistics"""
        return {
            'total_blocks': len(self.blocks),
            'total_transactions': len(self.transactions),
            'total_accounts': len(self.accounts),
            'latest_block': self.blocks[-1] if self.blocks else None
        }
