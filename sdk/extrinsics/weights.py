"""
Weights Extrinsics

Transaction builders for weight setting operations on the Luxtensor blockchain.
Validators use these to set weights for miners in their subnet.
"""

import logging
from typing import Optional, Dict, Any, List

logger = logging.getLogger(__name__)


def set_weights(
    client,
    subnet_uid: int,
    validator_hotkey: str,
    uids: List[int],
    weights: List[float],
    private_key: str,
    version_key: int = 0,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Set weights for miners in a subnet.
    
    Validators call this to assign weights to miners based on their
    performance. Weights determine reward distribution.
    
    Args:
        client: LuxtensorClient instance
        subnet_uid: Subnet UID
        validator_hotkey: Validator's hotkey
        uids: List of miner UIDs
        weights: List of weights (must match UIDs length)
        private_key: Validator's private key for signing
        version_key: Weights version key
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        from sdk.extrinsics import set_weights
        
        result = set_weights(
            client,
            subnet_uid=1,
            validator_hotkey="5C4hrfjw...",
            uids=[0, 1, 2],
            weights=[0.5, 0.3, 0.2],
            private_key="0x..."
        )
        ```
    """
    if len(uids) != len(weights):
        return {
            "success": False,
            "error": "UIDs and weights must have the same length"
        }
    
    logger.info(
        f"Setting weights for {len(uids)} miners on subnet {subnet_uid}..."
    )
    
    tx_data = {
        "type": "set_weights",
        "subnet_uid": subnet_uid,
        "validator": validator_hotkey,
        "uids": uids,
        "weights": weights,
        "version_key": version_key,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "subnet_uid": subnet_uid,
            "num_weights": len(uids),
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
        
        logger.info(f"Set weights successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Set weights failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def commit_weights(
    client,
    subnet_uid: int,
    validator_hotkey: str,
    commit_hash: str,
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Commit weights using commit-reveal scheme (phase 1).
    
    In a commit-reveal scheme, validators first commit a hash of their
    weights, then reveal the actual weights later. This prevents gaming.
    
    Args:
        client: LuxtensorClient instance
        subnet_uid: Subnet UID
        validator_hotkey: Validator's hotkey
        commit_hash: Hash of the weights to commit
        private_key: Validator's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
    """
    logger.info(f"Committing weights on subnet {subnet_uid}...")
    
    tx_data = {
        "type": "commit_weights",
        "subnet_uid": subnet_uid,
        "validator": validator_hotkey,
        "commit_hash": commit_hash,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "subnet_uid": subnet_uid,
            "commit_hash": commit_hash,
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
        
        logger.info(f"Commit weights successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Commit weights failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def reveal_weights(
    client,
    subnet_uid: int,
    validator_hotkey: str,
    uids: List[int],
    weights: List[float],
    salt: str,
    private_key: str,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Reveal weights using commit-reveal scheme (phase 2).
    
    After committing, validators reveal their actual weights along with
    the salt used for hashing. The blockchain verifies the reveal matches
    the earlier commit.
    
    Args:
        client: LuxtensorClient instance
        subnet_uid: Subnet UID
        validator_hotkey: Validator's hotkey
        uids: List of miner UIDs
        weights: List of weights (must match UIDs length)
        salt: Salt used in the original commit hash
        private_key: Validator's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
    """
    if len(uids) != len(weights):
        return {
            "success": False,
            "error": "UIDs and weights must have the same length"
        }
    
    logger.info(f"Revealing weights on subnet {subnet_uid}...")
    
    tx_data = {
        "type": "reveal_weights",
        "subnet_uid": subnet_uid,
        "validator": validator_hotkey,
        "uids": uids,
        "weights": weights,
        "salt": salt,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "subnet_uid": subnet_uid,
            "num_weights": len(uids),
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
        
        logger.info(f"Reveal weights successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Reveal weights failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


__all__ = ["set_weights", "commit_weights", "reveal_weights"]
