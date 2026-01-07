"""
Axon configuration module.

Defines configuration settings for the Axon server.
"""

from typing import Optional, List
from pydantic import BaseModel, Field, validator
import os


class AxonConfig(BaseModel):
    """Configuration for Axon server."""

    # Server settings
    host: str = Field(default="0.0.0.0", description="Host to bind the server to")
    port: int = Field(default=8091, ge=1, le=65535, description="Port to bind the server to")
    external_ip: Optional[str] = Field(None, description="External IP address for registration")
    external_port: Optional[int] = Field(None, description="External port for registration")
    
    # SSL/TLS settings
    ssl_enabled: bool = Field(default=False, description="Enable HTTPS")
    ssl_certfile: Optional[str] = Field(None, description="Path to SSL certificate file")
    ssl_keyfile: Optional[str] = Field(None, description="Path to SSL key file")
    
    # Security settings
    authentication_enabled: bool = Field(default=True, description="Enable authentication")
    rate_limiting_enabled: bool = Field(default=True, description="Enable rate limiting")
    rate_limit_requests: int = Field(default=100, description="Maximum requests per window")
    rate_limit_window: int = Field(default=60, description="Rate limit window in seconds")
    
    # Blacklist/Whitelist settings
    blacklist_enabled: bool = Field(default=True, description="Enable IP blacklisting")
    blacklist_ips: List[str] = Field(default_factory=list, description="List of blacklisted IPs")
    whitelist_enabled: bool = Field(default=False, description="Enable IP whitelisting")
    whitelist_ips: List[str] = Field(default_factory=list, description="List of whitelisted IPs")
    
    # DDoS protection
    ddos_protection_enabled: bool = Field(default=True, description="Enable DDoS protection")
    max_concurrent_requests: int = Field(default=50, description="Maximum concurrent requests")
    request_timeout: int = Field(default=30, description="Request timeout in seconds")
    
    # Monitoring settings
    metrics_enabled: bool = Field(default=True, description="Enable Prometheus metrics")
    health_check_enabled: bool = Field(default=True, description="Enable health check endpoint")
    
    # Logging settings
    log_requests: bool = Field(default=True, description="Log all requests")
    log_level: str = Field(default="INFO", description="Logging level")
    
    # Server metadata
    uid: Optional[str] = Field(None, description="Unique identifier for this Axon")
    api_version: str = Field(default="v1", description="API version")
    
    @validator("ssl_enabled")
    def validate_ssl_config(cls, v, values):
        """Validate SSL configuration."""
        if v:
            certfile = values.get("ssl_certfile")
            keyfile = values.get("ssl_keyfile")
            if not certfile or not keyfile:
                raise ValueError("SSL enabled but certfile or keyfile not provided")
            if not os.path.exists(certfile):
                raise ValueError(f"SSL certificate file not found: {certfile}")
            if not os.path.exists(keyfile):
                raise ValueError(f"SSL key file not found: {keyfile}")
        return v
    
    @validator("whitelist_enabled")
    def validate_whitelist(cls, v, values):
        """Ensure whitelist has IPs if enabled."""
        if v and not values.get("whitelist_ips"):
            raise ValueError("Whitelist enabled but no IPs provided")
        return v
    
    class Config:
        """Pydantic configuration."""
        arbitrary_types_allowed = True
        validate_assignment = True


class AxonMetrics(BaseModel):
    """Metrics for monitoring Axon performance."""
    
    total_requests: int = 0
    successful_requests: int = 0
    failed_requests: int = 0
    blocked_requests: int = 0
    average_response_time: float = 0.0
    active_connections: int = 0
    uptime_seconds: float = 0.0
