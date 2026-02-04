"""
Axon server implementation.

The main Axon server class that brings together all components
to create a production-ready server for miners and validators.
"""

from typing import Optional, Callable, Dict, List
from fastapi import FastAPI
import uvicorn
import asyncio
import time
import logging
from datetime import datetime

from .config import AxonConfig, AxonMetrics
from .security import SecurityManager
from .middleware import (
    AuthenticationMiddleware,
    RateLimitMiddleware,
    BlacklistMiddleware,
    DDoSProtectionMiddleware,
    RequestLoggingMiddleware,
)

logger = logging.getLogger(__name__)


class Axon:
    """
    Axon server for ModernTensor miners and validators.
    
    Provides a production-ready HTTP/HTTPS server with security features,
    rate limiting, monitoring, and request handling capabilities.
    """
    
    def __init__(self, config: Optional[AxonConfig] = None):
        """
        Initialize Axon server.
        
        Args:
            config: Axon configuration (uses defaults if not provided)
        """
        self.config = config or AxonConfig()
        
        # Initialize FastAPI app
        self.app = FastAPI(
            title="ModernTensor Axon Server",
            description="Server component for miners and validators",
            version=self.config.api_version,
        )
        
        # Initialize security manager
        self.security_manager = SecurityManager(
            blacklist_ips=set(self.config.blacklist_ips),
            whitelist_ips=set(self.config.whitelist_ips),
            enable_whitelist=self.config.whitelist_enabled,
        )
        
        # Metrics tracking
        self.metrics = AxonMetrics()
        self.start_time = time.time()
        
        # Request handlers (endpoint -> handler function)
        self.handlers: Dict[str, Callable] = {}
        
        # Setup middleware and routes
        self._setup_middleware()
        self._setup_default_routes()
        
        # Server state
        self._server: Optional[uvicorn.Server] = None
        self._running = False
        
        logger.info(f"Initialized Axon server (UID: {self.config.uid})")
    
    def _setup_middleware(self):
        """Set up all middleware in correct order."""
        # Request logging (first to log everything)
        if self.config.log_requests:
            self.app.add_middleware(
                RequestLoggingMiddleware,
                enabled=True,
            )
        
        # DDoS protection (before other checks)
        if self.config.ddos_protection_enabled:
            self.app.add_middleware(
                DDoSProtectionMiddleware,
                security_manager=self.security_manager,
                max_concurrent=self.config.max_concurrent_requests,
                enabled=True,
            )
        
        # Blacklist filtering (block IPs early)
        if self.config.blacklist_enabled:
            self.app.add_middleware(
                BlacklistMiddleware,
                security_manager=self.security_manager,
                enabled=True,
            )
        
        # Rate limiting (after blacklist)
        if self.config.rate_limiting_enabled:
            self.app.add_middleware(
                RateLimitMiddleware,
                security_manager=self.security_manager,
                max_requests=self.config.rate_limit_requests,
                window_seconds=self.config.rate_limit_window,
                enabled=True,
            )
        
        # Authentication (last middleware before handlers)
        if self.config.authentication_enabled:
            self.app.add_middleware(
                AuthenticationMiddleware,
                security_manager=self.security_manager,
                enabled=True,
            )
        
        logger.info("Middleware configured successfully")
    
    def _setup_default_routes(self):
        """Set up default health check and metrics endpoints."""
        
        @self.app.get("/health")
        async def health_check():
            """Health check endpoint."""
            return {
                "status": "healthy",
                "uptime": time.time() - self.start_time,
                "uid": self.config.uid,
            }
        
        @self.app.get("/metrics")
        async def get_metrics():
            """Metrics endpoint for Prometheus."""
            self.metrics.uptime_seconds = time.time() - self.start_time
            self.metrics.active_connections = sum(
                self.security_manager.active_connections.values()
            )
            return self.metrics.dict()
        
        @self.app.get("/info")
        async def get_info():
            """Server information endpoint."""
            return {
                "uid": self.config.uid,
                "version": self.config.api_version,
                "host": self.config.host,
                "port": self.config.port,
                "external_ip": self.config.external_ip,
                "external_port": self.config.external_port,
                "ssl_enabled": self.config.ssl_enabled,
                "uptime": time.time() - self.start_time,
                "started_at": datetime.fromtimestamp(self.start_time).isoformat(),
            }
    
    def attach(
        self,
        endpoint: str,
        handler: Callable,
        methods: List[str] = ["POST"],
    ) -> "Axon":
        """
        Attach a handler function to an endpoint.
        
        Args:
            endpoint: The endpoint path (e.g., "/forward", "/backward")
            handler: The async function to handle requests
            methods: HTTP methods to accept (default: ["POST"])
            
        Returns:
            Self for method chaining
        """
        # Store handler
        self.handlers[endpoint] = handler
        
        # Create route for each method
        for method in methods:
            if method.upper() == "GET":
                self.app.get(endpoint)(handler)
            elif method.upper() == "POST":
                self.app.post(endpoint)(handler)
            elif method.upper() == "PUT":
                self.app.put(endpoint)(handler)
            elif method.upper() == "DELETE":
                self.app.delete(endpoint)(handler)
        
        logger.info(f"Attached handler to {endpoint} ({', '.join(methods)})")
        return self
    
    def register_api_key(self, uid: str) -> str:
        """
        Register and return an API key for a UID.
        
        Args:
            uid: Unique identifier
            
        Returns:
            Generated API key
        """
        return self.security_manager.register_api_key(uid)
    
    def revoke_api_key(self, uid: str):
        """
        Revoke an API key for a UID.
        
        Args:
            uid: Unique identifier
        """
        self.security_manager.revoke_api_key(uid)
    
    def blacklist_ip(self, ip_address: str):
        """
        Add an IP address to the blacklist.
        
        Args:
            ip_address: IP address to blacklist
        """
        self.security_manager.add_to_blacklist(ip_address)
    
    def whitelist_ip(self, ip_address: str):
        """
        Add an IP address to the whitelist.
        
        Args:
            ip_address: IP address to whitelist
        """
        self.security_manager.add_to_whitelist(ip_address)
    
    async def start(self, blocking: bool = True):
        """
        Start the Axon server.
        
        Args:
            blocking: If True, blocks until server stops. If False, runs in background.
        """
        if self._running:
            logger.warning("Axon server is already running")
            return
        
        self._running = True
        
        # Configure uvicorn
        config = uvicorn.Config(
            app=self.app,
            host=self.config.host,
            port=self.config.port,
            log_level=self.config.log_level.lower(),
            timeout_keep_alive=self.config.request_timeout,
            ssl_keyfile=self.config.ssl_keyfile if self.config.ssl_enabled else None,
            ssl_certfile=self.config.ssl_certfile if self.config.ssl_enabled else None,
        )
        
        self._server = uvicorn.Server(config)
        
        protocol = "https" if self.config.ssl_enabled else "http"
        logger.info(
            f"Starting Axon server on {protocol}://{self.config.host}:{self.config.port}"
        )
        
        if blocking:
            await self._server.serve()
        else:
            # Run in background
            asyncio.create_task(self._server.serve())
    
    async def stop(self):
        """Stop the Axon server."""
        if not self._running:
            logger.warning("Axon server is not running")
            return
        
        logger.info("Stopping Axon server...")
        
        if self._server:
            self._server.should_exit = True
        
        self._running = False
        logger.info("Axon server stopped")
    
    def run(self):
        """Run the Axon server (blocking, synchronous)."""
        asyncio.run(self.start(blocking=True))
    
    @property
    def is_running(self) -> bool:
        """Check if the server is running."""
        return self._running
    
    def get_metrics(self) -> AxonMetrics:
        """Get current server metrics."""
        self.metrics.uptime_seconds = time.time() - self.start_time
        self.metrics.active_connections = sum(
            self.security_manager.active_connections.values()
        )
        return self.metrics
    
    def update_config(self, **kwargs):
        """
        Update server configuration.
        
        Args:
            **kwargs: Configuration parameters to update
        """
        for key, value in kwargs.items():
            if hasattr(self.config, key):
                setattr(self.config, key, value)
                logger.info(f"Updated config: {key} = {value}")
            else:
                logger.warning(f"Unknown config parameter: {key}")


# Convenience function to create and run an Axon server
def create_axon(config: Optional[AxonConfig] = None) -> Axon:
    """
    Create an Axon server instance.
    
    Args:
        config: Axon configuration
        
    Returns:
        Configured Axon instance
    """
    return Axon(config=config)
