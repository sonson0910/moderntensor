Axon Server API
================

The Axon is the server component in ModernTensor's communication layer. It receives and processes requests from Dendrites (clients).

.. note::
   This documentation is under development. The Axon module is being enhanced as part of Phase 2 implementation.

Overview
--------

The Axon provides:

- HTTP/HTTPS server for receiving requests
- Request routing and handling
- Authentication and authorization
- Rate limiting and DDoS protection
- Prometheus metrics integration

Basic Usage
-----------

.. code-block:: python

   from sdk.axon import Axon
   
   # Create Axon server
   axon = Axon(
       port=8080,
       hotkey="your_hotkey",
       coldkey="your_coldkey"
   )
   
   # Register request handler
   @axon.route("/forward")
   async def forward_handler(request):
       # Process request
       return {"result": "success"}
   
   # Start server
   await axon.serve()

.. automodule:: sdk.axon
   :members:
   :undoc-members:
   :show-inheritance:

See Also
--------

* :doc:`dendrite` - Client component
* Protocol definitions
