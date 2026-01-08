ModernTensor SDK Documentation
================================

Welcome to ModernTensor SDK's documentation. ModernTensor is a next-generation decentralized AI/ML network built on a custom Layer 1 blockchain (Luxtensor).

.. toctree::
   :maxdepth: 2
   :caption: Contents:

   getting_started
   transactions
   axon
   dendrite
   metagraph
   guides/transaction_usage
   guides/subnet_development

Getting Started
===============

Installation
------------

.. code-block:: bash

   pip install moderntensor

Quick Start
-----------

.. code-block:: python

   from sdk.transactions import TransactionBuilder
   
   # Create a transfer transaction
   tx = TransactionBuilder() \
       .transfer("sender_addr", "recipient_addr", 100.0) \
       .with_fee(0.01) \
       .build()

Key Features
============

* **Custom Layer 1 Blockchain**: Built on Luxtensor, a Rust-based PoS blockchain
* **Transaction System**: Comprehensive transaction types (transfer, stake, register, etc.)
* **Axon/Dendrite Pattern**: Server/client architecture for AI/ML communication
* **Metagraph**: Network state management and topology
* **zkML Integration**: Zero-knowledge machine learning support

Indices and tables
==================

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
