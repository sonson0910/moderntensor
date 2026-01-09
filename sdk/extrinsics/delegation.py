"""
Delegation Extrinsics

Transaction builders for delegation operations.
Allows token holders to delegate stake to validators.
"""

import logging
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)


def delegate(
    client,
    delegator_address: str,
    validator_hotkey: str,
    amount: float,
    private_key: str = None,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Delegate stake to a validator.
    
    Args:
        client: LuxtensorClient instance
        delegator_address: Delegator's address
        validator_hotkey: Validator's hotkey to delegate to
        amount: Amount to delegate
        private_key: Delegator's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        from sdk.extrinsics import delegate
        
        result = delegate(
            client,
            delegator_address="5GrwvaEF...",
            validator_hotkey="5C4hrfjw...",
            amount=1000.0,
            private_key="0x..."
        )
        ```
    """
    logger.info(
        f"Delegating {amount} from {delegator_address[:8]}... "
        f"to validator {validator_hotkey[:8]}..."
    )
    
    tx_data = {
        "type": "delegate",
        "delegator": delegator_address,
        "validator": validator_hotkey,
        "amount": amount,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "delegator": delegator_address,
            "validator": validator_hotkey,
            "amount": amount,
        }
        
        logger.info(f"Delegation successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Delegation failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def undelegate(
    client,
    delegator_address: str,
    validator_hotkey: str,
    amount: float,
    private_key: str = None,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Undelegate stake from a validator.
    
    Args:
        client: LuxtensorClient instance
        delegator_address: Delegator's address
        validator_hotkey: Validator's hotkey to undelegate from
        amount: Amount to undelegate
        private_key: Delegator's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
    """
    logger.info(
        f"Undelegating {amount} from validator {validator_hotkey[:8]}..."
    )
    
    tx_data = {
        "type": "undelegate",
        "delegator": delegator_address,
        "validator": validator_hotkey,
        "amount": amount,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "delegator": delegator_address,
            "validator": validator_hotkey,
            "amount": amount,
        }
        
        logger.info(f"Undelegation successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Undelegation failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def nominate(
    client,
    nominator_address: str,
    nominees: list,
    private_key: str = None,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Nominate validators for rewards.
    
    Args:
        client: LuxtensorClient instance
        nominator_address: Nominator's address
        nominees: List of validator hotkeys to nominate
        private_key: Nominator's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        result = nominate(
            client,
            nominator_address="5GrwvaEF...",
            nominees=["5C4hrfjw...", "5DTestNe..."],
            private_key="0x..."
        )
        ```
    """
    logger.info(f"Nominating {len(nominees)} validators")
    
    tx_data = {
        "type": "nominate",
        "nominator": nominator_address,
        "nominees": nominees,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "nominator": nominator_address,
            "num_nominees": len(nominees),
        }
        
        logger.info(f"Nomination successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Nomination failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


__all__ = ["delegate", "undelegate", "nominate"]
