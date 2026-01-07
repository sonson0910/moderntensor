"""
Middleware components for Axon server.

Provides authentication, rate limiting, and blacklist/whitelist functionality.
"""

from typing import Callable, Optional
from fastapi import Request, Response, HTTPException, status
from fastapi.responses import JSONResponse
from starlette.middleware.base import BaseHTTPMiddleware
import time
import logging

logger = logging.getLogger(__name__)


class AuthenticationMiddleware(BaseHTTPMiddleware):
    """Middleware for API key authentication."""
    
    def __init__(self, app, security_manager, enabled: bool = True):
        """
        Initialize authentication middleware.
        
        Args:
            app: FastAPI application
            security_manager: SecurityManager instance
            enabled: Enable/disable authentication
        """
        super().__init__(app)
        self.security_manager = security_manager
        self.enabled = enabled
        
        # Paths that don't require authentication
        self.public_paths = {
            "/health",
            "/metrics",
            "/docs",
            "/redoc",
            "/openapi.json",
        }
    
    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        """Process request with authentication check."""
        # Skip authentication for public paths
        if not self.enabled or request.url.path in self.public_paths:
            return await call_next(request)
        
        # Extract API key from header
        api_key = request.headers.get("X-API-Key")
        uid = request.headers.get("X-UID")
        
        if not api_key or not uid:
            logger.warning(f"Missing credentials from {request.client.host}")
            return JSONResponse(
                status_code=status.HTTP_401_UNAUTHORIZED,
                content={"detail": "Missing API key or UID"},
            )
        
        # Verify API key
        if not self.security_manager.verify_api_key(uid, api_key):
            logger.warning(
                f"Invalid API key from {request.client.host} for UID {uid}"
            )
            self.security_manager.record_failed_auth(request.client.host)
            return JSONResponse(
                status_code=status.HTTP_401_UNAUTHORIZED,
                content={"detail": "Invalid API key"},
            )
        
        # Reset failed auth on successful authentication
        self.security_manager.reset_failed_auth(request.client.host)
        
        # Add UID to request state for downstream use
        request.state.uid = uid
        
        return await call_next(request)


class RateLimitMiddleware(BaseHTTPMiddleware):
    """Middleware for rate limiting requests."""
    
    def __init__(
        self, 
        app, 
        security_manager,
        max_requests: int = 100,
        window_seconds: int = 60,
        enabled: bool = True,
    ):
        """
        Initialize rate limit middleware.
        
        Args:
            app: FastAPI application
            security_manager: SecurityManager instance
            max_requests: Maximum requests per window
            window_seconds: Time window in seconds
            enabled: Enable/disable rate limiting
        """
        super().__init__(app)
        self.security_manager = security_manager
        self.max_requests = max_requests
        self.window_seconds = window_seconds
        self.enabled = enabled
    
    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        """Process request with rate limiting."""
        if not self.enabled:
            return await call_next(request)
        
        client_ip = request.client.host
        
        # Check rate limit
        allowed, remaining = await self.security_manager.check_rate_limit(
            client_ip, self.max_requests, self.window_seconds
        )
        
        if not allowed:
            logger.warning(f"Rate limit exceeded for {client_ip}")
            return JSONResponse(
                status_code=status.HTTP_429_TOO_MANY_REQUESTS,
                content={
                    "detail": "Rate limit exceeded",
                    "retry_after": self.window_seconds,
                },
                headers={
                    "X-RateLimit-Limit": str(self.max_requests),
                    "X-RateLimit-Remaining": "0",
                    "X-RateLimit-Reset": str(self.window_seconds),
                    "Retry-After": str(self.window_seconds),
                },
            )
        
        # Process request
        response = await call_next(request)
        
        # Add rate limit headers to response
        response.headers["X-RateLimit-Limit"] = str(self.max_requests)
        response.headers["X-RateLimit-Remaining"] = str(remaining)
        response.headers["X-RateLimit-Window"] = str(self.window_seconds)
        
        return response


class BlacklistMiddleware(BaseHTTPMiddleware):
    """Middleware for IP blacklist/whitelist filtering."""
    
    def __init__(
        self, 
        app, 
        security_manager,
        enabled: bool = True,
    ):
        """
        Initialize blacklist middleware.
        
        Args:
            app: FastAPI application
            security_manager: SecurityManager instance
            enabled: Enable/disable blacklist checking
        """
        super().__init__(app)
        self.security_manager = security_manager
        self.enabled = enabled
    
    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        """Process request with IP filtering."""
        if not self.enabled:
            return await call_next(request)
        
        client_ip = request.client.host
        
        # Check if IP is allowed
        allowed, reason = self.security_manager.is_ip_allowed(client_ip)
        
        if not allowed:
            logger.warning(f"Blocked request from {client_ip}: {reason}")
            return JSONResponse(
                status_code=status.HTTP_403_FORBIDDEN,
                content={"detail": f"Access denied: {reason}"},
            )
        
        return await call_next(request)


class DDoSProtectionMiddleware(BaseHTTPMiddleware):
    """Middleware for DDoS protection through connection limiting."""
    
    def __init__(
        self, 
        app, 
        security_manager,
        max_concurrent: int = 50,
        enabled: bool = True,
    ):
        """
        Initialize DDoS protection middleware.
        
        Args:
            app: FastAPI application
            security_manager: SecurityManager instance
            max_concurrent: Maximum concurrent connections per IP
            enabled: Enable/disable DDoS protection
        """
        super().__init__(app)
        self.security_manager = security_manager
        self.max_concurrent = max_concurrent
        self.enabled = enabled
    
    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        """Process request with connection limiting."""
        if not self.enabled:
            return await call_next(request)
        
        client_ip = request.client.host
        
        # Check connection limit
        if not self.security_manager.check_connection_limit(client_ip, self.max_concurrent):
            logger.warning(f"Too many concurrent connections from {client_ip}")
            return JSONResponse(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                content={"detail": "Too many concurrent requests"},
            )
        
        # Increment connection count
        self.security_manager.increment_active_connections(client_ip)
        
        try:
            # Process request
            response = await call_next(request)
            return response
        finally:
            # Decrement connection count
            self.security_manager.decrement_active_connections(client_ip)


class RequestLoggingMiddleware(BaseHTTPMiddleware):
    """Middleware for logging requests and responses."""
    
    def __init__(self, app, enabled: bool = True):
        """
        Initialize request logging middleware.
        
        Args:
            app: FastAPI application
            enabled: Enable/disable request logging
        """
        super().__init__(app)
        self.enabled = enabled
    
    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        """Process request with logging."""
        if not self.enabled:
            return await call_next(request)
        
        # Log request
        start_time = time.time()
        client_ip = request.client.host
        
        logger.info(
            f"Request: {request.method} {request.url.path} from {client_ip}"
        )
        
        # Process request
        try:
            response = await call_next(request)
            duration = time.time() - start_time
            
            logger.info(
                f"Response: {response.status_code} for {request.method} "
                f"{request.url.path} ({duration:.3f}s)"
            )
            
            return response
        except Exception as e:
            duration = time.time() - start_time
            logger.error(
                f"Error: {str(e)} for {request.method} {request.url.path} "
                f"({duration:.3f}s)"
            )
            raise
