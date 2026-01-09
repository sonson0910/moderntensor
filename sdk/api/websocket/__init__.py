"""
WebSocket API for ModernTensor

Provides WebSocket endpoints for real-time updates from the blockchain.
"""

import logging
import json
import asyncio
from typing import Set, Dict, Any, Optional
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.websockets import WebSocketState

from sdk.async_luxtensor_client import AsyncLuxtensorClient


logger = logging.getLogger(__name__)


class WebSocketAPI:
    """
    WebSocket API for real-time blockchain updates.
    
    Provides WebSocket endpoints for:
    - Real-time block updates
    - Transaction notifications
    - Network state changes
    - Event subscriptions
    
    Example:
        ```python
        from sdk.api import WebSocketAPI
        from sdk.async_luxtensor_client import AsyncLuxtensorClient
        
        client = AsyncLuxtensorClient("ws://localhost:9944")
        ws_api = WebSocketAPI(client)
        
        # Run server
        import uvicorn
        uvicorn.run(ws_api.app, host="0.0.0.0", port=8001)
        ```
    """
    
    def __init__(
        self,
        client: AsyncLuxtensorClient,
        title: str = "ModernTensor WebSocket API",
        version: str = "0.4.0",
    ):
        """
        Initialize WebSocket API.
        
        Args:
            client: AsyncLuxtensorClient for blockchain queries
            title: API title
            version: API version
        """
        self.client = client
        self.app = FastAPI(
            title=title,
            version=version,
            description="WebSocket API for real-time ModernTensor updates"
        )
        
        # Active connections
        self.active_connections: Set[WebSocket] = set()
        self.subscriptions: Dict[WebSocket, Set[str]] = {}
        
        self._setup_routes()
        
        logger.info(f"Initialized WebSocket API: {title} v{version}")
    
    def _setup_routes(self):
        """Setup WebSocket routes."""
        
        @self.app.get("/")
        async def root():
            """API root endpoint."""
            return {
                "name": "ModernTensor WebSocket API",
                "version": "0.4.0",
                "status": "running",
                "endpoints": {
                    "blocks": "/ws/blocks",
                    "transactions": "/ws/transactions",
                    "events": "/ws/events"
                }
            }
        
        @self.app.websocket("/ws/blocks")
        async def websocket_blocks(websocket: WebSocket):
            """WebSocket endpoint for real-time block updates."""
            await self._handle_block_subscription(websocket)
        
        @self.app.websocket("/ws/transactions")
        async def websocket_transactions(websocket: WebSocket):
            """WebSocket endpoint for real-time transaction updates."""
            await self._handle_transaction_subscription(websocket)
        
        @self.app.websocket("/ws/events")
        async def websocket_events(websocket: WebSocket):
            """WebSocket endpoint for custom event subscriptions."""
            await self._handle_event_subscription(websocket)
    
    async def _handle_block_subscription(self, websocket: WebSocket):
        """Handle block subscription WebSocket connection."""
        await websocket.accept()
        self.active_connections.add(websocket)
        
        try:
            # Send initial message
            await websocket.send_json({
                "type": "connected",
                "message": "Subscribed to block updates"
            })
            
            # Poll for new blocks
            last_block = 0
            while True:
                try:
                    current_block = await self.client.get_block_number()
                    
                    if current_block > last_block:
                        block_info = await self.client.get_block(current_block)
                        
                        if block_info:
                            await websocket.send_json({
                                "type": "new_block",
                                "block": block_info.dict()
                            })
                        
                        last_block = current_block
                    
                    await asyncio.sleep(1)  # Poll every second
                    
                except WebSocketDisconnect:
                    break
                except Exception as e:
                    logger.error(f"Error in block subscription: {e}")
                    await websocket.send_json({
                        "type": "error",
                        "error": str(e)
                    })
                    break
                    
        finally:
            self.active_connections.discard(websocket)
    
    async def _handle_transaction_subscription(self, websocket: WebSocket):
        """Handle transaction subscription WebSocket connection."""
        await websocket.accept()
        self.active_connections.add(websocket)
        
        try:
            await websocket.send_json({
                "type": "connected",
                "message": "Subscribed to transaction updates"
            })
            
            # Keep connection alive and send updates
            while True:
                # Placeholder - actual implementation would monitor mempool
                await asyncio.sleep(1)
                
        except WebSocketDisconnect:
            pass
        finally:
            self.active_connections.discard(websocket)
    
    async def _handle_event_subscription(self, websocket: WebSocket):
        """Handle custom event subscription WebSocket connection."""
        await websocket.accept()
        self.active_connections.add(websocket)
        self.subscriptions[websocket] = set()
        
        try:
            await websocket.send_json({
                "type": "connected",
                "message": "Connected to event stream"
            })
            
            # Handle subscription messages
            while True:
                data = await websocket.receive_json()
                
                if data.get("action") == "subscribe":
                    event_type = data.get("event_type")
                    if event_type:
                        self.subscriptions[websocket].add(event_type)
                        await websocket.send_json({
                            "type": "subscribed",
                            "event_type": event_type
                        })
                
                elif data.get("action") == "unsubscribe":
                    event_type = data.get("event_type")
                    if event_type:
                        self.subscriptions[websocket].discard(event_type)
                        await websocket.send_json({
                            "type": "unsubscribed",
                            "event_type": event_type
                        })
                
        except WebSocketDisconnect:
            pass
        finally:
            self.active_connections.discard(websocket)
            self.subscriptions.pop(websocket, None)
    
    async def broadcast(self, message: Dict[str, Any], event_type: Optional[str] = None):
        """
        Broadcast message to all connected clients.
        
        Args:
            message: Message to broadcast
            event_type: Optional event type for filtering subscriptions
        """
        disconnected = set()
        
        for websocket in self.active_connections:
            try:
                # Check if client is subscribed to this event type
                if event_type and websocket in self.subscriptions:
                    if event_type not in self.subscriptions[websocket]:
                        continue
                
                if websocket.client_state == WebSocketState.CONNECTED:
                    await websocket.send_json(message)
                else:
                    disconnected.add(websocket)
                    
            except Exception as e:
                logger.error(f"Error broadcasting to client: {e}")
                disconnected.add(websocket)
        
        # Clean up disconnected clients
        for websocket in disconnected:
            self.active_connections.discard(websocket)
            self.subscriptions.pop(websocket, None)
    
    def run(self, host: str = "0.0.0.0", port: int = 8001, **kwargs):
        """
        Run the WebSocket API server.
        
        Args:
            host: Host to bind to
            port: Port to bind to
            **kwargs: Additional arguments for uvicorn.run()
        """
        import uvicorn
        uvicorn.run(self.app, host=host, port=port, **kwargs)


__all__ = ["WebSocketAPI"]
