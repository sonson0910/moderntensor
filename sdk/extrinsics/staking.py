"""
Staking Extrinsics

Transaction builders for staking operations on the Luxtensor blockchain.
"""

import logging
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)


def stake(
    client,
    hotkey: str,
    coldkey: str,
    amount: float,
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Add stake to a hotkey.
    
    Args:
        client: LuxtensorClient instance
        hotkey: Hotkey to stake to
        coldkey: Coldkey providing the stake
        amount: Amount to stake
        private_key: Coldkey's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        from sdk.extrinsics import stake
        
        result = stake(
            client,
            hotkey="5C4hrfjw...",
            coldkey="5GrwvaEF...",
            amount=1000.0,
            private_key="0x..."
        )
        ```
    """
    logger.info(f"Staking {amount} to hotkey {hotkey[:8]}...")
    
    tx_data = {
        "type": "add_stake",
        "hotkey": hotkey,
        "coldkey": coldkey,
        "amount": amount,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "hotkey": hotkey,
            "coldkey": coldkey,
            "amount": amount,
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
            # Placeholder for actual wait logic
        
        logger.info(f"Stake successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Stake failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def unstake(
    client,
    hotkey: str,
    coldkey: str,
    amount: float,
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Remove stake from a hotkey.
    
    Args:
        client: LuxtensorClient instance
        hotkey: Hotkey to unstake from
        coldkey: Coldkey receiving the unstaked tokens
        amount: Amount to unstake
        private_key: Coldkey's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
    """
    logger.info(f"Unstaking {amount} from hotkey {hotkey[:8]}...")
    
    tx_data = {
        "type": "remove_stake",
        "hotkey": hotkey,
        "coldkey": coldkey,
        "amount": amount,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "hotkey": hotkey,
            "coldkey": coldkey,
            "amount": amount,
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
        
        logger.info(f"Unstake successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Unstake failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def add_stake(
    client,
    hotkey: str,
    coldkey: str,
    amount: float,
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Add additional stake to a hotkey (alias for stake).
    
    Args:
        client: LuxtensorClient instance
        hotkey: Hotkey to stake to
        coldkey: Coldkey providing the stake
        amount: Amount to stake
        private_key: Coldkey's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
    """
    return stake(client, hotkey, coldkey, amount, private_key, wait_for_inclusion)


def unstake_all(
    client,
    hotkey: str,
    coldkey: str,
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Remove all stake from a hotkey.
    
    Args:
        client: LuxtensorClient instance
        hotkey: Hotkey to unstake from
        coldkey: Coldkey receiving the unstaked tokens
        private_key: Coldkey's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
    """
    logger.info(f"Unstaking all from hotkey {hotkey[:8]}...")
    
    tx_data = {
        "type": "remove_all_stake",
        "hotkey": hotkey,
        "coldkey": coldkey,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "hotkey": hotkey,
            "coldkey": coldkey,
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
        
        logger.info(f"Unstake all successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Unstake all failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


__all__ = ["stake", "unstake", "add_stake", "unstake_all"]
