"""
Tests for Luxtensor Transaction Module

Test suite for the Luxtensor transaction creation and signing.
Uses native Python cryptography matching Luxtensor's Rust implementation.
"""

import pytest
from sdk.keymanager import KeyGenerator
from sdk.transactions import (
    LuxtensorTransaction,
    create_transfer_transaction,
    sign_transaction,
    verify_transaction_signature,
    encode_transaction_for_rpc,
    estimate_gas_for_transfer,
    estimate_gas_for_contract_call,
    calculate_transaction_fee,
    derive_address_from_private_key
)


class TestLuxtensorTransaction:
    """Test LuxtensorTransaction functionality"""
    
    def test_transaction_creation(self):
        """Test creating a transaction"""
        tx = LuxtensorTransaction(
            nonce=0,
            from_address="0x742D35CC6634C0532925a3b844Bc9E7595f0beB2",
            to_address="0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3",
            value=1000000000,
            gas_price=50,
            gas_limit=21000,
            data=b''
        )
        
        assert tx.nonce == 0
        assert tx.from_address == "0x742D35CC6634C0532925a3b844Bc9E7595f0beB2"
        assert tx.to_address == "0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3"
        assert tx.value == 1000000000
        assert tx.gas_price == 50
        assert tx.gas_limit == 21000
        assert tx.data == b''
    
    def test_signing_message_generation(self):
        """Test signing message generation"""
        tx = LuxtensorTransaction(
            nonce=5,
            from_address="0x742D35CC6634C0532925a3b844Bc9E7595f0beB2",
            to_address="0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3",
            value=2000000000,
            gas_price=100,
            gas_limit=30000,
            data=b'\x12\x34'
        )
        
        message = tx.get_signing_message()
        
        # Check message is not empty
        assert len(message) > 0
        
        # Message should contain:
        # 8 bytes (nonce) + 20 bytes (from) + 20 bytes (to) + 16 bytes (value) + 8 bytes (gas_price) + 8 bytes (gas_limit) + data
        expected_min_length = 8 + 20 + 20 + 16 + 8 + 8 + 2
        assert len(message) >= expected_min_length
    
    def test_transaction_signing(self):
        """Test transaction signing"""
        # Create account
        kg = KeyGenerator()
        account = kg.generate_keypair()
        private_key = account['private_key']
        
        # Create transaction
        tx = LuxtensorTransaction(
            nonce=0,
            from_address=account['address'],
            to_address="0x742D35CC6634C0532925a3b844Bc9E7595f0beB2",
            value=1000000000,
            gas_price=50,
            gas_limit=21000,
            data=b''
        )
        
        # Sign
        signed_tx = sign_transaction(tx, private_key)
        
        # Verify signature components are set
        assert signed_tx.v > 0
        assert len(signed_tx.r) == 32
        assert len(signed_tx.s) == 32
        assert signed_tx.r != b'\x00' * 32
        assert signed_tx.s != b'\x00' * 32
    
    def test_transaction_hash(self):
        """Test transaction hash calculation"""
        tx = LuxtensorTransaction(
            nonce=0,
            from_address="0x742D35CC6634C0532925a3b844Bc9E7595f0beB2",
            to_address="0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3",
            value=1000000000,
            gas_price=50,
            gas_limit=21000,
            data=b''
        )
        
        hash1 = tx.hash()
        hash2 = tx.hash()
        
        # Hash should be consistent
        assert hash1 == hash2
        
        # Hash should be 32 bytes
        assert len(hash1) == 32
    
    def test_to_dict(self):
        """Test converting transaction to dictionary"""
        kg = KeyGenerator()
        account = kg.generate_keypair()
        private_key = account['private_key']
        
        tx = LuxtensorTransaction(
            nonce=0,
            from_address=account['address'],
            to_address="0x742D35CC6634C0532925a3b844Bc9E7595f0beB2",
            value=1000000000,
            gas_price=50,
            gas_limit=21000,
            data=b'\x12\x34'
        )
        
        signed_tx = sign_transaction(tx, private_key)
        tx_dict = signed_tx.to_dict()
        
        assert tx_dict['nonce'] == 0
        assert tx_dict['from'] == account['address']
        assert tx_dict['to'] == "0x742D35CC6634C0532925a3b844Bc9E7595f0beB2"
        assert tx_dict['value'] == 1000000000
        assert tx_dict['gasPrice'] == 50
        assert tx_dict['gasLimit'] == 21000
        assert 'v' in tx_dict
        assert 'r' in tx_dict
        assert 's' in tx_dict


class TestTransactionFunctions:
    """Test transaction helper functions"""
    
    def test_create_transfer_transaction(self):
        """Test creating a transfer transaction"""
        kg = KeyGenerator()
        account = kg.generate_keypair()
        private_key = account['private_key']
        
        tx_dict = create_transfer_transaction(
            from_address=account['address'],
            to_address="0x742D35CC6634C0532925a3b844Bc9E7595f0beB2",
            amount=1000000000,
            nonce=0,
            private_key=private_key
        )
        
        assert isinstance(tx_dict, dict)
        assert tx_dict['nonce'] == 0
        assert tx_dict['from'] == account['address']
        assert tx_dict['value'] == 1000000000
        assert 'v' in tx_dict
        assert 'r' in tx_dict
        assert 's' in tx_dict
    
    def test_encode_transaction_for_rpc(self):
        """Test encoding transaction for RPC"""
        kg = KeyGenerator()
        account = kg.generate_keypair()
        private_key = account['private_key']
        
        tx = LuxtensorTransaction(
            nonce=0,
            from_address=account['address'],
            to_address="0x742D35CC6634C0532925a3b844Bc9E7595f0beB2",
            value=1000000000,
            gas_price=50,
            gas_limit=21000,
            data=b''
        )
        
        signed_tx = sign_transaction(tx, private_key)
        encoded = encode_transaction_for_rpc(signed_tx)
        
        # Should be hex string with 0x prefix
        assert encoded.startswith('0x')
        assert len(encoded) > 100  # Should be reasonably long
    
    def test_estimate_gas_for_transfer(self):
        """Test gas estimation for transfer"""
        gas = estimate_gas_for_transfer()
        assert gas == 21000
    
    def test_estimate_gas_for_contract_call(self):
        """Test gas estimation for contract call"""
        # Empty data
        gas = estimate_gas_for_contract_call(0)
        assert gas == 21000
        
        # 100 bytes of data
        gas = estimate_gas_for_contract_call(100)
        assert gas == 21000 + (100 * 68)
    
    def test_calculate_transaction_fee(self):
        """Test transaction fee calculation"""
        fee = calculate_transaction_fee(21000, 50)
        assert fee == 1050000


class TestTransactionIntegration:
    """Integration tests for transactions"""
    
    def test_full_transaction_flow(self):
        """Test complete transaction flow"""
        # Create account
        kg = KeyGenerator()
        account = kg.generate_keypair()
        private_key = account['private_key']
        
        # Create transaction
        tx = LuxtensorTransaction(
            nonce=0,
            from_address=account['address'],
            to_address="0x742D35CC6634C0532925a3b844Bc9E7595f0beB2",
            value=1000000000,
            gas_price=50,
            gas_limit=21000,
            data=b''
        )
        
        # Sign
        signed_tx = sign_transaction(tx, private_key)
        
        # Verify signature components
        assert signed_tx.v > 0
        assert len(signed_tx.r) == 32
        assert len(signed_tx.s) == 32
        
        # Encode for RPC
        encoded = encode_transaction_for_rpc(signed_tx)
        assert encoded.startswith('0x')
        
        # Convert to dict
        tx_dict = signed_tx.to_dict()
        assert isinstance(tx_dict, dict)
    
    def test_multiple_transactions(self):
        """Test creating multiple transactions"""
        kg = KeyGenerator()
        account = kg.generate_keypair()
        private_key = account['private_key']
        
        txs = []
        for i in range(3):
            tx = LuxtensorTransaction(
                nonce=i,
                from_address=account['address'],
                to_address="0x742D35CC6634C0532925a3b844Bc9E7595f0beB2",
                value=1000000000 * (i + 1),
                gas_price=50,
                gas_limit=21000,
                data=b''
            )
            signed_tx = sign_transaction(tx, private_key)
            txs.append(signed_tx)
        
        # All should be signed
        assert len(txs) == 3
        for tx in txs:
            assert tx.v > 0
            assert len(tx.r) == 32
            assert len(tx.s) == 32
        
        # Each should have different nonce
        nonces = [tx.nonce for tx in txs]
        assert nonces == [0, 1, 2]


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
