Transaction Usage Guide
=======================

This guide covers common transaction patterns and best practices for the ModernTensor SDK.

Basic Transactions
------------------

Transfer Tokens
^^^^^^^^^^^^^^^

The simplest transaction type is a token transfer:

.. code-block:: python

   from sdk.transactions import TransactionBuilder
   
   tx = TransactionBuilder() \
       .transfer(
           from_address="your_address",
           to_address="recipient_address",
           amount=100.0
       ) \
       .with_fee(0.01) \
       .with_memo("Payment") \
       .build()

Stake Tokens
^^^^^^^^^^^^

Stake tokens to a hotkey for subnet participation:

.. code-block:: python

   tx = TransactionBuilder() \
       .stake(
           from_address="your_address",
           hotkey="hotkey_address",
           amount=1000.0,
           subnet_id=1
       ) \
       .build()

Register Neuron
^^^^^^^^^^^^^^^

Register a neuron on a subnet:

.. code-block:: python

   tx = TransactionBuilder() \
       .register(
           from_address="your_address",
           hotkey="hotkey_address",
           subnet_id=1
       ) \
       .build()

Advanced Patterns
-----------------

Batch Processing
^^^^^^^^^^^^^^^^

Process multiple transactions efficiently:

.. code-block:: python

   from sdk.transactions import BatchTransactionBuilder
   
   # Create batch
   batch = BatchTransactionBuilder(max_concurrent=5)
   
   # Add multiple transactions
   addresses = [("addr1", "addr2"), ("addr3", "addr4"), ("addr5", "addr6")]
   for from_addr, to_addr in addresses:
       tx = TransactionBuilder() \
           .transfer(from_addr, to_addr, 100.0) \
           .build()
       batch.add_transaction(tx)
   
   # Submit with progress tracking
   def on_progress(completed, total):
       print(f"Progress: {completed}/{total} ({completed/total*100:.1f}%)")
   
   results = await batch.submit_all_async(
       submit_fn=client.submit_transaction,
       on_progress=on_progress
   )
   
   # Check results
   successes = sum(1 for r in results if not isinstance(r, Exception))
   print(f"Completed: {successes}/{len(results)} transactions")

Transaction Validation
^^^^^^^^^^^^^^^^^^^^^^

Validate transactions before submission:

.. code-block:: python

   from sdk.transactions import TransactionValidator, ValidationError
   
   validator = TransactionValidator(strict=True)
   
   try:
       errors = validator.validate(transaction)
       if errors:
           print("Validation errors:")
           for error in errors:
               print(f"  - {error}")
       else:
           # Transaction is valid, submit it
           result = await client.submit_transaction(transaction)
   except ValidationError as e:
       print(f"Validation failed: {e}")

Transaction Monitoring
^^^^^^^^^^^^^^^^^^^^^^

Monitor transaction status in real-time:

.. code-block:: python

   from sdk.transactions import TransactionMonitor, TransactionStatus
   
   # Create monitor
   monitor = TransactionMonitor(required_confirmations=3)
   
   # Submit and track
   tx_hash = await client.submit_transaction(tx)
   record = monitor.track(
       tx_hash,
       metadata={
           "type": "transfer",
           "amount": 100.0,
           "timestamp": datetime.now()
       }
   )
   
   # Wait for confirmation with timeout
   status = await monitor.wait_for_confirmation(
       tx_hash,
       timeout=120.0,
       poll_interval=5.0
   )
   
   if status == TransactionStatus.CONFIRMED:
       record = monitor.get_record(tx_hash)
       duration = record.duration_seconds()
       print(f"Confirmed in {duration:.2f} seconds")
   elif status == TransactionStatus.TIMEOUT:
       print("Transaction timed out")
   else:
       print(f"Transaction failed: {record.error}")

Best Practices
--------------

1. Always Validate
^^^^^^^^^^^^^^^^^^

Validate transactions before submission to catch errors early:

.. code-block:: python

   validator = TransactionValidator(strict=True)
   errors = validator.validate(tx)
   if errors:
       # Handle validation errors
       return

2. Use Batch Processing
^^^^^^^^^^^^^^^^^^^^^^^^

For multiple transactions, use batch processing for better performance:

.. code-block:: python

   # Good: Use batch processing
   batch = BatchTransactionBuilder(max_concurrent=10)
   for tx in transactions:
       batch.add_transaction(tx)
   results = await batch.submit_all_async(submit_fn)
   
   # Avoid: Sequential submission
   # for tx in transactions:
   #     await submit_fn(tx)  # Slow!

3. Set Appropriate Fees
^^^^^^^^^^^^^^^^^^^^^^^

Always set transaction fees to ensure timely processing:

.. code-block:: python

   tx = TransactionBuilder() \
       .transfer(from_addr, to_addr, amount) \
       .with_fee(0.01)  # Set appropriate fee \
       .build()

4. Monitor Important Transactions
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Use transaction monitoring for critical operations:

.. code-block:: python

   monitor = TransactionMonitor(required_confirmations=3)
   tx_hash = await client.submit_transaction(tx)
   monitor.track(tx_hash)
   
   # Wait for confirmation
   status = await monitor.wait_for_confirmation(tx_hash, timeout=60)

5. Handle Errors Gracefully
^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Always handle transaction errors:

.. code-block:: python

   try:
       result = await client.submit_transaction(tx)
   except ValueError as e:
       print(f"Invalid transaction: {e}")
   except ConnectionError as e:
       print(f"Network error: {e}")
   except Exception as e:
       print(f"Unexpected error: {e}")

Common Pitfalls
---------------

1. Not Checking Balance
^^^^^^^^^^^^^^^^^^^^^^^^

Always ensure sufficient balance before creating transactions.

2. Ignoring Validation Errors
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Don't skip validation - it catches errors before they reach the blockchain.

3. Blocking Async Code
^^^^^^^^^^^^^^^^^^^^^^^

When using async methods, don't block the event loop:

.. code-block:: python

   # Good: Use async properly
   async def process_transactions():
       results = await batch.submit_all_async(submit_fn)
   
   # Bad: Mixing sync and async
   # results = batch.submit_all_async(submit_fn)  # Missing await!

4. Not Setting Timeouts
^^^^^^^^^^^^^^^^^^^^^^^

Always set timeouts when waiting for confirmations:

.. code-block:: python

   # Good: Set timeout
   status = await monitor.wait_for_confirmation(tx_hash, timeout=60.0)
   
   # Bad: No timeout (could wait forever)
   # status = await monitor.wait_for_confirmation(tx_hash)
