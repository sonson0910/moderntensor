"""
Luxtensor Indexer Client

Python client for querying indexed blockchain data from the Luxtensor Indexer.
Provides access to historical transactions, transfers, stake events, and block data.
"""

import httpx
import logging
from typing import Optional, Dict, Any, List
from dataclasses import dataclass

logger = logging.getLogger(__name__)


@dataclass
class IndexedBlock:
    """Block data from indexer"""
    number: int
    hash: str
    parent_hash: Optional[str]
    timestamp: int
    tx_count: int


@dataclass
class IndexedTransaction:
    """Transaction data from indexer"""
    hash: str
    block_number: int
    from_address: str
    to_address: Optional[str]
    value: str
    gas_used: int
    status: int
    tx_type: str


@dataclass
class TokenTransfer:
    """Token transfer event from indexer"""
    id: int
    tx_hash: str
    block_number: int
    from_address: str
    to_address: str
    amount: str
    timestamp: int


@dataclass
class StakeEvent:
    """Stake event from indexer"""
    id: int
    block_number: int
    coldkey: str
    hotkey: str
    amount: str
    action: str  # "stake" or "unstake"
    timestamp: int


@dataclass
class SyncStatus:
    """Indexer sync status"""
    last_indexed_block: int
    is_syncing: bool


class IndexerClient:
    """
    Client for querying the Luxtensor Indexer.

    Usage:
        ```python
        from sdk.indexer_client import IndexerClient

        client = IndexerClient("http://localhost:4000")

        # Check sync status
        status = client.get_sync_status()
        print(f"Last block: {status.last_indexed_block}")

        # Get transaction history
        txs = client.get_transactions("0x123...")

        # Get token transfers
        transfers = client.get_transfers("0x123...")
        ```
    """

    def __init__(
        self,
        indexer_url: str = "http://localhost:4000",
        timeout: int = 30,
    ):
        """
        Initialize indexer client.

        Args:
            indexer_url: Indexer HTTP endpoint
            timeout: Request timeout in seconds
        """
        self.indexer_url = indexer_url.rstrip("/")
        self.timeout = timeout
        self._client = httpx.Client(timeout=timeout)

    def _get(self, path: str, params: Optional[Dict] = None) -> Dict[str, Any]:
        """Make GET request to indexer."""
        url = f"{self.indexer_url}{path}"
        try:
            response = self._client.get(url, params=params)
            response.raise_for_status()
            return response.json()
        except httpx.HTTPError as e:
            logger.error(f"Indexer request failed: {e}")
            raise

    def _post(self, path: str, data: Dict[str, Any]) -> Dict[str, Any]:
        """Make POST request to indexer."""
        url = f"{self.indexer_url}{path}"
        try:
            response = self._client.post(url, json=data)
            response.raise_for_status()
            return response.json()
        except httpx.HTTPError as e:
            logger.error(f"Indexer request failed: {e}")
            raise

    # ============ Health & Status ============

    def health_check(self) -> bool:
        """Check if indexer is healthy."""
        try:
            result = self._get("/health")
            return result.get("status") == "ok"
        except Exception:
            return False

    def get_sync_status(self) -> SyncStatus:
        """Get indexer sync status."""
        result = self._get("/health")
        return SyncStatus(
            last_indexed_block=result.get("last_block", 0),
            is_syncing=result.get("syncing", False),
        )

    # ============ Block Queries ============

    def get_latest_block(self) -> Optional[IndexedBlock]:
        """Get the latest indexed block."""
        try:
            result = self._get("/blocks")
            if "number" not in result:
                return None
            return IndexedBlock(
                number=result.get("number", 0),
                hash=result.get("hash", ""),
                parent_hash=result.get("parent_hash"),
                timestamp=result.get("timestamp", 0),
                tx_count=result.get("tx_count", 0),
            )
        except Exception:
            return None

    def get_block(self, number: int) -> Optional[IndexedBlock]:
        """Get block by number."""
        result = self._post("/query", {
            "type": "block",
            "number": number,
        })
        if not result or "error" in result:
            return None
        return IndexedBlock(
            number=result.get("number", 0),
            hash=result.get("hash", ""),
            parent_hash=result.get("parent_hash"),
            timestamp=result.get("timestamp", 0),
            tx_count=result.get("tx_count", 0),
        )

    def get_blocks(self, from_block: int, to_block: int) -> List[IndexedBlock]:
        """Get blocks in range."""
        result = self._post("/query", {
            "type": "blocks",
            "from": from_block,
            "to": to_block,
        })
        blocks = result.get("blocks", [])
        return [
            IndexedBlock(
                number=b.get("number", 0),
                hash=b.get("hash", ""),
                parent_hash=b.get("parent_hash"),
                timestamp=b.get("timestamp", 0),
                tx_count=b.get("tx_count", 0),
            )
            for b in blocks
        ]

    # ============ Transaction Queries ============

    def get_transactions(
        self,
        address: str,
        limit: int = 50,
        offset: int = 0,
    ) -> List[IndexedTransaction]:
        """
        Get transactions for an address.

        Args:
            address: Account address to query
            limit: Maximum results to return
            offset: Pagination offset

        Returns:
            List of transactions involving this address
        """
        result = self._post("/query", {
            "type": "transactions",
            "address": address,
            "limit": limit,
            "offset": offset,
        })
        txs = result.get("transactions", [])
        return [
            IndexedTransaction(
                hash=tx.get("hash", ""),
                block_number=tx.get("block_number", 0),
                from_address=tx.get("from_address", ""),
                to_address=tx.get("to_address"),
                value=tx.get("value", "0"),
                gas_used=tx.get("gas_used", 0),
                status=tx.get("status", 0),
                tx_type=tx.get("tx_type", ""),
            )
            for tx in txs
        ]

    def get_transaction(self, tx_hash: str) -> Optional[IndexedTransaction]:
        """Get transaction by hash."""
        result = self._post("/query", {
            "type": "transaction",
            "hash": tx_hash,
        })
        if not result or "error" in result:
            return None
        return IndexedTransaction(
            hash=result.get("hash", ""),
            block_number=result.get("block_number", 0),
            from_address=result.get("from_address", ""),
            to_address=result.get("to_address"),
            value=result.get("value", "0"),
            gas_used=result.get("gas_used", 0),
            status=result.get("status", 0),
            tx_type=result.get("tx_type", ""),
        )

    # ============ Token Transfer Queries ============

    def get_transfers(
        self,
        address: str,
        limit: int = 50,
        offset: int = 0,
    ) -> List[TokenTransfer]:
        """
        Get token transfers for an address.

        Args:
            address: Account address to query
            limit: Maximum results to return
            offset: Pagination offset

        Returns:
            List of token transfers
        """
        result = self._post("/query", {
            "type": "transfers",
            "address": address,
            "limit": limit,
            "offset": offset,
        })
        transfers = result.get("transfers", [])
        return [
            TokenTransfer(
                id=t.get("id", 0),
                tx_hash=t.get("tx_hash", ""),
                block_number=t.get("block_number", 0),
                from_address=t.get("from_address", ""),
                to_address=t.get("to_address", ""),
                amount=t.get("amount", "0"),
                timestamp=t.get("timestamp", 0),
            )
            for t in transfers
        ]

    # ============ Stake Queries ============

    def get_stake_history(
        self,
        hotkey: str,
        limit: int = 100,
    ) -> List[StakeEvent]:
        """
        Get stake history for a hotkey.

        Args:
            hotkey: Hotkey address to query
            limit: Maximum results to return

        Returns:
            List of stake/unstake events
        """
        result = self._post("/query", {
            "type": "stake_history",
            "hotkey": hotkey,
            "limit": limit,
        })
        events = result.get("events", [])
        return [
            StakeEvent(
                id=e.get("id", 0),
                block_number=e.get("block_number", 0),
                coldkey=e.get("coldkey", ""),
                hotkey=e.get("hotkey", ""),
                amount=e.get("amount", "0"),
                action=e.get("action", ""),
                timestamp=e.get("timestamp", 0),
            )
            for e in events
        ]

    # ============ Aggregation Queries ============

    def get_total_transactions(self) -> int:
        """Get total number of indexed transactions."""
        result = self._post("/query", {"type": "stats"})
        return result.get("total_transactions", 0)

    def get_total_transfers(self) -> int:
        """Get total number of indexed transfers."""
        result = self._post("/query", {"type": "stats"})
        return result.get("total_transfers", 0)

    def close(self) -> None:
        """Close HTTP client."""
        self._client.close()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()


# Async version
class AsyncIndexerClient:
    """Async version of IndexerClient."""

    def __init__(
        self,
        indexer_url: str = "http://localhost:4000",
        timeout: int = 30,
    ):
        self.indexer_url = indexer_url.rstrip("/")
        self.timeout = timeout
        self._client = httpx.AsyncClient(timeout=timeout)

    async def _get(self, path: str, params: Optional[Dict] = None) -> Dict[str, Any]:
        """Make async GET request."""
        url = f"{self.indexer_url}{path}"
        response = await self._client.get(url, params=params)
        response.raise_for_status()
        return response.json()

    async def _post(self, path: str, data: Dict[str, Any]) -> Dict[str, Any]:
        """Make async POST request."""
        url = f"{self.indexer_url}{path}"
        response = await self._client.post(url, json=data)
        response.raise_for_status()
        return response.json()

    async def health_check(self) -> bool:
        """Check if indexer is healthy."""
        try:
            result = await self._get("/health")
            return result.get("status") == "ok"
        except Exception:
            return False

    async def get_sync_status(self) -> SyncStatus:
        """Get indexer sync status."""
        result = await self._get("/health")
        return SyncStatus(
            last_indexed_block=result.get("last_block", 0),
            is_syncing=result.get("syncing", False),
        )

    async def get_latest_block(self) -> Optional[IndexedBlock]:
        """Get latest indexed block."""
        try:
            result = await self._get("/blocks")
            if "number" not in result:
                return None
            return IndexedBlock(
                number=result.get("number", 0),
                hash=result.get("hash", ""),
                parent_hash=result.get("parent_hash"),
                timestamp=result.get("timestamp", 0),
                tx_count=result.get("tx_count", 0),
            )
        except Exception:
            return None

    async def get_transactions(
        self,
        address: str,
        limit: int = 50,
        offset: int = 0,
    ) -> List[IndexedTransaction]:
        """Get transactions for address."""
        result = await self._post("/query", {
            "type": "transactions",
            "address": address,
            "limit": limit,
            "offset": offset,
        })
        txs = result.get("transactions", [])
        return [
            IndexedTransaction(
                hash=tx.get("hash", ""),
                block_number=tx.get("block_number", 0),
                from_address=tx.get("from_address", ""),
                to_address=tx.get("to_address"),
                value=tx.get("value", "0"),
                gas_used=tx.get("gas_used", 0),
                status=tx.get("status", 0),
                tx_type=tx.get("tx_type", ""),
            )
            for tx in txs
        ]

    async def get_transfers(
        self,
        address: str,
        limit: int = 50,
        offset: int = 0,
    ) -> List[TokenTransfer]:
        """Get token transfers for address."""
        result = await self._post("/query", {
            "type": "transfers",
            "address": address,
            "limit": limit,
            "offset": offset,
        })
        transfers = result.get("transfers", [])
        return [
            TokenTransfer(
                id=t.get("id", 0),
                tx_hash=t.get("tx_hash", ""),
                block_number=t.get("block_number", 0),
                from_address=t.get("from_address", ""),
                to_address=t.get("to_address", ""),
                amount=t.get("amount", "0"),
                timestamp=t.get("timestamp", 0),
            )
            for t in transfers
        ]

    async def close(self) -> None:
        """Close async client."""
        await self._client.aclose()

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()
