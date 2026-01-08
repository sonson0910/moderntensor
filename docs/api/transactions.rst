Transaction System API
======================

The ModernTensor transaction system provides a comprehensive set of tools for creating, validating, and managing blockchain transactions.

Transaction Types
-----------------

.. automodule:: sdk.transactions.types
   :members:
   :undoc-members:
   :show-inheritance:

Core Transaction Types
^^^^^^^^^^^^^^^^^^^^^^

TransferTransaction
~~~~~~~~~~~~~~~~~~~

.. autoclass:: sdk.transactions.types.TransferTransaction
   :members:
   :special-members: __init__

StakeTransaction
~~~~~~~~~~~~~~~~

.. autoclass:: sdk.transactions.types.StakeTransaction
   :members:
   :special-members: __init__

UnstakeTransaction
~~~~~~~~~~~~~~~~~~

.. autoclass:: sdk.transactions.types.UnstakeTransaction
   :members:
   :special-members: __init__

RegisterTransaction
~~~~~~~~~~~~~~~~~~~

.. autoclass:: sdk.transactions.types.RegisterTransaction
   :members:
   :special-members: __init__

WeightTransaction
~~~~~~~~~~~~~~~~~

.. autoclass:: sdk.transactions.types.WeightTransaction
   :members:
   :special-members: __init__

Transaction Builder
-------------------

.. automodule:: sdk.transactions.builder
   :members:
   :undoc-members:
   :show-inheritance:

Example Usage
^^^^^^^^^^^^^

.. code-block:: python

   from sdk.transactions import TransactionBuilder
   
   # Build a transfer
   tx = TransactionBuilder() \
       .transfer("addr1", "addr2", 100.0) \
       .with_fee(0.01) \
       .build()
   
   # Build a stake
   stake_tx = TransactionBuilder() \
       .stake("addr1", "hotkey123", 50.0, subnet_id=1) \
       .build()

Batch Transaction Builder
--------------------------

.. automodule:: sdk.transactions.batch
   :members:
   :undoc-members:
   :show-inheritance:

Example Usage
^^^^^^^^^^^^^

.. code-block:: python

   from sdk.transactions import BatchTransactionBuilder
   
   batch = BatchTransactionBuilder(max_concurrent=10)
   batch.add_transaction(tx1)
   batch.add_transaction(tx2)
   
   # Submit all async
   results = await batch.submit_all_async(submit_fn)
   
   # Or submit sync
   results = batch.submit_all_sync(submit_fn)

Transaction Validator
---------------------

.. automodule:: sdk.transactions.validator
   :members:
   :undoc-members:
   :show-inheritance:

Example Usage
^^^^^^^^^^^^^

.. code-block:: python

   from sdk.transactions import TransactionValidator
   
   validator = TransactionValidator(strict=True)
   
   errors = validator.validate(transaction)
   if not errors:
       print("Transaction is valid!")
   
   # Validate batch
   batch_errors = validator.validate_batch([tx1, tx2, tx3])

Transaction Monitor
-------------------

.. automodule:: sdk.transactions.monitor
   :members:
   :undoc-members:
   :show-inheritance:

Example Usage
^^^^^^^^^^^^^

.. code-block:: python

   from sdk.transactions import TransactionMonitor, TransactionStatus
   
   monitor = TransactionMonitor(required_confirmations=3)
   
   # Track transaction
   record = monitor.track(tx_hash, metadata={"amount": 100})
   
   # Wait for confirmation
   status = await monitor.wait_for_confirmation(tx_hash, timeout=60.0)
   
   # Get statistics
   stats = monitor.get_statistics()
   print(f"Total tracked: {stats['total']}")
