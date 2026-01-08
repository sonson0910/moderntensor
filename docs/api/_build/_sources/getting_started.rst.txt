Getting Started with ModernTensor
==================================

Installation
------------

Prerequisites
^^^^^^^^^^^^^

* Python 3.9 or higher
* pip package manager

Install from Source
^^^^^^^^^^^^^^^^^^^

.. code-block:: bash

   git clone https://github.com/sonson0910/moderntensor.git
   cd moderntensor
   pip install -r requirements.txt

Basic Usage
-----------

Transaction Creation
^^^^^^^^^^^^^^^^^^^^

Create and submit a transfer transaction:

.. code-block:: python

   from sdk.transactions import TransactionBuilder
   
   # Build a transfer transaction
   tx = TransactionBuilder() \
       .transfer(
           from_address="addr1",
           to_address="addr2",
           amount=100.0
       ) \
       .with_fee(0.01) \
       .with_memo("Payment for services") \
       .build()
   
   # Submit to blockchain (requires client)
   # result = await client.submit_transaction(tx)

Batch Transactions
^^^^^^^^^^^^^^^^^^

Process multiple transactions efficiently:

.. code-block:: python

   from sdk.transactions import BatchTransactionBuilder, TransactionBuilder
   
   # Create batch builder
   batch = BatchTransactionBuilder(max_concurrent=10)
   
   # Add transactions
   for i in range(5):
       tx = TransactionBuilder().transfer(
           from_address=f"addr{i}",
           to_address=f"addr{i+1}",
           amount=float(i+1) * 100
       ).build()
       batch.add_transaction(tx)
   
   # Submit all with progress tracking
   async def submit_fn(tx):
       return await client.submit_transaction(tx)
   
   results = await batch.submit_all_async(
       submit_fn,
       on_progress=lambda done, total: print(f"{done}/{total}")
   )

Transaction Monitoring
^^^^^^^^^^^^^^^^^^^^^^

Monitor transaction status in real-time:

.. code-block:: python

   from sdk.transactions import TransactionMonitor
   
   # Create monitor
   monitor = TransactionMonitor(required_confirmations=3)
   
   # Track transaction
   tx_hash = await client.submit_transaction(tx)
   monitor.track(tx_hash, metadata={"type": "transfer"})
   
   # Wait for confirmation
   status = await monitor.wait_for_confirmation(
       tx_hash,
       timeout=60.0,
       poll_interval=2.0
   )
   
   if status == TransactionStatus.CONFIRMED:
       print("Transaction confirmed!")

Next Steps
----------

* Read the :doc:`transactions` documentation
* Learn about :doc:`guides/subnet_development`
* Explore the :doc:`axon` and :doc:`dendrite` pattern
