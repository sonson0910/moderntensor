Axon Server API
================

The Axon is the server component in ModernTensor's communication layer. It is a production-ready HTTP/HTTPS server that miners and validators use to receive and process requests from Dendrites (clients).

Overview
--------

The Axon provides a complete server solution with:

- **HTTP/HTTPS Server**: FastAPI-based server with async support
- **Security Features**: Authentication, rate limiting, blacklist/whitelist, DDoS protection
- **Request Handling**: Flexible endpoint routing and handler attachment
- **Monitoring**: Prometheus metrics, health checks, request logging
- **Production Ready**: SSL/TLS support, connection pooling, graceful shutdown

Architecture
------------

The Axon consists of several components:

1. **Core Server (axon.py)**: Main Axon class that integrates all components
2. **Configuration (config.py)**: AxonConfig for server settings and AxonMetrics for monitoring
3. **Security (security.py)**: SecurityManager for authentication and access control
4. **Middleware**: Authentication, rate limiting, blacklist, DDoS protection, logging

Configuration
-------------

Server Configuration
~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from sdk.axon import Axon, AxonConfig
   
   # Create configuration
   config = AxonConfig(
       host="0.0.0.0",              # Bind to all interfaces
       port=8091,                    # Server port
       external_ip="203.0.113.42",  # Public IP address
       external_port=8091,           # Public port
       
       # Security settings
       authentication_enabled=True,
       rate_limiting_enabled=True,
       rate_limit_requests=100,      # Max requests per window
       rate_limit_window=60,         # Window in seconds
       
       # DDoS protection
       ddos_protection_enabled=True,
       max_concurrent_requests=50,
       request_timeout=30,
       
       # Monitoring
       metrics_enabled=True,
       log_requests=True,
       log_level="INFO",
       
       # Server metadata
       uid="miner_001",
       api_version="v1"
   )
   
   # Create Axon with configuration
   axon = Axon(config=config)

SSL/TLS Configuration
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   config = AxonConfig(
       host="0.0.0.0",
       port=443,
       
       # Enable SSL
       ssl_enabled=True,
       ssl_certfile="/path/to/cert.pem",
       ssl_keyfile="/path/to/key.pem",
       
       uid="secure_miner_001"
   )
   
   axon = Axon(config=config)

Basic Usage
-----------

Creating and Starting an Axon
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from sdk.axon import Axon, AxonConfig
   import asyncio
   
   async def main():
       # Create Axon server
       config = AxonConfig(
           host="0.0.0.0",
           port=8091,
           uid="miner_001"
       )
       axon = Axon(config=config)
       
       # Define request handler
       async def forward_handler(request):
           """Handle forward requests from validators."""
           data = await request.json()
           
           # Process the request
           result = process_forward(data)
           
           return {"success": True, "result": result}
       
       # Attach handler to endpoint
       axon.attach("/forward", forward_handler, methods=["POST"])
       
       # Start server (blocking)
       await axon.start()
   
   if __name__ == "__main__":
       asyncio.run(main())

Attaching Multiple Endpoints
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   async def forward_handler(request):
       """Handle forward pass requests."""
       data = await request.json()
       return {"result": model.forward(data)}
   
   async def backward_handler(request):
       """Handle backward pass requests."""
       data = await request.json()
       return {"gradients": model.backward(data)}
   
   async def info_handler(request):
       """Return miner information."""
       return {
           "uid": axon.config.uid,
           "model_type": "transformer",
           "version": "1.0.0"
       }
   
   # Attach all handlers
   axon.attach("/forward", forward_handler, methods=["POST"])
   axon.attach("/backward", backward_handler, methods=["POST"])
   axon.attach("/info", info_handler, methods=["GET"])

Security Features
-----------------

API Key Authentication
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   # Register API key for a validator
   api_key = axon.register_api_key("validator_uid_123")
   print(f"API Key: {api_key}")
   
   # Clients must include this in headers:
   # Authorization: Bearer <api_key>
   
   # Revoke API key
   axon.revoke_api_key("validator_uid_123")

IP Blacklisting
~~~~~~~~~~~~~~~

.. code-block:: python

   # Blacklist malicious IPs
   axon.blacklist_ip("192.0.2.100")
   axon.blacklist_ip("198.51.100.50")
   
   # Or configure in AxonConfig
   config = AxonConfig(
       blacklist_enabled=True,
       blacklist_ips=["192.0.2.100", "198.51.100.50"]
   )

IP Whitelisting
~~~~~~~~~~~~~~~

.. code-block:: python

   # Only allow specific IPs
   config = AxonConfig(
       whitelist_enabled=True,
       whitelist_ips=[
           "203.0.113.10",  # Validator 1
           "203.0.113.20",  # Validator 2
       ]
   )
   axon = Axon(config=config)
   
   # Add more IPs dynamically
   axon.whitelist_ip("203.0.113.30")

Rate Limiting
~~~~~~~~~~~~~

.. code-block:: python

   # Configure rate limiting
   config = AxonConfig(
       rate_limiting_enabled=True,
       rate_limit_requests=100,  # Max 100 requests
       rate_limit_window=60      # Per 60 seconds
   )
   
   # Requests exceeding limit get HTTP 429 (Too Many Requests)

Monitoring and Metrics
----------------------

Health Check
~~~~~~~~~~~~

The Axon provides built-in health check endpoint:

.. code-block:: bash

   curl http://localhost:8091/health

Response:

.. code-block:: json

   {
       "status": "healthy",
       "uptime": 3600.5,
       "uid": "miner_001"
   }

Prometheus Metrics
~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   curl http://localhost:8091/metrics

Response:

.. code-block:: json

   {
       "total_requests": 1523,
       "successful_requests": 1498,
       "failed_requests": 25,
       "blocked_requests": 12,
       "average_response_time": 0.15,
       "uptime_seconds": 3600.5,
       "active_connections": 5
   }

Server Information
~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   curl http://localhost:8091/info

Response:

.. code-block:: json

   {
       "uid": "miner_001",
       "version": "v1",
       "host": "0.0.0.0",
       "port": 8091,
       "external_ip": "203.0.113.42",
       "external_port": 8091,
       "ssl_enabled": false,
       "uptime": 3600.5,
       "started_at": "2026-01-08T10:30:00"
   }

Advanced Usage
--------------

Custom Middleware
~~~~~~~~~~~~~~~~~

.. code-block:: python

   from starlette.middleware.base import BaseHTTPMiddleware
   
   class CustomMiddleware(BaseHTTPMiddleware):
       async def dispatch(self, request, call_next):
           # Pre-processing
           print(f"Request to {request.url.path}")
           
           response = await call_next(request)
           
           # Post-processing
           response.headers["X-Custom-Header"] = "ModernTensor"
           
           return response
   
   # Add custom middleware
   axon.app.add_middleware(CustomMiddleware)

Background Tasks
~~~~~~~~~~~~~~~~

.. code-block:: python

   import asyncio
   
   async def periodic_task():
       """Run periodic task in background."""
       while True:
           # Perform task
           print("Running periodic task...")
           await asyncio.sleep(60)
   
   async def main():
       axon = Axon(config=config)
       
       # Start background task
       task = asyncio.create_task(periodic_task())
       
       # Start server (non-blocking)
       await axon.start(blocking=False)
       
       # Do other work...
       await asyncio.sleep(3600)
       
       # Stop server
       await axon.stop()
       task.cancel()

Request Context and State
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   async def handler(request):
       """Access request context."""
       # Get client IP
       client_ip = request.client.host
       
       # Get headers
       auth_header = request.headers.get("authorization")
       
       # Get query parameters
       params = request.query_params
       
       # Access app state
       app_state = request.app.state
       
       return {"client_ip": client_ip}

Error Handling
--------------

.. code-block:: python

   from fastapi import HTTPException
   
   async def handler(request):
       try:
           data = await request.json()
           
           # Validate input
           if "required_field" not in data:
               raise HTTPException(
                   status_code=400,
                   detail="Missing required field"
               )
           
           # Process request
           result = process(data)
           return {"result": result}
           
       except ValueError as e:
           raise HTTPException(
               status_code=422,
               detail=f"Invalid data: {str(e)}"
           )
       except Exception as e:
           logger.error(f"Handler error: {e}")
           raise HTTPException(
               status_code=500,
               detail="Internal server error"
           )

Complete Example: Production Miner
-----------------------------------

.. code-block:: python

   from sdk.axon import Axon, AxonConfig
   import asyncio
   import logging
   
   logging.basicConfig(level=logging.INFO)
   logger = logging.getLogger(__name__)
   
   class Miner:
       def __init__(self, uid: str):
           self.uid = uid
           self.model = self.load_model()
           
           # Configure Axon
           config = AxonConfig(
               host="0.0.0.0",
               port=8091,
               external_ip="203.0.113.42",
               external_port=8091,
               
               # Security
               authentication_enabled=True,
               rate_limiting_enabled=True,
               rate_limit_requests=100,
               rate_limit_window=60,
               blacklist_enabled=True,
               ddos_protection_enabled=True,
               max_concurrent_requests=50,
               
               # Monitoring
               metrics_enabled=True,
               log_requests=True,
               
               uid=uid
           )
           
           self.axon = Axon(config=config)
           self._setup_handlers()
       
       def load_model(self):
           """Load ML model."""
           logger.info("Loading model...")
           return MockModel()  # Replace with actual model
       
       def _setup_handlers(self):
           """Setup request handlers."""
           self.axon.attach("/forward", self.forward_handler, methods=["POST"])
           self.axon.attach("/backward", self.backward_handler, methods=["POST"])
           self.axon.attach("/status", self.status_handler, methods=["GET"])
       
       async def forward_handler(self, request):
           """Handle forward pass."""
           try:
               data = await request.json()
               
               # Validate validator
               validator_uid = data.get("validator_uid")
               if not validator_uid:
                   return {"error": "Missing validator_uid"}, 400
               
               # Process forward pass
               result = self.model.forward(data["input"])
               
               return {
                   "success": True,
                   "miner_uid": self.uid,
                   "result": result
               }
           except Exception as e:
               logger.error(f"Forward handler error: {e}")
               return {"error": str(e)}, 500
       
       async def backward_handler(self, request):
           """Handle backward pass."""
           data = await request.json()
           gradients = self.model.backward(data["loss"])
           return {"gradients": gradients}
       
       async def status_handler(self, request):
           """Return miner status."""
           return {
               "uid": self.uid,
               "status": "active",
               "model_type": "transformer",
               "requests_processed": self.axon.metrics.total_requests
           }
       
       async def start(self):
           """Start miner."""
           logger.info(f"Starting miner {self.uid}...")
           
           # Register API keys for known validators
           for validator_uid in ["val1", "val2", "val3"]:
               api_key = self.axon.register_api_key(validator_uid)
               logger.info(f"Registered validator {validator_uid}: {api_key}")
           
           # Start Axon server
           await self.axon.start()
   
   async def main():
       miner = Miner(uid="miner_001")
       await miner.start()
   
   if __name__ == "__main__":
       asyncio.run(main())

API Reference
-------------

.. automodule:: sdk.axon.axon
   :members:
   :undoc-members:
   :show-inheritance:

.. automodule:: sdk.axon.config
   :members:
   :undoc-members:
   :show-inheritance:

.. automodule:: sdk.axon.security
   :members:
   :undoc-members:
   :show-inheritance:

.. automodule:: sdk.axon.middleware
   :members:
   :undoc-members:
   :show-inheritance:

See Also
--------

* :doc:`dendrite` - Client component for querying miners
* :doc:`synapse` - Protocol definitions for communication
* :doc:`guides/subnet_development` - Complete subnet development guide
