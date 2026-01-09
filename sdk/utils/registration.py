"""
Registration Utilities

Helper functions for registration operations.
"""

import logging
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)


def check_registration_status(
    client,
    hotkey: str,
    subnet_uid: int
) -> Dict[str, Any]:
    """
    Check if a neuron is registered on a subnet.
    
    Args:
        client: LuxtensorClient instance
        hotkey: Hotkey to check
        subnet_uid: Subnet UID
        
    Returns:
        Dictionary with registration status
        
    Example:
        ```python
        from sdk.utils import check_registration_status
        
        status = check_registration_status(
            client,
            hotkey="5C4hrfjw...",
            subnet_uid=1
        )
        print(status)  # {"registered": True, "uid": 5, ...}
        ```
    """
    try:
        neurons = client.get_neurons(subnet_uid)
        
        for neuron in neurons:
            if neuron.hotkey == hotkey:
                return {
                    "registered": True,
                    "uid": neuron.uid,
                    "subnet_uid": subnet_uid,
                    "active": neuron.active,
                    "stake": neuron.stake,
                }
        
        return {
            "registered": False,
            "subnet_uid": subnet_uid,
        }
        
    except Exception as e:
        logger.error(f"Error checking registration status: {e}")
        return {
            "registered": False,
            "error": str(e),
        }


def get_registration_cost(
    client,
    subnet_uid: int
) -> Optional[float]:
    """
    Get the current registration cost (burn amount) for a subnet.
    
    Args:
        client: LuxtensorClient instance
        subnet_uid: Subnet UID
        
    Returns:
        Registration cost in tokens, or None if error
        
    Example:
        ```python
        from sdk.utils import get_registration_cost
        
        cost = get_registration_cost(client, subnet_uid=1)
        print(f"Registration cost: {cost} MTAO")
        ```
    """
    try:
        subnet_info = client.get_subnet_info(subnet_uid)
        
        if subnet_info:
            return subnet_info.burn
        
        return None
        
    except Exception as e:
        logger.error(f"Error getting registration cost: {e}")
        return None


__all__ = ["check_registration_status", "get_registration_cost"]
