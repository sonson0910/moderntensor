import logging
import json
import asyncio
from typing import Dict, Any, Optional

try:
    import websockets
except ImportError:
    websockets = None

logger = logging.getLogger(__name__)


class HydraClient:
    """
    Client to interact with a Cardano Hydra Head (Layer 2).
    Handles WebSocket connection to the Hydra Node API.
    """

    def __init__(self, hydra_node_url: str = "ws://localhost:4001"):
        self.url = hydra_node_url
        self.ws = None
        self.is_connected = False

        if not websockets:
            logger.warning(
                "websockets library not found. HydraClient will run in MOCK mode."
            )

    async def connect(self):
        """Establishes connection to the Hydra Node."""
        if not websockets:
            logger.info(f"[MOCK] Connecting to Hydra Node at {self.url}...")
            await asyncio.sleep(0.1)
            self.is_connected = True
            return

        try:
            logger.info(f"Connecting to Hydra Node at {self.url}...")
            self.ws = await websockets.connect(self.url)
            self.is_connected = True
            logger.info("Connected to Hydra Node.")

            # Start listener task in background (optional, for receiving events)
            # asyncio.create_task(self._listen())

        except Exception as e:
            logger.error(f"Failed to connect to Hydra Node: {e}")
            self.is_connected = False

    async def submit_tx(self, tx_cbor: str) -> Optional[str]:
        """
        Submits a transaction to the Hydra Head.

        Args:
            tx_cbor: The CBOR hex string of the transaction.

        Returns:
            str: The transaction ID if successful, None otherwise.
        """
        if not self.is_connected:
            await self.connect()

        if not websockets or not self.ws:
            # Mock logic
            logger.info(
                f"[MOCK] Submitting transaction to Hydra Head: {tx_cbor[:20]}..."
            )
            await asyncio.sleep(0.1)
            import hashlib

            tx_id = hashlib.sha256(tx_cbor.encode()).hexdigest()
            return tx_id

        try:
            # Hydra Protocol: { "tag": "NewTx", "transaction": <cbor_hex> }
            message = {"tag": "NewTx", "transaction": tx_cbor}
            await self.ws.send(json.dumps(message))
            logger.info(f"Submitted NewTx to Hydra Node.")

            # Note: Hydra API is async. We don't get an immediate TxID response in the same socket frame usually.
            # We might need to listen for 'TxValid' or 'SnapshotConfirmed'.
            # For now, we calculate the TxID locally (hash of body) as optimistic return.
            # In a full implementation, we would wait for confirmation.

            # Calculate TxID locally for reference
            # (This is a simplification; real TxID requires hashing the Tx Body properly)
            import hashlib

            tx_id = hashlib.sha256(tx_cbor.encode()).hexdigest()
            return tx_id

        except Exception as e:
            logger.error(f"Error submitting tx to Hydra: {e}")
            return None

    async def close(self):
        """Closes the connection."""
        self.is_connected = False
        if self.ws:
            await self.ws.close()
        logger.info("Disconnected from Hydra Node.")
