Subnet Development Guide
========================

This guide covers how to develop and deploy subnets on ModernTensor.

.. note::
   This guide is under development as subnet framework is being enhanced in Phase 5.

What is a Subnet?
-----------------

A subnet in ModernTensor is a specialized network within the main network, focused on a specific task or domain. Each subnet:

- Has its own set of miners and validators
- Implements custom validation logic
- Can have unique tokenomics parameters
- Operates independently while connected to the main chain

Getting Started
---------------

1. Clone the Subnet Template
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: bash

   git clone https://github.com/sonson0910/moderntensor.git
   cd moderntensor/examples/subnet_template

2. Implement Your Validator
^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: python

   from sdk.subnets import BaseValidator
   
   class MyValidator(BaseValidator):
       def __init__(self, config):
           super().__init__(config)
       
       async def forward(self, synapse):
           """Process incoming requests."""
           # Your validation logic here
           return response
       
       async def score_responses(self, responses):
           """Score miner responses."""
           scores = []
           for response in responses:
               score = self.calculate_score(response)
               scores.append(score)
           return scores

3. Implement Your Miner
^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: python

   from sdk.subnets import BaseMiner
   
   class MyMiner(BaseMiner):
       def __init__(self, config):
           super().__init__(config)
       
       async def forward(self, synapse):
           """Process validator requests."""
           # Your mining logic here
           result = self.process(synapse.data)
           return result

4. Deploy Your Subnet
^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: bash

   # Register subnet
   mtcli subnet create --name "My Subnet" --netuid 1
   
   # Start validator
   python validator.py --subnet 1 --hotkey validator_key
   
   # Start miner
   python miner.py --subnet 1 --hotkey miner_key

Best Practices
--------------

1. Design Clear Validation Logic
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Make your scoring mechanism transparent and fair:

.. code-block:: python

   def calculate_score(self, response):
       # Clear, measurable criteria
       accuracy = self.check_accuracy(response)
       speed = self.check_speed(response)
       
       # Weighted combination
       score = 0.7 * accuracy + 0.3 * speed
       return score

2. Handle Errors Gracefully
^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: python

   async def forward(self, synapse):
       try:
           result = await self.process(synapse)
           return result
       except Exception as e:
           logger.error(f"Processing failed: {e}")
           return None

3. Optimize Performance
^^^^^^^^^^^^^^^^^^^^^^^

- Use async/await for I/O operations
- Implement caching where appropriate
- Monitor resource usage

4. Test Thoroughly
^^^^^^^^^^^^^^^^^^

.. code-block:: python

   # Test your validator
   pytest tests/test_validator.py
   
   # Test your miner
   pytest tests/test_miner.py

Advanced Topics
---------------

Custom Reward Mechanisms
^^^^^^^^^^^^^^^^^^^^^^^^

Implement custom reward distribution:

.. code-block:: python

   def distribute_rewards(self, scores):
       # Normalize scores
       total = sum(scores)
       normalized = [s / total for s in scores]
       
       # Apply custom distribution
       rewards = []
       for score in normalized:
           reward = self.calculate_reward(score)
           rewards.append(reward)
       
       return rewards

Integration with External Services
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Connect your subnet to external APIs or databases:

.. code-block:: python

   async def fetch_external_data(self, query):
       async with httpx.AsyncClient() as client:
           response = await client.get(
               f"https://api.example.com/data?q={query}"
           )
           return response.json()

See Also
--------

* :doc:`transactions` - Transaction usage guide
* Examples directory in the repository
