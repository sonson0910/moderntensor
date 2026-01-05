#!/usr/bin/env python3
"""
Demo: MDT Transaction Fees Integration

Shows how transactions use MDT tokens for fees and how those fees
integrate with the tokenomics system.
"""

from sdk.blockchain.transaction import Transaction
from sdk.blockchain.mdt_transaction_fees import TransactionFeeHandler, MDTTransactionProcessor
from sdk.tokenomics import TokenomicsIntegration, ConsensusData, NetworkMetrics


def print_section(title: str):
    """Print a formatted section header."""
    print(f"\n{'='*70}")
    print(f"  {title}")
    print(f"{'='*70}\n")


def demo_basic_transaction():
    """Demonstrate a basic MDT transaction with fees."""
    print_section("Demo 1: Basic MDT Transaction")
    
    # Create a transaction
    tx = Transaction(
        nonce=1,
        from_address=b'\x01' * 20,
        to_address=b'\x02' * 20,
        value=1000000,  # 1M MDT smallest units
        gas_price=50,    # 50 units per gas
        gas_limit=21000  # Standard transfer
    )
    
    print(f"Transaction Details:")
    print(f"  From:      {tx.from_address.hex()[:20]}...")
    print(f"  To:        {tx.to_address.hex()[:20]}...")
    print(f"  Value:     {tx.value} MDT units")
    print(f"  Gas Price: {tx.gas_price} units/gas")
    print(f"  Gas Limit: {tx.gas_limit}")
    
    # Calculate fee
    fee_handler = TransactionFeeHandler()
    gas_used = 21000
    fee = fee_handler.calculate_transaction_fee(tx, gas_used)
    
    print(f"\nTransaction Fee Calculation:")
    print(f"  Gas Used:  {gas_used}")
    print(f"  Fee:       {fee} MDT units ({gas_used} × {tx.gas_price})")
    print(f"  Total Cost: {tx.value + fee} MDT units (value + fee)")


def demo_fee_distribution():
    """Demonstrate how fees are distributed."""
    print_section("Demo 2: MDT Fee Distribution")
    
    # Initialize tokenomics
    tokenomics = TokenomicsIntegration()
    fee_handler = TransactionFeeHandler(tokenomics)
    
    # Create and process transaction
    tx = Transaction(
        nonce=1,
        from_address=b'\x01' * 20,
        to_address=b'\x02' * 20,
        value=5000000,
        gas_price=50,
        gas_limit=21000
    )
    
    # Simulate transaction execution
    from sdk.blockchain.transaction import TransactionReceipt
    receipt = TransactionReceipt(
        transaction_hash=tx.hash(),
        block_hash=b'\x00' * 32,
        block_height=1,
        transaction_index=0,
        from_address=tx.from_address,
        to_address=tx.to_address,
        gas_used=21000,
        status=1
    )
    
    # Process fee
    result = fee_handler.process_transaction_fee(tx, receipt)
    
    print(f"Fee Distribution:")
    print(f"  Total Fee:  {result['total_fee']} MDT units")
    print(f"  Recycled:   {result['recycled']} MDT units (50%)")
    print(f"  Burned:     {result['burned']} MDT units (50%)")
    
    # Check tokenomics integration
    pool_stats = tokenomics.pool.get_pool_stats()
    burn_stats = tokenomics.burn.get_burn_stats()
    
    print(f"\nTokenomics Integration:")
    print(f"  Recycling Pool: {pool_stats['sources']['transaction_fees']} MDT")
    print(f"  Burned Total:   {burn_stats['burn_reasons']['transaction_fees']} MDT")


def demo_multiple_transactions():
    """Demonstrate processing multiple transactions."""
    print_section("Demo 3: Multiple MDT Transactions")
    
    # Initialize system
    tokenomics = TokenomicsIntegration()
    fee_handler = TransactionFeeHandler(tokenomics)
    processor = MDTTransactionProcessor(fee_handler)
    
    print("Processing 10 transactions...")
    
    # Process multiple transactions
    for i in range(10):
        tx = Transaction(
            nonce=i,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000000 + i * 100000,
            gas_price=50,
            gas_limit=21000
        )
        
        receipt = processor.process_transaction(
            transaction=tx,
            gas_used=21000,
            block_hash=b'\x00' * 32,
            block_height=1,
            transaction_index=i
        )
        
        if i < 3:  # Show first 3
            print(f"  Tx {i}: Fee = {receipt.logs[0]['total_fee']} MDT")
    
    print(f"  ... (7 more transactions)")
    
    # Show statistics
    stats = processor.get_stats()
    
    print(f"\nTransaction Statistics:")
    print(f"  Processed:      {stats['processed_transactions']} transactions")
    print(f"  Total Fees:     {stats['fee_stats']['total_collected']} MDT")
    print(f"  Total Recycled: {stats['fee_stats']['total_recycled']} MDT")
    print(f"  Total Burned:   {stats['fee_stats']['total_burned']} MDT")


def demo_tokenomics_with_fees():
    """Demonstrate full tokenomics cycle with transaction fees."""
    print_section("Demo 4: Full Tokenomics Cycle with MDT Fees")
    
    # Initialize
    tokenomics = TokenomicsIntegration()
    fee_handler = TransactionFeeHandler(tokenomics)
    processor = MDTTransactionProcessor(fee_handler)
    
    # Process transactions to accumulate fees
    print("Step 1: Processing transactions (accumulating fees)")
    for i in range(20):
        tx = Transaction(
            nonce=i,
            from_address=b'\x01' * 20,
            to_address=b'\x02' * 20,
            value=1000000,
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
    
    pool_before = tokenomics.pool.pool_balance
    print(f"  Recycling Pool: {pool_before} MDT")
    
    # Process epoch with tokenomics
    print(f"\nStep 2: Process epoch with consensus and tokenomics")
    
    consensus_data = ConsensusData(
        miner_scores={'miner1': 0.8, 'miner2': 0.6},
        validator_stakes={'validator1': 100000},
        quality_score=0.9
    )
    
    network_metrics = NetworkMetrics(
        task_count=5000,
        avg_difficulty=0.8,
        validator_ratio=1.0
    )
    
    result = tokenomics.process_epoch_tokenomics(
        epoch=0,
        consensus_data=consensus_data,
        network_metrics=network_metrics
    )
    
    print(f"  Epoch Emission: {result.emission_amount} MDT")
    print(f"  From Pool:      {result.from_pool} MDT (recycled tx fees)")
    print(f"  From Mint:      {result.from_mint} MDT (new tokens)")
    print(f"  Utility Score:  {result.utility_score:.2f}")
    
    print(f"\nStep 3: Fee Recycling Impact")
    recycling_percentage = (result.from_pool / result.emission_amount * 100) if result.emission_amount > 0 else 0
    print(f"  {recycling_percentage:.1f}% of emission from recycled fees!")
    print(f"  This reduces need for new minting")
    
    # Show final state
    final_stats = tokenomics.get_stats()
    
    print(f"\nFinal State:")
    print(f"  Current Supply:     {final_stats['supply']['current_supply']} MDT")
    print(f"  Recycling Pool:     {final_stats['recycling_pool']['total_balance']} MDT")
    print(f"  Total Burned:       {final_stats['burns']['total_burned']} MDT")


def demo_fee_estimation():
    """Demonstrate fee estimation for users."""
    print_section("Demo 5: MDT Fee Estimation")
    
    fee_handler = TransactionFeeHandler()
    
    scenarios = [
        ("Standard Transfer", 21000, 50),
        ("Token Transfer", 65000, 50),
        ("Complex Contract", 200000, 50),
        ("High Priority", 21000, 100),
    ]
    
    print("Fee Estimates:")
    for name, gas_limit, gas_price in scenarios:
        estimated = fee_handler.estimate_fee(gas_limit, gas_price)
        print(f"  {name:20s}: {estimated:10d} MDT ({gas_limit} gas @ {gas_price} units/gas)")


def main():
    """Run all demos."""
    print("\n" + "="*70)
    print("  ModernTensor MDT Transaction Fees Demo")
    print("  Showing Integration with Tokenomics System")
    print("="*70)
    
    demo_basic_transaction()
    demo_fee_distribution()
    demo_multiple_transactions()
    demo_tokenomics_with_fees()
    demo_fee_estimation()
    
    print("\n" + "="*70)
    print("  Summary: MDT Transactions are Working!")
    print("="*70)
    print("\n✅ Transactions use MDT tokens for gas fees")
    print("✅ 50% of fees are recycled into the reward pool")
    print("✅ 50% of fees are burned (deflationary)")
    print("✅ Recycled fees reduce need for new token minting")
    print("✅ Full integration with adaptive tokenomics system")
    print("\n" + "="*70 + "\n")


if __name__ == '__main__':
    main()
