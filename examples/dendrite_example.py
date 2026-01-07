"""
Example usage of the Dendrite client for querying miners.

This example demonstrates how to use the Dendrite client to query
multiple miners and aggregate responses.
"""

import asyncio
from sdk.dendrite import Dendrite, DendriteConfig


async def basic_query_example():
    """Basic query example."""
    print("\n" + "="*60)
    print("Basic Query Example")
    print("="*60 + "\n")
    
    # Create Dendrite client
    config = DendriteConfig(
        timeout=30.0,
        max_retries=3,
        parallel_queries=True,
        cache_enabled=True,
    )
    
    dendrite = Dendrite(config=config)
    
    # Define miner endpoints
    miners = [
        "http://miner1.example.com:8091/forward",
        "http://miner2.example.com:8091/forward",
        "http://miner3.example.com:8091/forward",
    ]
    
    # Query data
    query_data = {
        "input": "What is AI?",
        "model": "gpt-example",
    }
    
    # Headers (including API key from Axon)
    headers = {
        "X-API-Key": "your-api-key-here",
        "X-UID": "validator-001",
    }
    
    try:
        # Query multiple miners and aggregate
        result = await dendrite.query(
            endpoints=miners,
            data=query_data,
            headers=headers,
            aggregation_strategy="majority",
        )
        
        print(f"Aggregated result: {result}")
        
        # Get metrics
        metrics = dendrite.get_metrics()
        print(f"\nMetrics:")
        print(f"  Total queries: {metrics.total_queries}")
        print(f"  Successful: {metrics.successful_queries}")
        print(f"  Failed: {metrics.failed_queries}")
        print(f"  Avg response time: {metrics.average_response_time:.3f}s")
        
    finally:
        await dendrite.close()


async def single_query_example():
    """Single endpoint query example."""
    print("\n" + "="*60)
    print("Single Query Example")
    print("="*60 + "\n")
    
    dendrite = Dendrite()
    
    endpoint = "http://miner1.example.com:8091/forward"
    query_data = {"input": "Hello, world!"}
    
    try:
        response = await dendrite.query_single(
            endpoint=endpoint,
            data=query_data,
            retry=True,
        )
        
        print(f"Response: {response}")
        
    finally:
        await dendrite.close()


async def aggregation_strategies_example():
    """Example of different aggregation strategies."""
    print("\n" + "="*60)
    print("Aggregation Strategies Example")
    print("="*60 + "\n")
    
    config = DendriteConfig(parallel_queries=True)
    dendrite = Dendrite(config=config)
    
    miners = [
        "http://miner1.example.com:8091/forward",
        "http://miner2.example.com:8091/forward",
        "http://miner3.example.com:8091/forward",
    ]
    
    query_data = {"input": "Calculate sentiment score"}
    
    strategies = ["majority", "average", "median", "first"]
    
    try:
        for strategy in strategies:
            result = await dendrite.query(
                endpoints=miners,
                data=query_data,
                aggregation_strategy=strategy,
            )
            print(f"{strategy.capitalize()} strategy result: {result}")
        
    finally:
        await dendrite.close()


async def circuit_breaker_example():
    """Example with circuit breaker."""
    print("\n" + "="*60)
    print("Circuit Breaker Example")
    print("="*60 + "\n")
    
    config = DendriteConfig(
        circuit_breaker_enabled=True,
        circuit_breaker_threshold=3,
        circuit_breaker_timeout=10.0,
        max_retries=2,
    )
    
    dendrite = Dendrite(config=config)
    
    # This endpoint might be failing
    failing_endpoint = "http://failing-miner.example.com:8091/forward"
    query_data = {"input": "test"}
    
    try:
        # First few requests will fail and open the circuit
        for i in range(5):
            print(f"\nAttempt {i + 1}:")
            response = await dendrite.query_single(
                endpoint=failing_endpoint,
                data=query_data,
                retry=True,
            )
            
            if response:
                print(f"  Success: {response}")
            else:
                print(f"  Failed (circuit may be open)")
            
            await asyncio.sleep(1)
        
    finally:
        await dendrite.close()


async def load_balancing_example():
    """Example with load balancing."""
    print("\n" + "="*60)
    print("Load Balancing Example")
    print("="*60 + "\n")
    
    config = DendriteConfig(
        load_balancing_strategy="round_robin",
        parallel_queries=False,  # Sequential for demo
    )
    
    dendrite = Dendrite(config=config)
    
    miners = [
        "http://miner1.example.com:8091/forward",
        "http://miner2.example.com:8091/forward",
        "http://miner3.example.com:8091/forward",
    ]
    
    query_data = {"input": "test query"}
    
    try:
        # Query will balance across miners
        for i in range(6):
            print(f"\nQuery {i + 1}:")
            result = await dendrite.query(
                endpoints=miners,
                data=query_data,
            )
            print(f"  Result: {result}")
        
    finally:
        await dendrite.close()


async def caching_example():
    """Example with response caching."""
    print("\n" + "="*60)
    print("Caching Example")
    print("="*60 + "\n")
    
    config = DendriteConfig(
        cache_enabled=True,
        cache_ttl=60.0,  # 60 second TTL
    )
    
    dendrite = Dendrite(config=config)
    
    miners = ["http://miner1.example.com:8091/forward"]
    query_data = {"input": "cached query"}
    
    try:
        # First query - will hit network
        print("First query (network):")
        start = asyncio.get_event_loop().time()
        result1 = await dendrite.query(miners, query_data)
        duration1 = asyncio.get_event_loop().time() - start
        print(f"  Result: {result1}")
        print(f"  Duration: {duration1:.3f}s")
        
        # Second query - should hit cache
        print("\nSecond query (cached):")
        start = asyncio.get_event_loop().time()
        result2 = await dendrite.query(miners, query_data)
        duration2 = asyncio.get_event_loop().time() - start
        print(f"  Result: {result2}")
        print(f"  Duration: {duration2:.3f}s")
        
        metrics = dendrite.get_metrics()
        print(f"\nCached responses: {metrics.cached_responses}")
        
    finally:
        await dendrite.close()


async def main():
    """Run all examples."""
    print("\n" + "="*60)
    print("Dendrite Client Examples")
    print("="*60)
    
    # Note: These are examples - actual execution requires running miners
    print("\nNote: These examples require running Axon miners.")
    print("See examples/axon_example.py to start miners first.\n")
    
    # Uncomment to run examples:
    # await basic_query_example()
    # await single_query_example()
    # await aggregation_strategies_example()
    # await circuit_breaker_example()
    # await load_balancing_example()
    # await caching_example()
    
    print("\nExamples ready to run. Uncomment function calls in main().")


if __name__ == "__main__":
    asyncio.run(main())
