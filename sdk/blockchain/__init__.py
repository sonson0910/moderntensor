"""
ModernTensor Layer 1 Blockchain Implementation

This package contains the core blockchain primitives for the ModernTensor L1:
- Block structure and management
- Transaction format and validation  
- State management (Account model)
- Cryptography primitives
- Block validation logic
- MDT token transaction fees
- Key management and HD wallet
"""

from .block import Block, BlockHeader
from .transaction import Transaction
from .state import StateDB, Account
from .mdt_transaction_fees import TransactionFeeHandler, MDTTransactionProcessor
from .crypto import KeyPair
from .l1_keymanager import L1HDWallet, L1Address, L1Network, Network, MAINNET, TESTNET, DEVNET
from .l1_context import L1ChainContext, L1UTxO, BlockFrostChainContext

__all__ = [
    "Block",
    "BlockHeader", 
    "Transaction",
    "StateDB",
    "Account",
    "TransactionFeeHandler",
    "MDTTransactionProcessor",
    "KeyPair",
    "L1HDWallet",
    "L1Address",
    "L1Network",
    "Network",
    "MAINNET",
    "TESTNET",
    "DEVNET",
    "L1ChainContext",
    "L1UTxO",
    "BlockFrostChainContext",  # Backward compatibility
]
