"""
Proxy Extrinsics

Transaction builders for proxy account operations.
Allows one account to perform operations on behalf of another.
"""

import logging
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)


def add_proxy(
    client,
    delegator_address: str,
    proxy_address: str,
    proxy_type: str = "Any",
    delay_blocks: int = 0,
    private_key: str = None,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Add a proxy account.
    
    Args:
        client: LuxtensorClient instance
        delegator_address: Delegator (owner) account address
        proxy_address: Proxy account address
        proxy_type: Type of proxy ("Any", "Staking", "Transfer", "Governance")
        delay_blocks: Number of blocks to delay proxy actions
        private_key: Delegator's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        from sdk.extrinsics import add_proxy
        
        result = add_proxy(
            client,
            delegator_address="5GrwvaEF...",
            proxy_address="5C4hrfjw...",
            proxy_type="Staking",
            private_key="0x..."
        )
        ```
    """
    logger.info(
        f"Adding proxy: {proxy_address[:8]}... for {delegator_address[:8]}... "
        f"(type: {proxy_type})"
    )
    
    tx_data = {
        "type": "add_proxy",
        "delegator": delegator_address,
        "proxy": proxy_address,
        "proxy_type": proxy_type,
        "delay_blocks": delay_blocks,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "delegator": delegator_address,
            "proxy": proxy_address,
            "proxy_type": proxy_type,
        }
        
        logger.info(f"Proxy added successfully: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Add proxy failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def remove_proxy(
    client,
    delegator_address: str,
    proxy_address: str,
    proxy_type: str = "Any",
    private_key: str = None,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Remove a proxy account.
    
    Args:
        client: LuxtensorClient instance
        delegator_address: Delegator (owner) account address
        proxy_address: Proxy account address to remove
        proxy_type: Type of proxy to remove
        private_key: Delegator's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
    """
    logger.info(f"Removing proxy: {proxy_address[:8]}...")
    
    tx_data = {
        "type": "remove_proxy",
        "delegator": delegator_address,
        "proxy": proxy_address,
        "proxy_type": proxy_type,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "delegator": delegator_address,
            "proxy": proxy_address,
        }
        
        logger.info(f"Proxy removed successfully: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Remove proxy failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def proxy_call(
    client,
    proxy_address: str,
    delegator_address: str,
    call_data: Dict[str, Any],
    private_key: str = None,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Execute a call through a proxy account.
    
    Args:
        client: LuxtensorClient instance
        proxy_address: Proxy account making the call
        delegator_address: Delegator (real) account
        call_data: The actual call to execute
        private_key: Proxy's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        # Transfer tokens on behalf of delegator
        result = proxy_call(
            client,
            proxy_address="5C4hrfjw...",
            delegator_address="5GrwvaEF...",
            call_data={
                "type": "transfer",
                "to": "5DTestNe...",
                "amount": 100.0
            },
            private_key="0x..."
        )
        ```
    """
    logger.info(f"Executing proxy call for {delegator_address[:8]}...")
    
    tx_data = {
        "type": "proxy_call",
        "proxy": proxy_address,
        "delegator": delegator_address,
        "call": call_data,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "proxy": proxy_address,
            "delegator": delegator_address,
            "call_type": call_data.get("type"),
        }
        
        logger.info(f"Proxy call successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Proxy call failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


__all__ = ["add_proxy", "remove_proxy", "proxy_call"]
