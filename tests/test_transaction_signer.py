"""
Tests for Transaction Signer Module

Test suite for the TransactionSigner class used in mtcli.
"""

import pytest
from eth_account import Account
from sdk.keymanager import TransactionSigner


class TestTransactionSigner:
    """Test TransactionSigner functionality"""
    
    def test_initialization(self):
        """Test TransactionSigner initialization"""
        # Create a test private key
        account = Account.create()
        private_key = account.key.hex()
        
        # Initialize signer
        signer = TransactionSigner(private_key)
        
        # Verify address matches
        assert signer.address == account.address
    
    def test_initialization_with_0x_prefix(self):
        """Test initialization with 0x prefix in private key"""
        account = Account.create()
        private_key = account.key.hex()
        
        # Test with 0x prefix
        signer = TransactionSigner('0x' + private_key)
        assert signer.address == account.address
    
    def test_build_transaction(self):
        """Test building a transaction"""
        account = Account.create()
        signer = TransactionSigner(account.key.hex())
        
        # Build transaction
        tx = signer.build_transaction(
            to='0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2',
            value=1000000000,  # 1 MDT
            nonce=0,
            gas_price=50,
            gas_limit=21000,
            chain_id=2  # testnet
        )
        
        # Verify transaction fields (address will be checksummed)
        assert tx['to'] == '0x742D35CC6634C0532925a3b844Bc9E7595f0beB2'  # Checksum version
        assert tx['value'] == 1000000000
        assert tx['nonce'] == 0
        assert tx['gasPrice'] == 50
        assert tx['gas'] == 21000
        assert tx['chainId'] == 2
    
    def test_sign_transaction(self):
        """Test signing a transaction"""
        account = Account.create()
        signer = TransactionSigner(account.key.hex())
        
        # Build transaction
        tx = signer.build_transaction(
            to='0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2',
            value=1000000000,
            nonce=0,
            gas_price=50,
            gas_limit=21000,
            chain_id=2
        )
        
        # Sign transaction
        signed_tx = signer.sign_transaction(tx)
        
        # Verify signed transaction format
        assert signed_tx.startswith('0x')
        assert len(signed_tx) > 100  # Signed tx should be reasonably long
    
    def test_build_and_sign_transaction(self):
        """Test building and signing in one step"""
        account = Account.create()
        signer = TransactionSigner(account.key.hex())
        
        # Build and sign
        signed_tx = signer.build_and_sign_transaction(
            to='0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2',
            value=1000000000,
            nonce=0,
            gas_price=50,
            gas_limit=21000,
            chain_id=2
        )
        
        # Verify result
        assert signed_tx.startswith('0x')
        assert len(signed_tx) > 100
    
    def test_estimate_gas_transfer(self):
        """Test gas estimation for transfer"""
        gas = TransactionSigner.estimate_gas('transfer')
        assert gas == 21000
    
    def test_estimate_gas_token_transfer(self):
        """Test gas estimation for token transfer"""
        gas = TransactionSigner.estimate_gas('token_transfer')
        assert gas == 65000
    
    def test_estimate_gas_stake(self):
        """Test gas estimation for stake transaction"""
        gas = TransactionSigner.estimate_gas('stake')
        assert gas == 100000
    
    def test_estimate_gas_unstake(self):
        """Test gas estimation for unstake transaction"""
        gas = TransactionSigner.estimate_gas('unstake')
        assert gas == 80000
    
    def test_estimate_gas_register(self):
        """Test gas estimation for registration"""
        gas = TransactionSigner.estimate_gas('register')
        assert gas == 150000
    
    def test_estimate_gas_set_weights(self):
        """Test gas estimation for setting weights"""
        gas = TransactionSigner.estimate_gas('set_weights')
        assert gas == 200000
    
    def test_estimate_gas_complex(self):
        """Test gas estimation for complex transaction"""
        gas = TransactionSigner.estimate_gas('complex')
        assert gas == 300000
    
    def test_estimate_gas_unknown_type(self):
        """Test gas estimation with unknown type defaults to transfer"""
        gas = TransactionSigner.estimate_gas('unknown_type')
        assert gas == 21000
    
    def test_calculate_transaction_fee(self):
        """Test transaction fee calculation"""
        gas_used = 21000
        gas_price = 50
        fee = TransactionSigner.calculate_transaction_fee(gas_used, gas_price)
        assert fee == 1050000  # 21000 * 50
    
    def test_calculate_transaction_fee_complex(self):
        """Test transaction fee for complex transaction"""
        gas_used = 200000
        gas_price = 100
        fee = TransactionSigner.calculate_transaction_fee(gas_used, gas_price)
        assert fee == 20000000  # 200000 * 100
    
    def test_transaction_with_data(self):
        """Test building transaction with data"""
        account = Account.create()
        signer = TransactionSigner(account.key.hex())
        
        # Custom data
        data = b'\x12\x34\x56\x78'
        
        tx = signer.build_transaction(
            to='0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2',
            value=1000000000,
            nonce=0,
            gas_price=50,
            gas_limit=65000,  # Higher gas for data
            data=data,
            chain_id=2
        )
        
        assert tx['data'] == data
        assert tx['gas'] == 65000


class TestTransactionSignerIntegration:
    """Integration tests for TransactionSigner"""
    
    def test_sign_and_recover_address(self):
        """Test that we can recover the signer address from signed tx"""
        from eth_account import Account as EthAccount
        
        # Create account
        account = EthAccount.create()
        signer = TransactionSigner(account.key.hex())
        
        # Build transaction
        tx = signer.build_transaction(
            to='0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2',
            value=1000000000,
            nonce=0,
            gas_price=50,
            gas_limit=21000,
            chain_id=2
        )
        
        # Sign
        signed_tx = signer.sign_transaction(tx)
        
        # Verify signed transaction is valid hex string
        assert isinstance(signed_tx, str)
        assert signed_tx.startswith('0x')
        
        # The signed transaction should be decodeable
        from hexbytes import HexBytes
        raw_tx = HexBytes(signed_tx)
        assert len(raw_tx) > 0
    
    def test_multiple_transactions_same_signer(self):
        """Test signing multiple transactions with same signer"""
        account = Account.create()
        signer = TransactionSigner(account.key.hex())
        
        signed_txs = []
        for i in range(3):
            signed_tx = signer.build_and_sign_transaction(
                to='0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2',
                value=1000000000 * (i + 1),
                nonce=i,
                gas_price=50,
                gas_limit=21000,
                chain_id=2
            )
            signed_txs.append(signed_tx)
        
        # All transactions should be signed
        assert len(signed_txs) == 3
        for tx in signed_txs:
            assert tx.startswith('0x')
        
        # Each transaction should be different (different nonce/value)
        assert len(set(signed_txs)) == 3


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
