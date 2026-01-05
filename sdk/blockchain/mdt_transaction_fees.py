"""
Transaction Fee Handler for MDT Token Integration.

This module handles transaction fees in MDT tokens and integrates them
with the tokenomics system (recycling pool and burning).
"""

from typing import Optional
from sdk.blockchain.transaction import Transaction, TransactionReceipt


class TransactionFeeHandler:
    """
    Handles transaction fees in MDT tokens.
    
    Features:
    - Calculates fees from gas_used * gas_price
    - Routes 50% of fees to recycling pool
    - Burns 50% of fees for deflationary pressure
    - Integrates with tokenomics system
    """
    
    def __init__(self, tokenomics_integration=None):
        """
        Initialize fee handler.
        
        Args:
            tokenomics_integration: Optional TokenomicsIntegration instance
        """
        self.tokenomics = tokenomics_integration
        self.total_fees_collected = 0
        self.total_fees_recycled = 0
        self.total_fees_burned = 0
        self.burn_percentage = 0.5  # 50% burned, 50% recycled
    
    def calculate_transaction_fee(
        self,
        transaction: Transaction,
        gas_used: int
    ) -> int:
        """
        Calculate transaction fee in MDT tokens.
        
        Formula: fee = gas_used × gas_price
        
        Args:
            transaction: The transaction
            gas_used: Actual gas consumed
            
        Returns:
            Transaction fee in MDT smallest unit
        """
        if gas_used > transaction.gas_limit:
            raise ValueError(f"Gas used ({gas_used}) exceeds gas limit ({transaction.gas_limit})")
        
        return gas_used * transaction.gas_price
    
    def process_transaction_fee(
        self,
        transaction: Transaction,
        receipt: TransactionReceipt
    ) -> dict:
        """
        Process transaction fee - split between recycling and burning.
        
        Args:
            transaction: The executed transaction
            receipt: Transaction receipt with gas_used
            
        Returns:
            Dictionary with fee breakdown
        """
        # Calculate total fee
        total_fee = self.calculate_transaction_fee(transaction, receipt.gas_used)
        
        # Split: 50% to recycling pool, 50% burned
        fee_to_recycle = int(total_fee * (1 - self.burn_percentage))
        fee_to_burn = int(total_fee * self.burn_percentage)
        
        # Track totals
        self.total_fees_collected += total_fee
        self.total_fees_recycled += fee_to_recycle
        self.total_fees_burned += fee_to_burn
        
        # Integrate with tokenomics if available
        if self.tokenomics:
            # Add to recycling pool
            self.tokenomics.add_to_recycling_pool(fee_to_recycle, 'transaction_fees')
            
            # Burn tokens
            self.tokenomics.burn.burn_transaction_fees(total_fee, self.burn_percentage)
        
        return {
            'total_fee': total_fee,
            'recycled': fee_to_recycle,
            'burned': fee_to_burn,
            'burn_percentage': self.burn_percentage
        }
    
    def estimate_fee(
        self,
        gas_limit: int,
        gas_price: int
    ) -> int:
        """
        Estimate maximum transaction fee.
        
        Args:
            gas_limit: Maximum gas for transaction
            gas_price: Price per unit of gas
            
        Returns:
            Maximum possible fee
        """
        return gas_limit * gas_price
    
    def get_stats(self) -> dict:
        """
        Get transaction fee statistics.
        
        Returns:
            Dictionary with fee stats
        """
        recycling_rate = 0.0
        burning_rate = 0.0
        
        if self.total_fees_collected > 0:
            recycling_rate = self.total_fees_recycled / self.total_fees_collected
            burning_rate = self.total_fees_burned / self.total_fees_collected
        
        return {
            'total_collected': self.total_fees_collected,
            'total_recycled': self.total_fees_recycled,
            'total_burned': self.total_fees_burned,
            'recycling_rate': recycling_rate,
            'burning_rate': burning_rate,
            'burn_percentage': self.burn_percentage
        }


class MDTTransactionProcessor:
    """
    Processes transactions with MDT token fee handling.
    
    This integrates the transaction system with MDT tokenomics.
    """
    
    def __init__(self, fee_handler: Optional[TransactionFeeHandler] = None):
        """
        Initialize transaction processor.
        
        Args:
            fee_handler: TransactionFeeHandler for fee processing
        """
        self.fee_handler = fee_handler or TransactionFeeHandler()
        self.processed_transactions = 0
    
    def process_transaction(
        self,
        transaction: Transaction,
        gas_used: int,
        block_hash: bytes,
        block_height: int,
        transaction_index: int
    ) -> TransactionReceipt:
        """
        Process a transaction and handle MDT fees.
        
        Args:
            transaction: Transaction to process
            gas_used: Gas consumed by execution
            block_hash: Hash of containing block
            block_height: Height of containing block
            transaction_index: Index in block
            
        Returns:
            TransactionReceipt with fee details
        """
        # Verify transaction has enough gas
        if gas_used > transaction.gas_limit:
            # Transaction failed due to out of gas
            status = 0
        else:
            status = 1
        
        # Create receipt
        receipt = TransactionReceipt(
            transaction_hash=transaction.hash(),
            block_hash=block_hash,
            block_height=block_height,
            transaction_index=transaction_index,
            from_address=transaction.from_address,
            to_address=transaction.to_address,
            gas_used=gas_used,
            status=status
        )
        
        # Process fee with MDT tokenomics integration
        if status == 1:  # Only process fees for successful transactions
            fee_breakdown = self.fee_handler.process_transaction_fee(
                transaction,
                receipt
            )
            
            # Add fee info to receipt logs
            receipt.logs.append({
                'type': 'mdt_fee',
                'total_fee': fee_breakdown['total_fee'],
                'recycled': fee_breakdown['recycled'],
                'burned': fee_breakdown['burned']
            })
        
        self.processed_transactions += 1
        
        return receipt
    
    def get_stats(self) -> dict:
        """
        Get processor statistics.
        
        Returns:
            Dictionary with processor stats
        """
        return {
            'processed_transactions': self.processed_transactions,
            'fee_stats': self.fee_handler.get_stats()
        }
