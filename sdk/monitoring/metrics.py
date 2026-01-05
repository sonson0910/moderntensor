"""
Prometheus metrics for ModernTensor blockchain.

This module provides metrics collection for monitoring blockchain health,
performance, and network status.
"""

import logging
import time
from typing import Optional, Dict, Any

try:
    from prometheus_client import (
        Counter,
        Gauge,
        Histogram,
        Summary,
        Info,
        CollectorRegistry,
        generate_latest,
        CONTENT_TYPE_LATEST,
    )
    PROMETHEUS_AVAILABLE = True
except ImportError:
    PROMETHEUS_AVAILABLE = False
    # Create dummy classes if prometheus_client is not available
    class Counter:
        def __init__(self, *args, **kwargs): pass
        def inc(self, *args, **kwargs): pass
        def labels(self, *args, **kwargs): return self
    
    class Gauge:
        def __init__(self, *args, **kwargs): pass
        def set(self, *args, **kwargs): pass
        def inc(self, *args, **kwargs): pass
        def dec(self, *args, **kwargs): pass
        def labels(self, *args, **kwargs): return self
    
    class Histogram:
        def __init__(self, *args, **kwargs): pass
        def observe(self, *args, **kwargs): pass
        def labels(self, *args, **kwargs): return self
        def time(self): 
            class Timer:
                def __enter__(self): return self
                def __exit__(self, *args): pass
            return Timer()
    
    class Summary:
        def __init__(self, *args, **kwargs): pass
        def observe(self, *args, **kwargs): pass
        def labels(self, *args, **kwargs): return self
    
    class Info:
        def __init__(self, *args, **kwargs): pass
        def info(self, *args, **kwargs): pass
    
    CollectorRegistry = None
    def generate_latest(*args, **kwargs): return b""
    CONTENT_TYPE_LATEST = "text/plain"

logger = logging.getLogger(__name__)


# ===== Blockchain Metrics =====

# Block metrics
block_height = Gauge(
    'moderntensor_block_height',
    'Current blockchain height (latest block number)'
)

block_production_time = Histogram(
    'moderntensor_block_production_seconds',
    'Time taken to produce a block',
    buckets=[1, 2, 5, 10, 15, 30, 60, 120]
)

blocks_produced_total = Counter(
    'moderntensor_blocks_produced_total',
    'Total number of blocks produced'
)

block_size_bytes = Histogram(
    'moderntensor_block_size_bytes',
    'Size of blocks in bytes',
    buckets=[1024, 10240, 102400, 1024000, 10240000]
)

# Transaction metrics
transactions_total = Counter(
    'moderntensor_transactions_total',
    'Total number of transactions processed',
    ['status']  # status: success, failed, pending
)

transactions_per_block = Histogram(
    'moderntensor_transactions_per_block',
    'Number of transactions per block',
    buckets=[0, 1, 5, 10, 50, 100, 500, 1000]
)

transaction_gas_used = Histogram(
    'moderntensor_transaction_gas_used',
    'Gas used by transactions',
    buckets=[21000, 50000, 100000, 500000, 1000000, 5000000]
)

# State metrics
state_accounts_total = Gauge(
    'moderntensor_state_accounts_total',
    'Total number of accounts in state'
)

state_size_bytes = Gauge(
    'moderntensor_state_size_bytes',
    'Total size of blockchain state in bytes'
)

# ===== Network Metrics =====

peers_connected = Gauge(
    'moderntensor_peers_connected',
    'Number of currently connected peers'
)

peer_messages_received = Counter(
    'moderntensor_peer_messages_received_total',
    'Total peer messages received',
    ['message_type']
)

peer_messages_sent = Counter(
    'moderntensor_peer_messages_sent_total',
    'Total peer messages sent',
    ['message_type']
)

network_bytes_received = Counter(
    'moderntensor_network_bytes_received_total',
    'Total network bytes received'
)

network_bytes_sent = Counter(
    'moderntensor_network_bytes_sent_total',
    'Total network bytes sent'
)

sync_progress = Gauge(
    'moderntensor_sync_progress_percent',
    'Blockchain synchronization progress percentage'
)

# ===== Consensus Metrics =====

validators_active = Gauge(
    'moderntensor_validators_active',
    'Number of active validators'
)

validator_stake_total = Gauge(
    'moderntensor_validator_stake_total',
    'Total stake across all validators'
)

epoch_number = Gauge(
    'moderntensor_epoch_number',
    'Current epoch number'
)

validator_rewards_total = Counter(
    'moderntensor_validator_rewards_total',
    'Total validator rewards distributed'
)

validator_penalties_total = Counter(
    'moderntensor_validator_penalties_total',
    'Total validator penalties applied'
)

# ===== AI Task Metrics =====

ai_tasks_submitted = Counter(
    'moderntensor_ai_tasks_submitted_total',
    'Total AI tasks submitted'
)

ai_tasks_completed = Counter(
    'moderntensor_ai_tasks_completed_total',
    'Total AI tasks completed',
    ['status']  # status: success, failed, timeout
)

ai_task_execution_time = Histogram(
    'moderntensor_ai_task_execution_seconds',
    'AI task execution time',
    buckets=[1, 5, 10, 30, 60, 300, 600, 1800]
)

# ===== System Metrics =====

system_info = Info(
    'moderntensor_system',
    'System information'
)

node_uptime_seconds = Gauge(
    'moderntensor_node_uptime_seconds',
    'Node uptime in seconds'
)


class MetricsCollector:
    """
    Central metrics collector for ModernTensor blockchain.
    
    Collects and exposes metrics for Prometheus scraping.
    """
    
    def __init__(self, registry: Optional['CollectorRegistry'] = None):
        """
        Initialize metrics collector.
        
        Args:
            registry: Prometheus registry (optional)
        """
        self.registry = registry
        self.start_time = time.time()
        
        if not PROMETHEUS_AVAILABLE:
            logger.warning(
                "prometheus_client not installed. Metrics disabled. "
                "Install with: pip install prometheus-client"
            )
        
        logger.info("MetricsCollector initialized")
    
    def update_blockchain_metrics(
        self,
        height: int,
        transactions_count: int,
        block_size: int,
        accounts_count: int,
    ):
        """Update blockchain-related metrics"""
        block_height.set(height)
        blocks_produced_total.inc()
        block_size_bytes.observe(block_size)
        transactions_per_block.observe(transactions_count)
        state_accounts_total.set(accounts_count)
    
    def record_transaction(self, status: str, gas_used: int):
        """Record transaction metrics"""
        transactions_total.labels(status=status).inc()
        transaction_gas_used.observe(gas_used)
    
    def update_network_metrics(
        self,
        peers_count: int,
        sync_percent: float,
    ):
        """Update network-related metrics"""
        peers_connected.set(peers_count)
        sync_progress.set(sync_percent)
    
    def record_peer_message(self, message_type: str, direction: str, size: int):
        """Record peer message metrics"""
        if direction == "received":
            peer_messages_received.labels(message_type=message_type).inc()
            network_bytes_received.inc(size)
        elif direction == "sent":
            peer_messages_sent.labels(message_type=message_type).inc()
            network_bytes_sent.inc(size)
    
    def update_consensus_metrics(
        self,
        active_validators: int,
        total_stake: int,
        current_epoch: int,
    ):
        """Update consensus-related metrics"""
        validators_active.set(active_validators)
        validator_stake_total.set(total_stake)
        epoch_number.set(current_epoch)
    
    def record_validator_reward(self, amount: int):
        """Record validator reward"""
        validator_rewards_total.inc(amount)
    
    def record_validator_penalty(self, amount: int):
        """Record validator penalty"""
        validator_penalties_total.inc(amount)
    
    def record_ai_task_submitted(self):
        """Record AI task submission"""
        ai_tasks_submitted.inc()
    
    def record_ai_task_completed(self, status: str, execution_time: float):
        """Record AI task completion"""
        ai_tasks_completed.labels(status=status).inc()
        ai_task_execution_time.observe(execution_time)
    
    def update_system_info(self, info: Dict[str, str]):
        """Update system information"""
        system_info.info(info)
    
    def update_uptime(self):
        """Update node uptime"""
        uptime = time.time() - self.start_time
        node_uptime_seconds.set(uptime)
    
    def get_metrics(self) -> bytes:
        """
        Get metrics in Prometheus format.
        
        Returns:
            bytes: Metrics data
        """
        if not PROMETHEUS_AVAILABLE:
            return b"# Prometheus client not available\n"
        
        return generate_latest(self.registry)
    
    def get_content_type(self) -> str:
        """Get content type for metrics endpoint"""
        return CONTENT_TYPE_LATEST


# Singleton instances for easy access
# Note: These are convenience singletons that share the same registry.
# In production, you would typically use a single collector instance
# or implement scoped collectors with distinct responsibilities.
blockchain_metrics = MetricsCollector()
network_metrics = MetricsCollector()
consensus_metrics = MetricsCollector()
