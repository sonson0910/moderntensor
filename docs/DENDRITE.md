# Dendrite Client Documentation

## Overview

The Dendrite client is a production-ready HTTP client component for ModernTensor validators. It enables querying multiple miners simultaneously with resilience, load balancing, and response aggregation capabilities.

## Features

### Core Features
- **Async HTTP Client**: Built on httpx with full async/await support
- **Connection Pooling**: Efficient connection reuse and management
- **Parallel Queries**: Query multiple miners simultaneously
- **Response Aggregation**: Multiple strategies to combine miner responses

### Resilience Features
- **Retry Logic**: Exponential backoff, fixed delay, or linear backoff
- **Circuit Breaker**: Prevents cascading failures
- **Timeout Management**: Configurable timeouts for connections and requests
- **Fallback Strategies**: Graceful degradation on failures

### Performance Features
- **Query Result Caching**: Cache responses with configurable TTL
- **Request Deduplication**: Avoid duplicate in-flight requests
- **Load Balancing**: Round-robin, random, least-loaded, or weighted strategies
- **Connection Limits**: Per-host and global connection management

### Monitoring Features
- **Metrics Tracking**: Queries, successes, failures, response times
- **Circuit Breaker Stats**: State and failure tracking per host
- **Connection Tracking**: Active connections per host

## Quick Start

### Basic Usage

```python
import asyncio
from sdk.dendrite import Dendrite, DendriteConfig

async def main():
    # Create Dendrite client
    config = DendriteConfig(
        timeout=30.0,
        max_retries=3,
        parallel_queries=True,
    )
    
    dendrite = Dendrite(config=config)
    
    # Query multiple miners
    miners = [
        "http://miner1.example.com:8091/forward",
        "http://miner2.example.com:8091/forward",
        "http://miner3.example.com:8091/forward",
    ]
    
    query_data = {"input": "What is AI?"}
    
    # Aggregate responses using majority voting
    result = await dendrite.query(
        endpoints=miners,
        data=query_data,
        aggregation_strategy="majority",
    )
    
    print(f"Result: {result}")
    
    # Cleanup
    await dendrite.close()

if __name__ == "__main__":
    asyncio.run(main())
```

### Configuration Options

```python
from sdk.dendrite import DendriteConfig, LoadBalancingStrategy, RetryStrategy

config = DendriteConfig(
    # Connection settings
    timeout=30.0,
    connect_timeout=10.0,
    read_timeout=30.0,
    
    # Connection pool
    max_connections=100,
    max_connections_per_host=10,
    keepalive_expiry=5.0,
    
    # Retry settings
    max_retries=3,
    retry_strategy=RetryStrategy.EXPONENTIAL_BACKOFF,
    retry_delay=1.0,
    max_retry_delay=30.0,
    
    # Circuit breaker
    circuit_breaker_enabled=True,
    circuit_breaker_threshold=5,
    circuit_breaker_timeout=60.0,
    
    # Query settings
    parallel_queries=True,
    max_parallel_queries=10,
    query_timeout=30.0,
    
    # Load balancing
    load_balancing_strategy=LoadBalancingStrategy.ROUND_ROBIN,
    
    # Caching
    cache_enabled=True,
    cache_ttl=300.0,
    cache_max_size=1000,
    
    # Deduplication
    deduplication_enabled=True,
    deduplication_window=1.0,
    
    # Aggregation
    aggregation_strategy="majority",
    min_responses=1,
)
```

## API Reference

### Dendrite Class

#### `__init__(config: Optional[DendriteConfig] = None)`
Initialize a Dendrite client with the given configuration.

#### `async query(endpoints, data, headers=None, timeout=None, aggregate=True, aggregation_strategy=None) -> Any`
Query multiple endpoints and aggregate responses.

**Parameters:**
- `endpoints`: List of miner endpoint URLs
- `data`: Request payload dictionary
- `headers`: Optional request headers
- `timeout`: Optional timeout override
- `aggregate`: Whether to aggregate responses (default: True)
- `aggregation_strategy`: Strategy to use for aggregation

**Returns:** Aggregated response or list of responses

**Example:**
```python
result = await dendrite.query(
    endpoints=[
        "http://miner1.example.com:8091/forward",
        "http://miner2.example.com:8091/forward",
    ],
    data={"input": "query data"},
    aggregation_strategy="majority",
)
```

#### `async query_single(endpoint, data, headers=None, timeout=None, retry=True) -> Optional[Dict]`
Query a single endpoint with retry logic.

**Parameters:**
- `endpoint`: Miner endpoint URL
- `data`: Request payload dictionary
- `headers`: Optional request headers
- `timeout`: Optional timeout override
- `retry`: Whether to retry on failure (default: True)

**Returns:** Response dictionary or None on failure

#### `get_metrics() -> DendriteMetrics`
Get current client metrics.

**Returns:** DendriteMetrics object with statistics

#### `async close()`
Close the Dendrite client and cleanup resources.

## Aggregation Strategies

The Dendrite client supports multiple response aggregation strategies:

### 1. Majority Vote
Returns the most common response value.

```python
result = await dendrite.query(
    endpoints=miners,
    data=query_data,
    aggregation_strategy="majority",
)
```

**Use case:** Classification tasks where miners vote on categories

### 2. Average
Returns the arithmetic mean of numerical responses.

```python
result = await dendrite.query(
    endpoints=miners,
    data=query_data,
    aggregation_strategy="average",
)
```

**Use case:** Numerical predictions, scores, or ratings

### 3. Median
Returns the median of numerical responses.

```python
result = await dendrite.query(
    endpoints=miners,
    data=query_data,
    aggregation_strategy="median",
)
```

**Use case:** Numerical predictions with outlier resilience

### 4. Weighted Average
Returns weighted average based on miner reputation/trust.

```python
result = await dendrite.query(
    endpoints=miners,
    data=query_data,
    aggregation_strategy="weighted_average",
    weights=[0.5, 0.3, 0.2],  # Based on miner scores
)
```

**Use case:** When miner quality varies significantly

### 5. First Valid
Returns the first successful response.

```python
result = await dendrite.query(
    endpoints=miners,
    data=query_data,
    aggregation_strategy="first",
)
```

**Use case:** Fast response priority, fallback scenarios

### 6. Consensus
Requires threshold agreement among responses.

```python
result = await dendrite.query(
    endpoints=miners,
    data=query_data,
    aggregation_strategy="consensus",
    threshold=0.66,  # 66% agreement required
)
```

**Use case:** High confidence required for decisions

### 7. All Responses
Returns all responses without aggregation.

```python
results = await dendrite.query(
    endpoints=miners,
    data=query_data,
    aggregation_strategy="all",
)
```

**Use case:** Custom aggregation logic, detailed analysis

## Resilience Features

### Retry Logic

The Dendrite client automatically retries failed requests using configurable strategies:

**Exponential Backoff:**
```python
config = DendriteConfig(
    max_retries=3,
    retry_strategy=RetryStrategy.EXPONENTIAL_BACKOFF,
    retry_delay=1.0,  # Initial delay
    max_retry_delay=30.0,  # Cap delay
)
```
Delays: 1s, 2s, 4s, 8s (capped at 30s)

**Fixed Delay:**
```python
config = DendriteConfig(
    retry_strategy=RetryStrategy.FIXED_DELAY,
    retry_delay=2.0,
)
```
Delays: 2s, 2s, 2s

**Linear Backoff:**
```python
config = DendriteConfig(
    retry_strategy=RetryStrategy.LINEAR_BACKOFF,
    retry_delay=1.0,
)
```
Delays: 1s, 2s, 3s, 4s

### Circuit Breaker

Prevents requests to failing miners:

```python
config = DendriteConfig(
    circuit_breaker_enabled=True,
    circuit_breaker_threshold=5,  # Open after 5 failures
    circuit_breaker_timeout=60.0,  # Try again after 60s
    circuit_breaker_half_open_max_calls=3,  # Test with 3 calls
)
```

**States:**
1. **Closed**: Normal operation, requests allowed
2. **Open**: Too many failures, requests blocked
3. **Half-Open**: Testing if service recovered

### Caching

Cache responses to improve performance:

```python
config = DendriteConfig(
    cache_enabled=True,
    cache_ttl=300.0,  # 5 minutes
    cache_max_size=1000,  # Max 1000 entries
)
```

### Request Deduplication

Avoid duplicate in-flight requests:

```python
config = DendriteConfig(
    deduplication_enabled=True,
    deduplication_window=1.0,  # 1 second window
)
```

## Load Balancing

Distribute queries across miners:

### Round Robin
```python
config = DendriteConfig(
    load_balancing_strategy=LoadBalancingStrategy.ROUND_ROBIN
)
```

### Random
```python
config = DendriteConfig(
    load_balancing_strategy=LoadBalancingStrategy.RANDOM
)
```

### Least Loaded
```python
config = DendriteConfig(
    load_balancing_strategy=LoadBalancingStrategy.LEAST_LOADED
)
```

### Weighted
```python
config = DendriteConfig(
    load_balancing_strategy=LoadBalancingStrategy.WEIGHTED
)
```

## Monitoring

### Metrics

```python
metrics = dendrite.get_metrics()

print(f"Total queries: {metrics.total_queries}")
print(f"Successful: {metrics.successful_queries}")
print(f"Failed: {metrics.failed_queries}")
print(f"Retried: {metrics.retried_queries}")
print(f"Cached: {metrics.cached_responses}")
print(f"Avg response time: {metrics.average_response_time:.3f}s")
print(f"Circuit breaker opens: {metrics.circuit_breaker_opens}")
print(f"Active connections: {metrics.active_connections}")
```

### Circuit Breaker Stats

```python
if dendrite.circuit_breaker:
    stats = dendrite.circuit_breaker.get_stats("miner-host")
    print(f"State: {stats['state']}")
    print(f"Failures: {stats['failure_count']}")
    print(f"Last failure: {stats['last_failure']}")
```

## Error Handling

```python
try:
    result = await dendrite.query(endpoints, data)
    if result is None:
        print("All miners failed or returned invalid responses")
except asyncio.TimeoutError:
    print("Query timed out")
except Exception as e:
    print(f"Query error: {e}")
```

## Best Practices

### 1. Use Connection Pooling
```python
# Reuse the same Dendrite instance
dendrite = Dendrite(config)

# Make multiple queries
for query in queries:
    result = await dendrite.query(miners, query)

# Close when done
await dendrite.close()
```

### 2. Configure Appropriate Timeouts
```python
config = DendriteConfig(
    timeout=30.0,  # Overall timeout
    connect_timeout=5.0,  # Quick connection
    read_timeout=25.0,  # Allow time for computation
)
```

### 3. Enable Circuit Breaker
```python
config = DendriteConfig(
    circuit_breaker_enabled=True,
    circuit_breaker_threshold=5,
)
```

### 4. Use Caching for Repeated Queries
```python
config = DendriteConfig(
    cache_enabled=True,
    cache_ttl=300.0,  # 5 minutes
)
```

### 5. Handle Partial Failures
```python
config = DendriteConfig(
    min_responses=2,  # Succeed with 2/3 miners
)

result = await dendrite.query(
    endpoints=three_miners,
    data=query_data,
)
```

## Advanced Usage

### Custom Aggregation

```python
def custom_aggregator(responses):
    # Custom logic
    valid = [r for r in responses if r.get('confidence', 0) > 0.8]
    if not valid:
        return None
    return max(valid, key=lambda r: r['confidence'])

result = ResponseAggregator.custom(responses, custom_aggregator)
```

### Sequential Queries with Early Exit

```python
config = DendriteConfig(
    parallel_queries=False,  # Sequential
    min_responses=1,  # Exit after first success
)
```

### Query with Authentication

```python
headers = {
    "X-API-Key": "your-api-key",
    "X-UID": "validator-001",
}

result = await dendrite.query(
    endpoints=miners,
    data=query_data,
    headers=headers,
)
```

## Troubleshooting

### All Queries Failing

1. Check miner endpoints are accessible
2. Verify API keys are correct
3. Check circuit breaker state
4. Review timeout settings

```python
# Check circuit breaker
if dendrite.circuit_breaker:
    for host in miners:
        stats = dendrite.circuit_breaker.get_stats(host)
        print(f"{host}: {stats['state']}")
```

### Slow Response Times

1. Enable parallel queries
2. Adjust connection pool size
3. Check network latency
4. Enable caching

```python
config = DendriteConfig(
    parallel_queries=True,
    max_parallel_queries=10,
    cache_enabled=True,
)
```

### Circuit Breaker Always Open

1. Increase threshold
2. Increase timeout
3. Check miner health

```python
config = DendriteConfig(
    circuit_breaker_threshold=10,  # Higher threshold
    circuit_breaker_timeout=120.0,  # Longer recovery time
)
```

## See Also

- [Axon Server Documentation](./AXON.md) (Phase 3)
- [Synapse Protocol Documentation](./SYNAPSE.md) (Phase 5)
- [ModernTensor SDK Roadmap](../SDK_REDESIGN_ROADMAP.md)

## Support

For issues or questions:
- GitHub Issues: https://github.com/sonson0910/moderntensor/issues
- Documentation: See roadmap documents in repository root
