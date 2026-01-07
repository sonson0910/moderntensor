# Axon Server Documentation

## Overview

The Axon server is a production-ready HTTP/HTTPS server component for ModernTensor miners and validators. It provides a secure, scalable platform for serving AI/ML models and handling network requests.

## Features

### Core Features
- **HTTP/HTTPS Server**: FastAPI-based server with SSL/TLS support
- **Request Routing**: Flexible endpoint attachment and handler management
- **Async Support**: Full async/await support for high performance

### Security Features
- **Authentication**: API key-based authentication with secure token generation
- **Rate Limiting**: Configurable request rate limiting per IP
- **IP Filtering**: Blacklist and whitelist support
- **DDoS Protection**: Connection limiting and request throttling
- **Auto-Blacklist**: Automatic blacklisting after failed authentication attempts

### Monitoring Features
- **Prometheus Metrics**: Built-in metrics endpoint for monitoring
- **Health Checks**: Health check endpoint for load balancers
- **Request Logging**: Detailed request/response logging
- **Performance Tracking**: Response time and throughput metrics

## Quick Start

### Basic Usage

```python
import asyncio
from fastapi import Request
from sdk.axon import Axon, AxonConfig

async def forward_handler(request: Request):
    """Handle inference requests."""
    data = await request.json()
    # Process your AI/ML model here
    return {"result": "processed"}

async def main():
    # Create configuration
    config = AxonConfig(
        host="0.0.0.0",
        port=8091,
        uid="miner-001",
    )
    
    # Create Axon server
    axon = Axon(config=config)
    
    # Register API key
    api_key = axon.register_api_key("validator-001")
    print(f"API Key: {api_key}")
    
    # Attach handler
    axon.attach("/forward", forward_handler, methods=["POST"])
    
    # Start server
    await axon.start(blocking=True)

if __name__ == "__main__":
    asyncio.run(main())
```

### Configuration Options

```python
from sdk.axon import AxonConfig

config = AxonConfig(
    # Server settings
    host="0.0.0.0",
    port=8091,
    external_ip="203.0.113.1",  # Optional: for registration
    external_port=8091,          # Optional: for registration
    
    # SSL/TLS settings
    ssl_enabled=False,
    ssl_certfile=None,
    ssl_keyfile=None,
    
    # Security settings
    authentication_enabled=True,
    rate_limiting_enabled=True,
    rate_limit_requests=100,     # Max requests per window
    rate_limit_window=60,        # Window in seconds
    
    # Blacklist/Whitelist
    blacklist_enabled=True,
    blacklist_ips=[],
    whitelist_enabled=False,
    whitelist_ips=[],
    
    # DDoS protection
    ddos_protection_enabled=True,
    max_concurrent_requests=50,
    request_timeout=30,
    
    # Monitoring
    metrics_enabled=True,
    health_check_enabled=True,
    
    # Logging
    log_requests=True,
    log_level="INFO",
    
    # Metadata
    uid="my-axon-server",
    api_version="v1",
)
```

## API Reference

### Axon Class

#### `__init__(config: Optional[AxonConfig] = None)`
Initialize an Axon server with the given configuration.

#### `attach(endpoint: str, handler: Callable, methods: List[str] = ["POST"]) -> Axon`
Attach a request handler to an endpoint.

**Parameters:**
- `endpoint`: The URL path (e.g., "/forward")
- `handler`: Async function to handle requests
- `methods`: HTTP methods to accept (default: ["POST"])

**Returns:** Self for method chaining

**Example:**
```python
async def my_handler(request: Request):
    return {"status": "ok"}

axon.attach("/custom", my_handler, methods=["POST", "GET"])
```

#### `register_api_key(uid: str) -> str`
Register and return an API key for a UID.

**Parameters:**
- `uid`: Unique identifier

**Returns:** Generated API key string

#### `revoke_api_key(uid: str)`
Revoke an API key for a UID.

#### `blacklist_ip(ip_address: str)`
Add an IP address to the blacklist.

#### `whitelist_ip(ip_address: str)`
Add an IP address to the whitelist.

#### `async start(blocking: bool = True)`
Start the Axon server.

**Parameters:**
- `blocking`: If True, blocks until server stops. If False, runs in background.

#### `async stop()`
Stop the Axon server.

#### `run()`
Run the Axon server (blocking, synchronous wrapper).

#### `is_running -> bool`
Property that returns True if the server is running.

#### `get_metrics() -> AxonMetrics`
Get current server metrics.

### Default Endpoints

The Axon server provides these default endpoints:

#### `GET /health`
Health check endpoint for load balancers.

**Response:**
```json
{
    "status": "healthy",
    "uptime": 3600.5,
    "uid": "miner-001"
}
```

#### `GET /metrics`
Prometheus-compatible metrics endpoint.

**Response:**
```json
{
    "total_requests": 1000,
    "successful_requests": 950,
    "failed_requests": 50,
    "blocked_requests": 10,
    "average_response_time": 0.15,
    "active_connections": 5,
    "uptime_seconds": 3600.5
}
```

#### `GET /info`
Server information endpoint.

**Response:**
```json
{
    "uid": "miner-001",
    "version": "v1",
    "host": "0.0.0.0",
    "port": 8091,
    "external_ip": "203.0.113.1",
    "external_port": 8091,
    "ssl_enabled": false,
    "uptime": 3600.5,
    "started_at": "2026-01-07T12:00:00"
}
```

## Authentication

### Using API Keys

Clients must include authentication headers in their requests:

```python
import httpx

headers = {
    "X-API-Key": "your-api-key-here",
    "X-UID": "validator-001",
}

response = await httpx.post(
    "http://localhost:8091/forward",
    json={"input": "data"},
    headers=headers,
)
```

### Registering API Keys

Server-side:
```python
axon = Axon()
api_key = axon.register_api_key("client-uid")
# Share this key securely with the client
```

### Revoking API Keys

```python
axon.revoke_api_key("client-uid")
```

## Security Best Practices

### 1. Enable HTTPS in Production

```python
config = AxonConfig(
    ssl_enabled=True,
    ssl_certfile="/path/to/cert.pem",
    ssl_keyfile="/path/to/key.pem",
)
```

### 2. Configure Rate Limiting

```python
config = AxonConfig(
    rate_limiting_enabled=True,
    rate_limit_requests=100,  # Adjust based on your needs
    rate_limit_window=60,
)
```

### 3. Use IP Whitelisting for Critical Servers

```python
config = AxonConfig(
    whitelist_enabled=True,
    whitelist_ips=[
        "203.0.113.1",  # Trusted validator
        "198.51.100.1",  # Another trusted node
    ],
)
```

### 4. Enable DDoS Protection

```python
config = AxonConfig(
    ddos_protection_enabled=True,
    max_concurrent_requests=50,
    request_timeout=30,
)
```

## Monitoring

### Prometheus Integration

The `/metrics` endpoint provides metrics in a format compatible with Prometheus:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'moderntensor-axon'
    static_configs:
      - targets: ['localhost:8091']
    metrics_path: '/metrics'
```

### Available Metrics

- `total_requests`: Total number of requests received
- `successful_requests`: Number of successful requests
- `failed_requests`: Number of failed requests
- `blocked_requests`: Number of blocked requests (rate limit, blacklist)
- `average_response_time`: Average response time in seconds
- `active_connections`: Current number of active connections
- `uptime_seconds`: Server uptime in seconds

## Error Handling

### Rate Limit Exceeded

**Status Code:** 429 Too Many Requests

**Response:**
```json
{
    "detail": "Rate limit exceeded",
    "retry_after": 60
}
```

**Headers:**
- `X-RateLimit-Limit`: Maximum requests per window
- `X-RateLimit-Remaining`: Requests remaining
- `X-RateLimit-Reset`: Seconds until reset
- `Retry-After`: Seconds to wait before retrying

### Authentication Failed

**Status Code:** 401 Unauthorized

**Response:**
```json
{
    "detail": "Invalid API key"
}
```

### IP Blocked

**Status Code:** 403 Forbidden

**Response:**
```json
{
    "detail": "Access denied: IP is blacklisted"
}
```

## Advanced Usage

### Custom Middleware

You can add custom middleware to the Axon server:

```python
from starlette.middleware.base import BaseHTTPMiddleware

class CustomMiddleware(BaseHTTPMiddleware):
    async def dispatch(self, request, call_next):
        # Custom logic before request
        response = await call_next(request)
        # Custom logic after request
        return response

axon = Axon()
axon.app.add_middleware(CustomMiddleware)
```

### Multiple Endpoints

```python
axon = Axon()

# Chain multiple attachments
(axon
    .attach("/forward", forward_handler, methods=["POST"])
    .attach("/backward", backward_handler, methods=["POST"])
    .attach("/status", status_handler, methods=["GET"])
)
```

### Background Tasks

```python
from fastapi import BackgroundTasks

async def process_task(data):
    # Long-running task
    pass

async def handler(request: Request, background_tasks: BackgroundTasks):
    data = await request.json()
    background_tasks.add_task(process_task, data)
    return {"status": "processing"}
```

## Troubleshooting

### Server Won't Start

1. Check if port is already in use:
   ```bash
   lsof -i :8091
   ```

2. Verify SSL certificates exist and are readable:
   ```bash
   ls -la /path/to/cert.pem
   ```

### High Memory Usage

- Enable rate limiting to prevent abuse
- Reduce `max_concurrent_requests`
- Monitor and clean up old data regularly

### Authentication Issues

- Verify API keys are correctly registered
- Check that headers are properly formatted
- Review logs for failed authentication attempts

## Performance Tips

1. **Use Async Handlers**: Always use `async def` for handlers
2. **Enable Rate Limiting**: Protect against abuse
3. **Configure Connection Pooling**: Adjust `max_concurrent_requests`
4. **Monitor Metrics**: Watch for bottlenecks
5. **Use Load Balancing**: Distribute traffic across multiple Axon servers

## See Also

- [Dendrite Client Documentation](./DENDRITE.md) (Phase 4)
- [Synapse Protocol Documentation](./SYNAPSE.md) (Phase 5)
- [ModernTensor SDK Roadmap](../SDK_REDESIGN_ROADMAP.md)

## Support

For issues or questions:
- GitHub Issues: https://github.com/sonson0910/moderntensor/issues
- Documentation: See roadmap documents in repository root
