Dendrite Client API
===================

The Dendrite is the client component in ModernTensor's communication layer. It sends requests to Axon servers.

.. note::
   This documentation is under development. The Dendrite module is being enhanced as part of Phase 2 implementation.

Overview
--------

The Dendrite provides:

- Async HTTP client for querying miners
- Connection pooling
- Query routing and load balancing
- Response aggregation
- Timeout management

Basic Usage
-----------

.. code-block:: python

   from sdk.dendrite import Dendrite
   
   # Create Dendrite client
   dendrite = Dendrite(
       hotkey="your_hotkey",
       coldkey="your_coldkey"
   )
   
   # Query a miner
   response = await dendrite.query(
       axon_info,
       synapse=MySynapse(data="query")
   )
   
   # Query multiple miners
   responses = await dendrite.query_multiple(
       axon_infos,
       synapse=MySynapse(data="query")
   )

.. automodule:: sdk.dendrite
   :members:
   :undoc-members:
   :show-inheritance:

See Also
--------

* :doc:`axon` - Server component
* Protocol definitions
