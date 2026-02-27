"""
Luxtensor WebSocket Client

Provides real-time subscriptions for blockchain events:
- New blocks
- Pending transactions
- Account balance changes
- Stake/delegation updates
- Neuron weight commits
"""

import asyncio
import json
import logging
from typing import Optional, Callable, Dict, Any, List
from dataclasses import dataclass
from enum import Enum
import websockets
from websockets.exceptions import ConnectionClosed

from sdk.client.constants import DEFAULT_WS_URL

logger = logging.getLogger(__name__)


class SubscriptionType(Enum):
    """Available subscription types matching LuxTensor WebSocket server (Ethereum standard)."""
    NEW_HEADS = "newHeads"                      # New block headers
    NEW_PENDING_TRANSACTIONS = "newPendingTransactions"  # Pending TXs
    LOGS = "logs"                                # Contract event logs
    SYNCING = "syncing"                          # Sync status


@dataclass
class BlockEvent:
    """New block event"""
    block_number: int
    block_hash: str
    timestamp: int
    tx_count: int


@dataclass
class TransactionEvent:
    """Transaction event"""
    tx_hash: str
    from_address: str
    to_address: Optional[str]
    value: int
    status: str  # pending, confirmed, failed


@dataclass
class AccountChangeEvent:
    """Account balance/state change event"""
    address: str
    old_balance: int
    new_balance: int
    block_number: int


class LuxtensorWebSocket:
    """
    WebSocket client for real-time Luxtensor blockchain events.

    Usage:
        ```python
        async def on_block(event: BlockEvent):
            print(f"New block: {event.block_number}")

        ws = LuxtensorWebSocket(DEFAULT_WS_URL)
        ws.subscribe_blocks(on_block)
        await ws.connect()
        ```
    """

    def __init__(
        self,
        ws_url: str = DEFAULT_WS_URL,
        reconnect_delay: float = 5.0,
        max_reconnect_attempts: int = 10,
        ping_interval: float = 20.0,
        ping_timeout: float = 10.0,
    ):
        """
        Initialize WebSocket client.

        Args:
            ws_url: Luxtensor WebSocket endpoint
            reconnect_delay: Seconds between reconnection attempts
            max_reconnect_attempts: Maximum reconnection attempts (0 = infinite)
            ping_interval: Seconds between heartbeat pings (0 = disabled)
            ping_timeout: Seconds to wait for a pong response before closing
        """
        self.ws_url = ws_url
        self.reconnect_delay = reconnect_delay
        self.max_reconnect_attempts = max_reconnect_attempts
        self.ping_interval = ping_interval
        self.ping_timeout = ping_timeout

        self._ws: Optional[websockets.WebSocketClientProtocol] = None
        self._running = False
        self._subscriptions: Dict[SubscriptionType, List[Callable]] = {}
        self._subscription_ids: Dict[str, SubscriptionType] = {}
        self._request_id = 0
        self._pending_requests: Dict[int, asyncio.Future] = {}

    def subscribe_blocks(self, callback: Callable[[BlockEvent], None]) -> None:
        """Subscribe to new block events."""
        self._add_subscription(SubscriptionType.NEW_HEADS, callback)

    def subscribe_pending_transactions(
        self, callback: Callable[[TransactionEvent], None]
    ) -> None:
        """Subscribe to pending transaction events."""
        self._add_subscription(SubscriptionType.NEW_PENDING_TRANSACTIONS, callback)

    def subscribe_account_changes(
        self,
        address: str,
        callback: Callable[[AccountChangeEvent], None]
    ) -> None:
        """Subscribe to account balance changes for specific address via log events."""
        # Store address filter with callback
        callback._filter_address = address  # type: ignore
        self._add_subscription(SubscriptionType.LOGS, callback)

    def subscribe_stake_updates(
        self,
        callback: Callable[[Dict[str, Any]], None],
        hotkey: Optional[str] = None
    ) -> None:
        """Subscribe to stake/delegation updates via log events."""
        if hotkey:
            callback._filter_hotkey = hotkey  # type: ignore
        self._add_subscription(SubscriptionType.LOGS, callback)

    def subscribe_weight_commits(
        self,
        callback: Callable[[Dict[str, Any]], None],
        subnet_id: Optional[int] = None
    ) -> None:
        """Subscribe to weight commit events via log events."""
        if subnet_id is not None:
            callback._filter_subnet = subnet_id  # type: ignore
        self._add_subscription(SubscriptionType.LOGS, callback)

    def _add_subscription(
        self, sub_type: SubscriptionType, callback: Callable
    ) -> None:
        """Add subscription callback."""
        if sub_type not in self._subscriptions:
            self._subscriptions[sub_type] = []
        self._subscriptions[sub_type].append(callback)

    def _get_request_id(self) -> int:
        """Get next request ID."""
        self._request_id += 1
        return self._request_id

    async def _send_request(
        self, method: str, params: Optional[List[Any]] = None
    ) -> Any:
        """Send JSON-RPC request and wait for response."""
        if not self._ws:
            raise ConnectionError("WebSocket not connected")

        request_id = self._get_request_id()
        request = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or [],
            "id": request_id
        }

        future: asyncio.Future = asyncio.get_event_loop().create_future()
        self._pending_requests[request_id] = future

        await self._ws.send(json.dumps(request))

        try:
            result = await asyncio.wait_for(future, timeout=30.0)
            return result
        finally:
            self._pending_requests.pop(request_id, None)

    async def _subscribe_to_events(self) -> None:
        """Register all subscriptions with the server."""
        for sub_type in self._subscriptions.keys():
            try:
                sub_id = await self._send_request(
                    "eth_subscribe",
                    [sub_type.value]
                )
                if sub_id:
                    self._subscription_ids[sub_id] = sub_type
                    logger.info("Subscribed to %s: %s", sub_type.value, sub_id)
            except Exception as e:
                logger.error("Failed to subscribe to %s: %s", sub_type.value, e)

    async def _handle_message(self, message: str) -> None:
        """Handle incoming WebSocket message."""
        try:
            data = json.loads(message)

            # Handle subscription response
            if "id" in data and data["id"] in self._pending_requests:
                future = self._pending_requests[data["id"]]
                if "error" in data:
                    future.set_exception(Exception(data["error"]["message"]))
                else:
                    future.set_result(data.get("result"))
                return

            # Handle subscription event (Ethereum standard)
            if "method" in data and data["method"] == "eth_subscription":
                params = data.get("params", {})
                sub_id = params.get("subscription")
                result = params.get("result", {})

                if sub_id in self._subscription_ids:
                    sub_type = self._subscription_ids[sub_id]
                    await self._dispatch_event(sub_type, result)

        except json.JSONDecodeError:
            logger.error("Failed to parse message: %.100s", message)

    async def _dispatch_event(
        self, sub_type: SubscriptionType, data: Dict[str, Any]
    ) -> None:
        """Dispatch event to registered callbacks."""
        callbacks = self._subscriptions.get(sub_type, [])

        for callback in callbacks:
            try:
                # Create typed event object
                event = self._create_event(sub_type, data)

                # Apply filters if present
                if hasattr(callback, '_filter_address'):
                    if data.get("address") != callback._filter_address:
                        continue
                if hasattr(callback, '_filter_hotkey'):
                    if data.get("hotkey") != callback._filter_hotkey:
                        continue
                if hasattr(callback, '_filter_subnet'):
                    if data.get("subnet_id") != callback._filter_subnet:
                        continue

                # Call callback
                if asyncio.iscoroutinefunction(callback):
                    await callback(event)
                else:
                    callback(event)

            except Exception as e:
                logger.error("Callback error for %s: %s", sub_type.value, e)

    def _create_event(
        self, sub_type: SubscriptionType, data: Dict[str, Any]
    ) -> Any:
        """Create typed event from raw data."""
        if sub_type == SubscriptionType.NEW_HEADS:
            return BlockEvent(
                block_number=int(data.get("number", "0x0"), 16) if isinstance(data.get("number"), str) else data.get("number", 0),
                block_hash=data.get("hash", ""),
                timestamp=int(data.get("timestamp", "0x0"), 16) if isinstance(data.get("timestamp"), str) else data.get("timestamp", 0),
                tx_count=len(data.get("transactions", [])) if isinstance(data.get("transactions"), list) else data.get("tx_count", 0)
            )
        elif sub_type == SubscriptionType.NEW_PENDING_TRANSACTIONS:
            # newPendingTransactions returns just the tx hash string
            if isinstance(data, str):
                return TransactionEvent(
                    tx_hash=data,
                    from_address="",
                    to_address=None,
                    value=0,
                    status="pending"
                )
            return TransactionEvent(
                tx_hash=data.get("hash", ""),
                from_address=data.get("from", ""),
                to_address=data.get("to"),
                value=data.get("value", 0),
                status="pending"
            )
        elif sub_type == SubscriptionType.LOGS:
            # Logs can represent account changes, stake updates, weight commits, etc.
            if "address" in data and "old_balance" in data:
                return AccountChangeEvent(
                    address=data.get("address", ""),
                    old_balance=data.get("old_balance", 0),
                    new_balance=data.get("new_balance", 0),
                    block_number=int(data.get("blockNumber", "0x0"), 16) if isinstance(data.get("blockNumber"), str) else data.get("block_number", 0)
                )
            return data  # Return raw log for other log types
        else:
            return data  # Return raw dict for syncing and other types

    async def connect(self) -> None:
        """Connect to WebSocket and start listening."""
        self._running = True
        attempt = 0

        while self._running:
            try:
                logger.info("Connecting to %s...", self.ws_url)

                async with websockets.connect(
                    self.ws_url,
                    ping_interval=self.ping_interval if self.ping_interval > 0 else None,
                    ping_timeout=self.ping_timeout if self.ping_timeout > 0 else None,
                ) as ws:
                    self._ws = ws
                    attempt = 0
                    logger.info("WebSocket connected")

                    # Subscribe to all registered events
                    await self._subscribe_to_events()

                    # Listen for messages
                    async for message in ws:
                        await self._handle_message(message)

            except ConnectionClosed as e:
                logger.warning("WebSocket closed: %s", e)
            except Exception as e:
                logger.error("WebSocket error: %s", e)

            self._ws = None

            if not self._running:
                break

            # Reconnect
            attempt += 1
            if self.max_reconnect_attempts > 0 and attempt >= self.max_reconnect_attempts:
                logger.error("Max reconnection attempts reached")
                break

            logger.info("Reconnecting in %ss (attempt %s)...", self.reconnect_delay, attempt)
            await asyncio.sleep(self.reconnect_delay)

    async def disconnect(self) -> None:
        """Disconnect from WebSocket."""
        self._running = False
        if self._ws:
            await self._ws.close()
            self._ws = None
        logger.info("WebSocket disconnected")

    @property
    def is_connected(self) -> bool:
        """Check if WebSocket is connected."""
        return self._ws is not None and self._ws.open


# Convenience function for simple usage
async def subscribe_to_blocks(
    ws_url: str,
    callback: Callable[[BlockEvent], None]
) -> LuxtensorWebSocket:
    """
    Quick subscribe to new blocks.

    Example:
        ```python
        async def on_block(block):
            print(f"Block {block.block_number}")

        ws = await subscribe_to_blocks(DEFAULT_WS_URL, on_block)
        ```
    """
    ws = LuxtensorWebSocket(ws_url)
    ws.subscribe_blocks(callback)
    asyncio.create_task(ws.connect())
    return ws
