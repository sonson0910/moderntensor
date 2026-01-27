"""
Luxtensor Metrics Client

Provides methods to fetch and display node and indexer metrics.
"""

from typing import Dict, Any, Optional
from dataclasses import dataclass


@dataclass
class NodeMetrics:
    """Node metrics snapshot."""
    block_height: int
    peer_count: int
    tx_count: int
    mempool_size: int
    total_stake: int
    avg_block_time_ms: float
    last_block_time_ms: int
    uptime_secs: int
    tx_throughput: float

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "NodeMetrics":
        """Create from RPC response dict."""
        return cls(
            block_height=data.get("blockHeight", 0),
            peer_count=data.get("peerCount", 0),
            tx_count=data.get("txCount", 0),
            mempool_size=data.get("mempoolSize", 0),
            total_stake=int(data.get("totalStake", "0")),
            avg_block_time_ms=data.get("avgBlockTimeMs", 0.0),
            last_block_time_ms=data.get("lastBlockTimeMs", 0),
            uptime_secs=data.get("uptimeSecs", 0),
            tx_throughput=data.get("txThroughput", 0.0),
        )

    def __str__(self) -> str:
        return (
            f"NodeMetrics(height={self.block_height}, peers={self.peer_count}, "
            f"tx/s={self.tx_throughput:.2f}, uptime={self.uptime_secs}s)"
        )


@dataclass
class IndexerMetrics:
    """Indexer metrics snapshot."""
    last_indexed_block: int
    total_blocks_indexed: int
    total_transactions: int
    total_transfers: int
    total_stake_events: int
    is_syncing: bool
    uptime_secs: int
    blocks_per_sec: float

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "IndexerMetrics":
        """Create from API response dict."""
        return cls(
            last_indexed_block=data.get("lastIndexedBlock", 0),
            total_blocks_indexed=data.get("totalBlocksIndexed", 0),
            total_transactions=data.get("totalTransactions", 0),
            total_transfers=data.get("totalTransfers", 0),
            total_stake_events=data.get("totalStakeEvents", 0),
            is_syncing=data.get("isSyncing", False),
            uptime_secs=data.get("uptimeSecs", 0),
            blocks_per_sec=data.get("blocksPerSec", 0.0),
        )

    def __str__(self) -> str:
        sync_status = "syncing" if self.is_syncing else "synced"
        return (
            f"IndexerMetrics({sync_status}, block={self.last_indexed_block}, "
            f"{self.blocks_per_sec:.2f} blocks/s)"
        )


class MetricsClient:
    """
    Client for fetching metrics from Luxtensor node and indexer.

    Example:
        >>> from sdk.metrics_client import MetricsClient
        >>> client = MetricsClient("http://localhost:8545")
        >>> metrics = client.get_node_metrics()
        >>> print(f"Block height: {metrics.block_height}")
    """

    def __init__(
        self,
        node_url: str = "http://localhost:8545",
        indexer_url: Optional[str] = None
    ):
        """
        Initialize metrics client.

        Args:
            node_url: Node RPC URL
            indexer_url: Optional indexer GraphQL URL
        """
        self.node_url = node_url
        self.indexer_url = indexer_url or node_url.replace(":8545", ":4000")

    def get_node_metrics(self) -> NodeMetrics:
        """
        Fetch metrics from Luxtensor node.

        Returns:
            NodeMetrics dataclass
        """
        import requests

        response = requests.post(
            self.node_url,
            json={
                "jsonrpc": "2.0",
                "method": "system_metrics",
                "params": [],
                "id": 1
            },
            timeout=10
        )
        response.raise_for_status()
        result = response.json()

        if "error" in result:
            raise Exception(f"RPC error: {result['error']}")

        return NodeMetrics.from_dict(result.get("result", {}))

    def get_indexer_metrics(self) -> IndexerMetrics:
        """
        Fetch metrics from indexer.

        Returns:
            IndexerMetrics dataclass
        """
        import requests

        # GraphQL query for indexer metrics
        query = """
        query {
            metrics {
                lastIndexedBlock
                totalBlocksIndexed
                totalTransactions
                totalTransfers
                totalStakeEvents
                isSyncing
                uptimeSecs
                blocksPerSec
            }
        }
        """

        response = requests.post(
            self.indexer_url,
            json={"query": query},
            timeout=10
        )
        response.raise_for_status()
        result = response.json()

        if "errors" in result:
            raise Exception(f"GraphQL error: {result['errors']}")

        data = result.get("data", {}).get("metrics", {})
        return IndexerMetrics.from_dict(data)

    def print_status(self) -> None:
        """Print formatted status of node and indexer."""
        try:
            node = self.get_node_metrics()
            print(f"🔗 Node: {node}")
        except Exception as e:
            print(f"❌ Node error: {e}")

        try:
            indexer = self.get_indexer_metrics()
            print(f"📊 Indexer: {indexer}")
        except Exception as e:
            print(f"❌ Indexer error: {e}")


if __name__ == "__main__":
    # Example usage
    client = MetricsClient()
    client.print_status()
