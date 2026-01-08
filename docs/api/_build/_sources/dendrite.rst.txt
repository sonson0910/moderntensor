Dendrite Client API
===================

The Dendrite is the client component in ModernTensor's communication layer. It is used by validators to query multiple miners efficiently with features like connection pooling, load balancing, retry logic, and response aggregation.

Overview
--------

The Dendrite provides a production-ready async HTTP client with:

- **Async HTTP Client**: Built on httpx for high performance
- **Connection Pooling**: Efficient connection reuse with configurable limits
- **Load Balancing**: Multiple strategies (round-robin, random, least-loaded, weighted)
- **Retry Logic**: Exponential backoff and fixed delay strategies
- **Circuit Breaker**: Automatic failure detection and recovery
- **Response Aggregation**: Combine responses from multiple miners
- **Caching**: Response caching with TTL
- **Request Deduplication**: Avoid duplicate requests
- **Monitoring**: Comprehensive metrics and logging

Architecture
------------

The Dendrite consists of several components:

1. **Core Client (dendrite.py)**: Main Dendrite class for querying miners
2. **Configuration (config.py)**: DendriteConfig for client settings and metrics
3. **Connection Pool (pool.py)**: ConnectionPool and CircuitBreaker for connection management
4. **Aggregator (aggregator.py)**: ResponseAggregator for combining responses

Configuration
-------------

Basic Configuration
~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from sdk.dendrite import Dendrite, DendriteConfig
   
   # Create configuration
   config = DendriteConfig(
       # Connection settings
       timeout=30.0,
       connect_timeout=10.0,
       max_connections=100,
       max_connections_per_host=10,
       
       # Retry settings
       max_retries=3,
       retry_strategy="exponential_backoff",
       retry_delay=1.0,
       max_retry_delay=30.0,
       
       # Circuit breaker
       circuit_breaker_enabled=True,
       circuit_breaker_threshold=5,
       circuit_breaker_timeout=60.0,
       
       # Query settings
       parallel_queries=True,
       max_parallel_queries=10,
       
       # Caching
       cache_enabled=True,
       cache_ttl=300.0,
       
       # Aggregation
       aggregation_strategy="majority",
       min_responses=1
   )
   
   # Create Dendrite with configuration
   dendrite = Dendrite(config=config)

Load Balancing Strategies
~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from sdk.dendrite import DendriteConfig, LoadBalancingStrategy
   
   # Round-robin (default)
   config = DendriteConfig(
       load_balancing_strategy=LoadBalancingStrategy.ROUND_ROBIN
   )
   
   # Random selection
   config = DendriteConfig(
       load_balancing_strategy=LoadBalancingStrategy.RANDOM
   )
   
   # Least loaded (based on active connections)
   config = DendriteConfig(
       load_balancing_strategy=LoadBalancingStrategy.LEAST_LOADED
   )
   
   # Weighted (based on response times or custom weights)
   config = DendriteConfig(
       load_balancing_strategy=LoadBalancingStrategy.WEIGHTED
   )

Basic Usage
-----------

Querying Single Miner
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from sdk.dendrite import Dendrite
   import asyncio
   
   async def main():
       dendrite = Dendrite()
       
       # Query single miner
       endpoint = "http://miner1.example.com:8091/forward"
       data = {
           "input": "Hello, world!",
           "validator_uid": "val_001"
       }
       
       response = await dendrite.query_single(
           endpoint=endpoint,
           data=data,
           timeout=30.0,
           retry=True
       )
       
       if response:
           print(f"Result: {response['result']}")
       else:
           print("Query failed")
   
   asyncio.run(main())

Querying Multiple Miners
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   async def main():
       dendrite = Dendrite()
       
       # List of miner endpoints
       endpoints = [
           "http://miner1.example.com:8091/forward",
           "http://miner2.example.com:8091/forward",
           "http://miner3.example.com:8091/forward",
       ]
       
       # Query all miners
       data = {"input": "Process this data"}
       
       result = await dendrite.query(
           endpoints=endpoints,
           data=data,
           aggregate=True,  # Aggregate responses
           aggregation_strategy="majority"
       )
       
       print(f"Aggregated result: {result}")
   
   asyncio.run(main())

Parallel vs Sequential Queries
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   # Parallel queries (default, faster)
   config = DendriteConfig(parallel_queries=True, max_parallel_queries=10)
   dendrite = Dendrite(config=config)
   
   # All queries execute simultaneously
   responses = await dendrite.query(endpoints, data)
   
   # Sequential queries (slower, but more controlled)
   config = DendriteConfig(parallel_queries=False)
   dendrite = Dendrite(config=config)
   
   # Queries execute one after another
   responses = await dendrite.query(endpoints, data)

Advanced Features
-----------------

Response Aggregation
~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   # Majority voting
   result = await dendrite.query(
       endpoints=endpoints,
       data=data,
       aggregate=True,
       aggregation_strategy="majority"
   )
   
   # Average (for numerical responses)
   result = await dendrite.query(
       endpoints=endpoints,
       data=data,
       aggregate=True,
       aggregation_strategy="average"
   )
   
   # Median
   result = await dendrite.query(
       endpoints=endpoints,
       data=data,
       aggregate=True,
       aggregation_strategy="median"
   )
   
   # Weighted average (based on stake or trust)
   result = await dendrite.query(
       endpoints=endpoints,
       data=data,
       aggregate=True,
       aggregation_strategy="weighted_average"
   )
   
   # Get all responses without aggregation
   responses = await dendrite.query(
       endpoints=endpoints,
       data=data,
       aggregate=False  # Returns list of all responses
   )

Circuit Breaker
~~~~~~~~~~~~~~~

The circuit breaker automatically detects failing endpoints and temporarily stops sending requests to them:

.. code-block:: python

   config = DendriteConfig(
       circuit_breaker_enabled=True,
       circuit_breaker_threshold=5,      # Open after 5 failures
       circuit_breaker_timeout=60.0,     # Stay open for 60 seconds
       circuit_breaker_half_open_max_calls=3  # Test with 3 calls
   )
   
   dendrite = Dendrite(config=config)
   
   # Circuit breaker states:
   # 1. CLOSED: Normal operation, requests go through
   # 2. OPEN: Too many failures, requests blocked
   # 3. HALF_OPEN: Testing if endpoint recovered

Retry Logic
~~~~~~~~~~~

.. code-block:: python

   from sdk.dendrite import RetryStrategy
   
   # Exponential backoff (default)
   config = DendriteConfig(
       max_retries=3,
       retry_strategy=RetryStrategy.EXPONENTIAL_BACKOFF,
       retry_delay=1.0,        # Start with 1 second
       max_retry_delay=30.0    # Cap at 30 seconds
   )
   # Delays: 1s, 2s, 4s, 8s, ...
   
   # Fixed delay
   config = DendriteConfig(
       max_retries=3,
       retry_strategy=RetryStrategy.FIXED_DELAY,
       retry_delay=2.0  # Always wait 2 seconds
   )
   
   # Linear backoff
   config = DendriteConfig(
       max_retries=3,
       retry_strategy=RetryStrategy.LINEAR_BACKOFF,
       retry_delay=1.0  # 1s, 2s, 3s, 4s, ...
   )

Request Caching
~~~~~~~~~~~~~~~

.. code-block:: python

   config = DendriteConfig(
       cache_enabled=True,
       cache_ttl=300.0,      # Cache for 5 minutes
       cache_max_size=1000   # Maximum 1000 cached entries
   )
   
   dendrite = Dendrite(config=config)
   
   # First query: hits miners
   result1 = await dendrite.query(endpoints, data)
   
   # Second query with same data: uses cache
   result2 = await dendrite.query(endpoints, data)
   
   # Clear cache if needed
   dendrite.cache.clear()

Request Deduplication
~~~~~~~~~~~~~~~~~~~~~

Prevents duplicate requests from being sent simultaneously:

.. code-block:: python

   config = DendriteConfig(
       deduplication_enabled=True,
       deduplication_window=1.0  # 1 second window
   )
   
   # If the same request is made multiple times within 1 second,
   # only one actual request is sent, others wait for the result

Custom Headers and Authentication
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   # Set default headers
   config = DendriteConfig(
       default_headers={
           "User-Agent": "MyValidator/1.0",
           "Accept": "application/json",
       }
   )
   
   dendrite = Dendrite(config=config)
   
   # Add authentication per request
   headers = {
       "Authorization": "Bearer api_key_here",
       "X-Validator-UID": "validator_123"
   }
   
   response = await dendrite.query_single(
       endpoint=endpoint,
       data=data,
       headers=headers
   )

Monitoring and Metrics
----------------------

Accessing Metrics
~~~~~~~~~~~~~~~~~

.. code-block:: python

   # Get current metrics
   metrics = dendrite.metrics
   
   print(f"Total queries: {metrics.total_queries}")
   print(f"Successful: {metrics.successful_queries}")
   print(f"Failed: {metrics.failed_queries}")
   print(f"Cached responses: {metrics.cached_responses}")
   print(f"Average response time: {metrics.average_response_time:.3f}s")
   print(f"Circuit breaker opens: {metrics.circuit_breaker_opens}")
   
   # Export metrics as dictionary
   metrics_dict = metrics.dict()

Complete Example: Production Validator
---------------------------------------

.. code-block:: python

   from sdk.dendrite import Dendrite, DendriteConfig, LoadBalancingStrategy
   from sdk.metagraph import Metagraph
   import asyncio
   import logging
   
   logging.basicConfig(level=logging.INFO)
   logger = logging.getLogger(__name__)
   
   class Validator:
       def __init__(self, uid: str, subnet_id: int):
           self.uid = uid
           self.subnet_id = subnet_id
           
           # Configure Dendrite
           config = DendriteConfig(
               # Connection settings
               timeout=30.0,
               max_connections=100,
               max_connections_per_host=10,
               
               # Retry with exponential backoff
               max_retries=3,
               retry_strategy="exponential_backoff",
               retry_delay=1.0,
               max_retry_delay=30.0,
               
               # Circuit breaker for failing miners
               circuit_breaker_enabled=True,
               circuit_breaker_threshold=5,
               circuit_breaker_timeout=60.0,
               
               # Parallel queries for speed
               parallel_queries=True,
               max_parallel_queries=20,
               
               # Caching for efficiency
               cache_enabled=True,
               cache_ttl=60.0,  # 1 minute cache
               
               # Load balancing
               load_balancing_strategy=LoadBalancingStrategy.ROUND_ROBIN,
               
               # Aggregation
               aggregation_strategy="weighted_average",
               min_responses=3  # Need at least 3 responses
           )
           
           self.dendrite = Dendrite(config=config)
           self.metagraph = Metagraph(subnet_id=subnet_id)
       
       async def query_miners(self, task_data: dict) -> dict:
           """Query all miners in subnet."""
           # Get active miner endpoints from metagraph
           await self.metagraph.sync()
           
           endpoints = []
           for neuron in self.metagraph.neurons:
               if neuron.is_active and neuron.axon_info:
                   endpoint = f"http://{neuron.axon_info.ip}:{neuron.axon_info.port}/forward"
                   endpoints.append(endpoint)
           
           logger.info(f"Querying {len(endpoints)} miners...")
           
           # Prepare query data
           data = {
               "task": task_data,
               "validator_uid": self.uid,
               "subnet_id": self.subnet_id
           }
           
           # Add authentication
           headers = {
               "Authorization": f"Bearer {self.get_api_key()}",
               "X-Validator-UID": self.uid
           }
           
           try:
               # Query all miners
               result = await self.dendrite.query(
                   endpoints=endpoints,
                   data=data,
                   headers=headers,
                   timeout=30.0,
                   aggregate=True
               )
               
               logger.info(f"Query completed successfully")
               
               # Log metrics
               metrics = self.dendrite.metrics
               logger.info(
                   f"Metrics - Total: {metrics.total_queries}, "
                   f"Success: {metrics.successful_queries}, "
                   f"Failed: {metrics.failed_queries}, "
                   f"Avg time: {metrics.average_response_time:.3f}s"
               )
               
               return result
               
           except Exception as e:
               logger.error(f"Query failed: {e}")
               return None
       
       def get_api_key(self) -> str:
           """Get API key for authentication."""
           return "validator_api_key_here"
       
       async def validate_responses(self, responses: list) -> list:
           """Validate and score miner responses."""
           scores = []
           
           for i, response in enumerate(responses):
               if response is None:
                   scores.append(0.0)
                   continue
               
               # Validate response quality
               score = self.calculate_score(response)
               scores.append(score)
               
               logger.info(f"Miner {i} score: {score:.3f}")
           
           return scores
       
       def calculate_score(self, response: dict) -> float:
           """Calculate quality score for a response."""
           # Implement your scoring logic
           # Based on accuracy, speed, quality, etc.
           return 0.85  # Example score
       
       async def run_validation_loop(self):
           """Main validation loop."""
           logger.info(f"Starting validator {self.uid}...")
           
           while True:
               try:
                   # Generate validation task
                   task = self.generate_task()
                   
                   # Query miners
                   result = await self.query_miners(task)
                   
                   if result:
                       # Process and score responses
                       logger.info(f"Processing validation result...")
                       # Set weights, submit to chain, etc.
                   
                   # Wait before next validation
                   await asyncio.sleep(60)  # 1 minute interval
                   
               except Exception as e:
                   logger.error(f"Validation loop error: {e}")
                   await asyncio.sleep(10)
       
       def generate_task(self) -> dict:
           """Generate validation task."""
           return {"task_type": "forward", "input": "test data"}
   
   async def main():
       validator = Validator(uid="validator_001", subnet_id=1)
       await validator.run_validation_loop()
   
   if __name__ == "__main__":
       asyncio.run(main())

Error Handling
--------------

.. code-block:: python

   async def query_with_error_handling():
       dendrite = Dendrite()
       
       try:
           result = await dendrite.query(
               endpoints=endpoints,
               data=data,
               timeout=30.0
           )
           
           if result is None:
               logger.error("All queries failed")
               return None
           
           return result
           
       except asyncio.TimeoutError:
           logger.error("Query timed out")
           return None
           
       except Exception as e:
           logger.error(f"Query error: {e}")
           return None

Cleanup and Resource Management
--------------------------------

.. code-block:: python

   async def main():
       dendrite = Dendrite()
       
       try:
           # Use dendrite
           result = await dendrite.query(endpoints, data)
           
       finally:
           # Cleanup resources
           await dendrite.close()
   
   # Or use context manager
   async def main():
       async with Dendrite() as dendrite:
           result = await dendrite.query(endpoints, data)
       # Automatically cleaned up

API Reference
-------------

.. automodule:: sdk.dendrite.dendrite
   :members:
   :undoc-members:
   :show-inheritance:

.. automodule:: sdk.dendrite.config
   :members:
   :undoc-members:
   :show-inheritance:

.. automodule:: sdk.dendrite.pool
   :members:
   :undoc-members:
   :show-inheritance:

.. automodule:: sdk.dendrite.aggregator
   :members:
   :undoc-members:
   :show-inheritance:

See Also
--------

* :doc:`axon` - Server component for miners
* :doc:`metagraph` - Network state and topology
* :doc:`guides/subnet_development` - Complete subnet development guide
