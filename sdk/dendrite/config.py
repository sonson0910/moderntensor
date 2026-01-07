"""
Dendrite configuration module.

Defines configuration settings for the Dendrite client.
"""

from typing import Optional, List
from pydantic import BaseModel, Field, validator
from enum import Enum


class LoadBalancingStrategy(str, Enum):
    """Load balancing strategies for query distribution."""
    ROUND_ROBIN = "round_robin"
    RANDOM = "random"
    LEAST_LOADED = "least_loaded"
    WEIGHTED = "weighted"


class RetryStrategy(str, Enum):
    """Retry strategies for failed requests."""
    EXPONENTIAL_BACKOFF = "exponential_backoff"
    FIXED_DELAY = "fixed_delay"
    LINEAR_BACKOFF = "linear_backoff"


class DendriteConfig(BaseModel):
    """Configuration for Dendrite client."""

    # Connection settings
    timeout: float = Field(default=30.0, ge=0.1, description="Request timeout in seconds")
    connect_timeout: float = Field(default=10.0, ge=0.1, description="Connection timeout in seconds")
    read_timeout: float = Field(default=30.0, ge=0.1, description="Read timeout in seconds")
    
    # Connection pool settings
    max_connections: int = Field(default=100, ge=1, description="Maximum total connections")
    max_connections_per_host: int = Field(default=10, ge=1, description="Maximum connections per host")
    keepalive_expiry: float = Field(default=5.0, ge=0, description="Keep-alive connection expiry in seconds")
    
    # Retry settings
    max_retries: int = Field(default=3, ge=0, description="Maximum number of retries")
    retry_strategy: RetryStrategy = Field(
        default=RetryStrategy.EXPONENTIAL_BACKOFF,
        description="Retry strategy to use"
    )
    retry_delay: float = Field(default=1.0, ge=0, description="Initial retry delay in seconds")
    max_retry_delay: float = Field(default=30.0, ge=0, description="Maximum retry delay in seconds")
    
    # Circuit breaker settings
    circuit_breaker_enabled: bool = Field(default=True, description="Enable circuit breaker")
    circuit_breaker_threshold: int = Field(default=5, ge=1, description="Failed requests before opening")
    circuit_breaker_timeout: float = Field(default=60.0, ge=0, description="Circuit open duration in seconds")
    circuit_breaker_half_open_max_calls: int = Field(default=3, ge=1, description="Max calls in half-open state")
    
    # Query settings
    parallel_queries: bool = Field(default=True, description="Enable parallel query execution")
    max_parallel_queries: int = Field(default=10, ge=1, description="Maximum parallel queries")
    query_timeout: float = Field(default=30.0, ge=0.1, description="Individual query timeout")
    
    # Load balancing
    load_balancing_strategy: LoadBalancingStrategy = Field(
        default=LoadBalancingStrategy.ROUND_ROBIN,
        description="Load balancing strategy"
    )
    
    # Caching settings
    cache_enabled: bool = Field(default=True, description="Enable response caching")
    cache_ttl: float = Field(default=300.0, ge=0, description="Cache TTL in seconds")
    cache_max_size: int = Field(default=1000, ge=1, description="Maximum cache entries")
    
    # Request deduplication
    deduplication_enabled: bool = Field(default=True, description="Enable request deduplication")
    deduplication_window: float = Field(default=1.0, ge=0, description="Deduplication window in seconds")
    
    # Response aggregation
    aggregation_strategy: str = Field(default="majority", description="Response aggregation strategy")
    min_responses: int = Field(default=1, ge=1, description="Minimum successful responses required")
    
    # Fallback settings
    fallback_enabled: bool = Field(default=True, description="Enable fallback on failure")
    fallback_nodes: List[str] = Field(default_factory=list, description="Fallback node endpoints")
    
    # Monitoring
    metrics_enabled: bool = Field(default=True, description="Enable metrics collection")
    
    # Headers
    default_headers: dict = Field(
        default_factory=lambda: {
            "User-Agent": "ModernTensor-Dendrite/1.0",
            "Accept": "application/json",
        },
        description="Default request headers"
    )
    
    @validator("max_retry_delay")
    def validate_max_retry_delay(cls, v, values):
        """Ensure max_retry_delay is greater than retry_delay."""
        if "retry_delay" in values and v < values["retry_delay"]:
            raise ValueError("max_retry_delay must be >= retry_delay")
        return v
    
    @validator("max_connections_per_host")
    def validate_connections_per_host(cls, v, values):
        """Ensure max_connections_per_host doesn't exceed max_connections."""
        if "max_connections" in values and v > values["max_connections"]:
            raise ValueError("max_connections_per_host must be <= max_connections")
        return v
    
    class Config:
        """Pydantic configuration."""
        arbitrary_types_allowed = True
        validate_assignment = True


class DendriteMetrics(BaseModel):
    """Metrics for monitoring Dendrite performance."""
    
    total_queries: int = 0
    successful_queries: int = 0
    failed_queries: int = 0
    retried_queries: int = 0
    cached_responses: int = 0
    average_response_time: float = 0.0
    circuit_breaker_opens: int = 0
    active_connections: int = 0
