"""
REST API for ModernTensor

Provides RESTful HTTP endpoints for querying and interacting with
the ModernTensor blockchain and network.
"""

import logging
from typing import Optional, Dict, Any, List
from fastapi import FastAPI, HTTPException, Query
from fastapi.responses import JSONResponse
from pydantic import BaseModel

from sdk.luxtensor_client import LuxtensorClient
from sdk.chain_data import (
    NeuronInfo,
    SubnetInfo,
    BlockInfo,
    TransactionInfo,
)


logger = logging.getLogger(__name__)


class RestAPI:
    """
    REST API server for ModernTensor network.
    
    Provides HTTP endpoints for:
    - Blockchain queries (blocks, transactions, state)
    - Network queries (neurons, subnets, validators)
    - Stake queries
    - Transaction submission
    
    Example:
        ```python
        from sdk.api import RestAPI
        from sdk.luxtensor_client import LuxtensorClient
        
        client = LuxtensorClient("http://localhost:9933")
        api = RestAPI(client)
        
        # Run server
        import uvicorn
        uvicorn.run(api.app, host="0.0.0.0", port=8000)
        ```
    """
    
    def __init__(
        self,
        client: LuxtensorClient,
        title: str = "ModernTensor API",
        version: str = "0.4.0",
    ):
        """
        Initialize REST API.
        
        Args:
            client: LuxtensorClient for blockchain queries
            title: API title
            version: API version
        """
        self.client = client
        self.app = FastAPI(
            title=title,
            version=version,
            description="REST API for ModernTensor blockchain and AI network"
        )
        
        self._setup_routes()
        
        logger.info(f"Initialized REST API: {title} v{version}")
    
    def _setup_routes(self):
        """Setup API routes."""
        
        @self.app.get("/")
        async def root():
            """API root endpoint."""
            return {
                "name": "ModernTensor API",
                "version": "0.4.0",
                "status": "running",
                "endpoints": {
                    "blockchain": "/blockchain/...",
                    "network": "/network/...",
                    "stake": "/stake/...",
                    "docs": "/docs"
                }
            }
        
        @self.app.get("/health")
        async def health():
            """Health check endpoint."""
            try:
                block_number = self.client.get_block_number()
                return {
                    "status": "healthy",
                    "connected": True,
                    "block_number": block_number
                }
            except Exception as e:
                return JSONResponse(
                    status_code=503,
                    content={
                        "status": "unhealthy",
                        "connected": False,
                        "error": str(e)
                    }
                )
        
        # Blockchain endpoints
        
        @self.app.get("/blockchain/block/{block_number}")
        async def get_block(block_number: int):
            """Get block by number."""
            try:
                block = self.client.get_block(block_number)
                if block:
                    return block.dict()
                raise HTTPException(status_code=404, detail="Block not found")
            except Exception as e:
                raise HTTPException(status_code=500, detail=str(e))
        
        @self.app.get("/blockchain/block/latest")
        async def get_latest_block():
            """Get latest block."""
            try:
                block = self.client.get_latest_block()
                if block:
                    return block.dict()
                raise HTTPException(status_code=404, detail="Block not found")
            except Exception as e:
                raise HTTPException(status_code=500, detail=str(e))
        
        @self.app.get("/blockchain/transaction/{tx_hash}")
        async def get_transaction(tx_hash: str):
            """Get transaction by hash."""
            try:
                tx = self.client.get_transaction(tx_hash)
                if tx:
                    return tx.dict()
                raise HTTPException(status_code=404, detail="Transaction not found")
            except Exception as e:
                raise HTTPException(status_code=500, detail=str(e))
        
        # Network endpoints
        
        @self.app.get("/network/subnets")
        async def get_subnets():
            """Get all subnets."""
            try:
                subnets = self.client.get_subnets()
                return [s.dict() for s in subnets]
            except Exception as e:
                raise HTTPException(status_code=500, detail=str(e))
        
        @self.app.get("/network/subnet/{subnet_uid}")
        async def get_subnet(subnet_uid: int):
            """Get subnet by UID."""
            try:
                subnet = self.client.get_subnet_info(subnet_uid)
                if subnet:
                    return subnet.dict()
                raise HTTPException(status_code=404, detail="Subnet not found")
            except Exception as e:
                raise HTTPException(status_code=500, detail=str(e))
        
        @self.app.get("/network/subnet/{subnet_uid}/neurons")
        async def get_subnet_neurons(subnet_uid: int):
            """Get all neurons in a subnet."""
            try:
                neurons = self.client.get_neurons(subnet_uid)
                return [n.dict() for n in neurons]
            except Exception as e:
                raise HTTPException(status_code=500, detail=str(e))
        
        @self.app.get("/network/subnet/{subnet_uid}/neuron/{uid}")
        async def get_neuron(subnet_uid: int, uid: int):
            """Get specific neuron."""
            try:
                neuron = self.client.get_neuron(uid, subnet_uid)
                if neuron:
                    return neuron.dict()
                raise HTTPException(status_code=404, detail="Neuron not found")
            except Exception as e:
                raise HTTPException(status_code=500, detail=str(e))
        
        # Stake endpoints
        
        @self.app.get("/stake/{address}")
        async def get_stake(address: str):
            """Get total stake for an address."""
            try:
                stake = self.client.get_total_stake(address)
                return {
                    "address": address,
                    "total_stake": stake
                }
            except Exception as e:
                raise HTTPException(status_code=500, detail=str(e))
        
        @self.app.get("/balance/{address}")
        async def get_balance(address: str):
            """Get balance for an address."""
            try:
                balance = self.client.get_balance(address)
                return {
                    "address": address,
                    "balance": balance
                }
            except Exception as e:
                raise HTTPException(status_code=500, detail=str(e))
    
    def run(self, host: str = "0.0.0.0", port: int = 8000, **kwargs):
        """
        Run the REST API server.
        
        Args:
            host: Host to bind to
            port: Port to bind to
            **kwargs: Additional arguments for uvicorn.run()
        """
        import uvicorn
        uvicorn.run(self.app, host=host, port=port, **kwargs)


__all__ = ["RestAPI"]
