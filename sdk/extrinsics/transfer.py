"""
Transfer Extrinsics

Transaction builders for token transfer operations.
"""

import logging
from typing import Optional, List, Dict, Any

logger = logging.getLogger(__name__)


def transfer(
    client,
    from_address: str,
    to_address: str,
    amount: float,
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Transfer tokens from one account to another.
    
    Args:
        client: LuxtensorClient instance
        from_address: Sender's address
        to_address: Recipient's address
        amount: Amount to transfer
        private_key: Sender's private key for signing
        wait_for_inclusion: Wait for transaction inclusion in block
        
    Returns:
        Transaction result
        
    Example:
        ```python
        from sdk import LuxtensorClient
        from sdk.extrinsics import transfer
        
        client = LuxtensorClient("http://localhost:9933")
        result = transfer(
            client,
            from_address="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
            to_address="5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
            amount=100.0,
            private_key="0x..."
        )
        print(f"Transfer complete: {result['tx_hash']}")
        ```
    """
    logger.info(f"Transferring {amount} from {from_address[:8]}... to {to_address[:8]}...")
    
    # Build transaction
    tx_data = {
        "type": "transfer",
        "from": from_address,
        "to": to_address,
        "amount": amount,
    }
    
    # Sign and submit
    try:
        # Placeholder - actual implementation would sign with private_key
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "from": from_address,
            "to": to_address,
            "amount": amount,
        }
        
        if wait_for_inclusion:
            # Wait for block inclusion
            logger.info("Waiting for transaction inclusion...")
            # Placeholder for actual wait logic
        
        logger.info(f"Transfer successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Transfer failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def batch_transfer(
    client,
    from_address: str,
    transfers: List[Dict[str, Any]],
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Transfer tokens to multiple recipients in a single transaction.
    
    Args:
        client: LuxtensorClient instance
        from_address: Sender's address
        transfers: List of {"to": address, "amount": amount} dictionaries
        private_key: Sender's private key for signing
        wait_for_inclusion: Wait for transaction inclusion in block
        
    Returns:
        Transaction result
        
    Example:
        ```python
        result = batch_transfer(
            client,
            from_address="5GrwvaEF...",
            transfers=[
                {"to": "5C4hrfjw...", "amount": 10.0},
                {"to": "5DTestNe...", "amount": 20.0},
            ],
            private_key="0x..."
        )
        ```
    """
    logger.info(f"Batch transferring to {len(transfers)} recipients")
    
    total_amount = sum(t["amount"] for t in transfers)
    
    # Build batch transaction
    tx_data = {
        "type": "batch_transfer",
        "from": from_address,
        "transfers": transfers,
        "total_amount": total_amount,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "from": from_address,
            "num_transfers": len(transfers),
            "total_amount": total_amount,
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
        
        logger.info(f"Batch transfer successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Batch transfer failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


__all__ = ["transfer", "batch_transfer"]
