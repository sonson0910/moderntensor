Metagraph API
=============

The Metagraph represents the network state and topology in ModernTensor.

.. note::
   This documentation is under development. The Metagraph module is being enhanced as part of Phase 1 implementation.

Overview
--------

The Metagraph provides:

- Network topology representation
- Neuron (node) information storage
- Weight matrix management
- Stake distribution tracking
- Trust scores and rankings

Basic Usage
-----------

.. code-block:: python

   from sdk.metagraph import Metagraph
   
   # Create metagraph for a subnet
   metagraph = Metagraph(subnet_id=1)
   
   # Sync with blockchain
   await metagraph.sync()
   
   # Access neuron information
   neurons = metagraph.neurons
   weights = metagraph.weights
   
   # Query specific neuron
   neuron = metagraph.get_neuron(uid=42)
   print(f"Stake: {neuron.stake}")
   print(f"Trust: {neuron.trust}")

.. automodule:: sdk.metagraph
   :members:
   :undoc-members:
   :show-inheritance:

See Also
--------

* :doc:`transactions` - Transaction system
* Getting started guide
