"""
Serving Extrinsics

Transaction builders for serving operations (Axon and Prometheus endpoints).
"""

import logging
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)


def serve_axon(
    client,
    subnet_uid: int,
    hotkey: str,
    ip: str,
    port: int,
    protocol: str = "http",
    version: int = 1,
    private_key: str = None,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Announce an Axon endpoint for a neuron.
    
    Neurons call this to register their Axon (API server) endpoint
    on the blockchain so validators can query them.
    
    Args:
        client: LuxtensorClient instance
        subnet_uid: Subnet UID
        hotkey: Neuron's hotkey
        ip: IP address of the Axon server
        port: Port number
        protocol: Protocol (http, https)
        version: Axon version
        private_key: Hotkey's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        from sdk.extrinsics import serve_axon
        
        result = serve_axon(
            client,
            subnet_uid=1,
            hotkey="5C4hrfjw...",
            ip="192.168.1.100",
            port=8091,
            protocol="http",
            private_key="0x..."
        )
        ```
    """
    logger.info(
        f"Serving Axon for neuron {hotkey[:8]}... "
        f"at {protocol}://{ip}:{port}"
    )
    
    tx_data = {
        "type": "serve_axon",
        "subnet_uid": subnet_uid,
        "hotkey": hotkey,
        "ip": ip,
        "port": port,
        "protocol": protocol,
        "version": version,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "subnet_uid": subnet_uid,
            "hotkey": hotkey,
            "endpoint": f"{protocol}://{ip}:{port}",
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
        
        logger.info(f"Serve Axon successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Serve Axon failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


def serve_prometheus(
    client,
    subnet_uid: int,
    hotkey: str,
    ip: str,
    port: int,
    version: int = 1,
    private_key: str = None,
    wait_for_inclusion: bool = True,
) -> Dict[str, Any]:
    """
    Announce a Prometheus metrics endpoint for a neuron.
    
    Neurons call this to register their Prometheus metrics endpoint
    on the blockchain for monitoring and observability.
    
    Args:
        client: LuxtensorClient instance
        subnet_uid: Subnet UID
        hotkey: Neuron's hotkey
        ip: IP address of the Prometheus server
        port: Port number (typically 9090)
        version: Prometheus version
        private_key: Hotkey's private key for signing
        wait_for_inclusion: Wait for transaction inclusion
        
    Returns:
        Transaction result
        
    Example:
        ```python
        from sdk.extrinsics import serve_prometheus
        
        result = serve_prometheus(
            client,
            subnet_uid=1,
            hotkey="5C4hrfjw...",
            ip="192.168.1.100",
            port=9090,
            private_key="0x..."
        )
        ```
    """
    logger.info(
        f"Serving Prometheus for neuron {hotkey[:8]}... "
        f"at {ip}:{port}"
    )
    
    tx_data = {
        "type": "serve_prometheus",
        "subnet_uid": subnet_uid,
        "hotkey": hotkey,
        "ip": ip,
        "port": port,
        "version": version,
    }
    
    try:
        tx_hash = client.submit_transaction(tx_data, private_key)
        
        result = {
            "success": True,
            "tx_hash": tx_hash,
            "subnet_uid": subnet_uid,
            "hotkey": hotkey,
            "endpoint": f"http://{ip}:{port}/metrics",
        }
        
        if wait_for_inclusion:
            logger.info("Waiting for transaction inclusion...")
        
        logger.info(f"Serve Prometheus successful: {tx_hash}")
        return result
        
    except Exception as e:
        logger.error(f"Serve Prometheus failed: {e}")
        return {
            "success": False,
            "error": str(e),
        }


__all__ = ["serve_axon", "serve_prometheus"]
