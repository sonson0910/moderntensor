"""
Test MDT transaction fee integration with tokenomics.
"""

import pytest
from sdk.blockchain.transaction import Transaction, TransactionReceipt
from sdk.blockchain.mdt_transaction_fees import TransactionFeeHandler, MDTTransactionProcessor
from sdk.tokenomics import TokenomicsIntegration


class TestTransactionFeeHandler:
    """Test cases for TransactionFeeHandler."""
    
    def test_calculate_fee(self):
        """Test fee calculation."""
        handler = TransactionFeeHandler()
        
        tx = Transaction(
            nonce=1,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=50,
            gas_limit=21000
        )
        
        gas_used = 21000
        fee = handler.calculate_transaction_fee(tx, gas_used)
        
        assert fee == 1050000  # 21000 * 50
    
    def test_calculate_fee_exceeds_limit(self):
        """Test that fee calculation fails if gas exceeds limit."""
        handler = TransactionFeeHandler()
        
        tx = Transaction(
            nonce=1,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=50,
            gas_limit=21000
        )
        
        with pytest.raises(ValueError, match="exceeds gas limit"):
            handler.calculate_transaction_fee(tx, 25000)
    
    def test_process_fee_without_tokenomics(self):
        """Test fee processing without tokenomics integration."""
        handler = TransactionFeeHandler()
        
        tx = Transaction(
            nonce=1,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=50,
            gas_limit=21000
        )
        
        receipt = TransactionReceipt(
            transaction_hash=tx.hash(),
            block_hash=b'\x00' * 32,
            block_height=1,
            transaction_index=0,
            from_address=tx.from_address,
            to_address=tx.to_address,
            gas_used=21000
        )
        
        result = handler.process_transaction_fee(tx, receipt)
        
        assert result['total_fee'] == 1050000
        assert result['recycled'] == 525000  # 50%
        assert result['burned'] == 525000    # 50%
        assert result['burn_percentage'] == 0.5
    
    def test_process_fee_with_tokenomics(self):
        """Test fee processing with tokenomics integration."""
        tokenomics = TokenomicsIntegration()
        handler = TransactionFeeHandler(tokenomics)
        
        tx = Transaction(
            nonce=1,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=50,
            gas_limit=21000
        )
        
        receipt = TransactionReceipt(
            transaction_hash=tx.hash(),
            block_hash=b'\x00' * 32,
            block_height=1,
            transaction_index=0,
            from_address=tx.from_address,
            to_address=tx.to_address,
            gas_used=21000
        )
        
        result = handler.process_transaction_fee(tx, receipt)
        
        # Verify fees were processed
        assert result['total_fee'] == 1050000
        
        # Check recycling pool received fees
        pool_stats = tokenomics.pool.get_pool_stats()
        assert pool_stats['sources']['transaction_fees'] == 525000
        
        # Check tokens were burned
        burn_stats = tokenomics.burn.get_burn_stats()
        assert burn_stats['burn_reasons']['transaction_fees'] == 525000
    
    def test_estimate_fee(self):
        """Test fee estimation."""
        handler = TransactionFeeHandler()
        
        estimated = handler.estimate_fee(gas_limit=21000, gas_price=50)
        assert estimated == 1050000
    
    def test_get_stats(self):
        """Test statistics collection."""
        handler = TransactionFeeHandler()
        
        tx = Transaction(
            nonce=1,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=50,
            gas_limit=21000
        )
        
        receipt = TransactionReceipt(
            transaction_hash=tx.hash(),
            block_hash=b'\x00' * 32,
            block_height=1,
            transaction_index=0,
            from_address=tx.from_address,
            to_address=tx.to_address,
            gas_used=21000
        )
        
        handler.process_transaction_fee(tx, receipt)
        handler.process_transaction_fee(tx, receipt)
        
        stats = handler.get_stats()
        
        assert stats['total_collected'] == 2100000
        assert stats['total_recycled'] == 1050000
        assert stats['total_burned'] == 1050000
        assert stats['recycling_rate'] == 0.5
        assert stats['burning_rate'] == 0.5


class TestMDTTransactionProcessor:
    """Test cases for MDTTransactionProcessor."""
    
    def test_process_successful_transaction(self):
        """Test processing a successful transaction."""
        tokenomics = TokenomicsIntegration()
        fee_handler = TransactionFeeHandler(tokenomics)
        processor = MDTTransactionProcessor(fee_handler)
        
        tx = Transaction(
            nonce=1,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=50,
            gas_limit=21000
        )
        
        receipt = processor.process_transaction(
            transaction=tx,
            gas_used=21000,
            block_hash=b'\x00' * 32,
            block_height=1,
            transaction_index=0
        )
        
        assert receipt.status == 1
        assert receipt.gas_used == 21000
        assert len(receipt.logs) == 1
        assert receipt.logs[0]['type'] == 'mdt_fee'
        assert receipt.logs[0]['total_fee'] == 1050000
    
    def test_process_failed_transaction(self):
        """Test processing a failed transaction (out of gas)."""
        processor = MDTTransactionProcessor()
        
        tx = Transaction(
            nonce=1,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000,
            gas_price=50,
            gas_limit=21000
        )
        
        receipt = processor.process_transaction(
            transaction=tx,
            gas_used=25000,  # Exceeds limit
            block_hash=b'\x00' * 32,
            block_height=1,
            transaction_index=0
        )
        
        assert receipt.status == 0  # Failed
        assert len(receipt.logs) == 0  # No fee processing for failed tx
    
    def test_processor_stats(self):
        """Test processor statistics."""
        tokenomics = TokenomicsIntegration()
        fee_handler = TransactionFeeHandler(tokenomics)
        processor = MDTTransactionProcessor(fee_handler)
        
        # Process multiple transactions
        for i in range(3):
            tx = Transaction(
                nonce=i,
                from_address=b'\x01' * 20,
                to_address=b'\x02' * 20,
                value=1000,
                gas_price=50,
                gas_limit=21000
            )
            
            processor.process_transaction(
                transaction=tx,
                gas_used=21000,
                block_hash=b'\x00' * 32,
                block_height=1,
                transaction_index=i
            )
        
        stats = processor.get_stats()
        
        assert stats['processed_transactions'] == 3
        assert stats['fee_stats']['total_collected'] == 3150000


class TestIntegrationWithTokenomics:
    """Integration tests showing MDT fees flowing into tokenomics."""
    
    def test_full_transaction_lifecycle(self):
        """Test complete transaction lifecycle with tokenomics."""
        # Initialize tokenomics
        tokenomics = TokenomicsIntegration()
        
        # Create transaction processor with tokenomics
        fee_handler = TransactionFeeHandler(tokenomics)
        processor = MDTTransactionProcessor(fee_handler)
        
        # Process several transactions
        for i in range(10):
            tx = Transaction(
                nonce=i,
                from_address=b'\x01' * 20,
                to_address=b'\x02' * 20,
                value=1000,
                gas_price=50,
                gas_limit=21000
            )
            
            processor.process_transaction(
                transaction=tx,
                gas_used=21000,
                block_hash=b'\x00' * 32,
                block_height=1,
                transaction_index=i
            )
        
        # Verify fees went to recycling pool
        pool_stats = tokenomics.pool.get_pool_stats()
        assert pool_stats['sources']['transaction_fees'] == 5250000  # 10 * 525000
        
        # Verify fees were burned
        burn_stats = tokenomics.burn.get_burn_stats()
        assert burn_stats['burn_reasons']['transaction_fees'] == 5250000
        
        # Check processor stats
        proc_stats = processor.get_stats()
        assert proc_stats['processed_transactions'] == 10
        assert proc_stats['fee_stats']['total_collected'] == 10500000
