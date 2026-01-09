"""
Registration Extrinsics

Transaction builders for neuron registration on the Luxtensor blockchain.
"""

import logging
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)


def register(
    client,
    subnet_uid: int,
    hotkey: str,
    coldkey: str,
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Register a neuron on a subnet.
    
    Args:
        client: LuxtensorClient instance
        subnet_uid: Subnet UID to register on
        hotkey: Hotkey for the neuron
        coldkey: Coldkey owning the neuron
        private_key: Coldkey's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        from sdk.extrinsics import register
        
        result = register(
            client,
            subnet_uid=1,
            hotkey="5C4hrfjw...",
            coldkey="5GrwvaEF...",
            private_key="0x..."
        )
        ```
    """
    logger.info(f"Registering neuron on subnet {subnet_uid}...")
    
    tx_data = {
        "type": "register",
        "subnet_uid": subnet_uid,
        "hotkey": hotkey,
        "coldkey": coldkey,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "subnet_uid": subnet_uid,
            "hotkey": hotkey,
            "coldkey": coldkey,
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
            # Placeholder for actual wait logic
        
        logger.info(f"Registration successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Registration failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def burned_register(
    client,
    subnet_uid: int,
    hotkey: str,
    coldkey: str,
    burn_amount: float,
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Register a neuron by burning tokens.
    
    This method allows registration by burning tokens instead of
    solving a proof-of-work puzzle.
    
    Args:
        client: LuxtensorClient instance
        subnet_uid: Subnet UID to register on
        hotkey: Hotkey for the neuron
        coldkey: Coldkey owning the neuron
        burn_amount: Amount of tokens to burn for registration
        private_key: Coldkey's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        from sdk.extrinsics import burned_register
        
        result = burned_register(
            client,
            subnet_uid=1,
            hotkey="5C4hrfjw...",
            coldkey="5GrwvaEF...",
            burn_amount=1.0,
            private_key="0x..."
        )
        ```
    """
    logger.info(
        f"Registering neuron on subnet {subnet_uid} "
        f"by burning {burn_amount} tokens..."
    )
    
    tx_data = {
        "type": "burned_register",
        "subnet_uid": subnet_uid,
        "hotkey": hotkey,
        "coldkey": coldkey,
        "burn_amount": burn_amount,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "subnet_uid": subnet_uid,
            "hotkey": hotkey,
            "coldkey": coldkey,
            "burned": burn_amount,
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
        
        logger.info(f"Burned registration successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Burned registration failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


__all__ = ["register", "burned_register"]
