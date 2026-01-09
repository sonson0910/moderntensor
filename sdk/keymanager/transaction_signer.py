"""
Transaction Signing Module

Provides functionality for building and signing transactions for Luxtensor blockchain.
"""

from typing import Dict, Any, Optional
from eth_account import Account
from eth_account.signers.local import LocalAccount
from eth_typing import HexStr
from hexbytes import HexBytes
from eth_utils import to_checksum_address


class TransactionSigner:
    """
    Transaction signer for Luxtensor blockchain
    
    Handles transaction building, signing, and encoding.
    """
    
    def __init__(self, private_key: str):
        """
        Initialize transaction signer with private key
        
        Args:
            private_key: Private key in hex format (with or without 0x prefix)
        """
        # Remove 0x prefix if present
        if private_key.startswith('0x'):
            private_key = private_key[2:]
        
        self.account: LocalAccount = Account.from_key(private_key)
        self.address = self.account.address
    
    def build_transaction(
        self,
        to: str,
        value: int,
        nonce: int,
        gas_price: int,
        gas_limit: int = 21000,
        data: bytes = b'',
        chain_id: int = 1
    ) -> Dict[str, Any]:
        """
        Build a transaction dictionary
        
        Args:
            to: Recipient address
            value: Amount to send (in base units)
            nonce: Transaction nonce
            gas_price: Gas price (in base units)
            gas_limit: Gas limit (default 21000 for simple transfer)
            data: Transaction data (default empty for simple transfer)
            chain_id: Chain ID (1 for mainnet, 2 for testnet)
        
        Returns:
            Transaction dictionary
        """
        # Convert to checksum address
        to_checksum = to_checksum_address(to)
        
        transaction = {
            'to': to_checksum,
            'value': value,
            'gas': gas_limit,
            'gasPrice': gas_price,
            'nonce': nonce,
            'chainId': chain_id,
            'data': data if isinstance(data, bytes) else HexBytes(data)
        }
        
        return transaction
    
    def sign_transaction(self, transaction: Dict[str, Any]) -> HexStr:
        """
        Sign a transaction
        
        Args:
            transaction: Transaction dictionary
        
        Returns:
            Signed transaction as hex string (with 0x prefix)
        """
        signed = self.account.sign_transaction(transaction)
        return HexStr(signed.rawTransaction.hex())
    
    def build_and_sign_transaction(
        self,
        to: str,
        value: int,
        nonce: int,
        gas_price: int,
        gas_limit: int = 21000,
        data: bytes = b'',
        chain_id: int = 1
    ) -> HexStr:
        """
        Build and sign a transaction in one step
        
        Args:
            to: Recipient address
            value: Amount to send (in base units)
            nonce: Transaction nonce
            gas_price: Gas price (in base units)
            gas_limit: Gas limit (default 21000)
            data: Transaction data (default empty)
            chain_id: Chain ID
        
        Returns:
            Signed transaction as hex string
        """
        transaction = self.build_transaction(
            to=to,
            value=value,
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit,
            data=data,
            chain_id=chain_id
        )
        
        return self.sign_transaction(transaction)
    
    @staticmethod
    def estimate_gas(transaction_type: str = 'transfer') -> int:
        """
        Estimate gas for different transaction types
        
        Args:
            transaction_type: Type of transaction (transfer, stake, etc.)
        
        Returns:
            Estimated gas limit
        """
        gas_estimates = {
            'transfer': 21000,
            'token_transfer': 65000,
            'stake': 100000,
            'unstake': 80000,
            'register': 150000,
            'set_weights': 200000,
            'complex': 300000
        }
        
        return gas_estimates.get(transaction_type, 21000)
    
    @staticmethod
    def calculate_transaction_fee(gas_used: int, gas_price: int) -> int:
        """
        Calculate transaction fee
        
        Args:
            gas_used: Amount of gas used
            gas_price: Gas price per unit
        
        Returns:
            Total transaction fee (in base units)
        """
        return gas_used * gas_price
