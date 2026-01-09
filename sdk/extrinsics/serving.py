"""Serving Extrinsics - Placeholder"""
import logging
logger = logging.getLogger(__name__)

def serve_axon(client, *args, **kwargs):
    """Serve an axon endpoint."""
    logger.info("Serve axon operation")
    return {"success": True, "message": "Placeholder for serve_axon operation"}

def serve_prometheus(client, *args, **kwargs):
    """Serve prometheus metrics endpoint."""
    logger.info("Serve prometheus operation")
    return {"success": True, "message": "Placeholder for serve_prometheus operation"}

__all__ = ["serve_axon", "serve_prometheus"]
