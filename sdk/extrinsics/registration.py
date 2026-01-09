"""Registration Extrinsics - Placeholder"""
import logging
logger = logging.getLogger(__name__)

def register(client, *args, **kwargs):
    """Register a neuron on the network."""
    logger.info("Registration operation")
    return {"success": True, "message": "Placeholder for register operation"}

def burned_register(client, *args, **kwargs):
    """Register by burning tokens."""
    logger.info("Burned registration operation")
    return {"success": True, "message": "Placeholder for burned_register operation"}

__all__ = ["register", "burned_register"]
