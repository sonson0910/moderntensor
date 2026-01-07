"""
Protocol version management for Synapse.

Handles version negotiation and compatibility checking.
"""

from typing import Tuple
from enum import Enum


class ProtocolVersion(str, Enum):
    """Protocol version enumeration."""
    V1_0 = "1.0"
    V1_1 = "1.1"
    V2_0 = "2.0"


# Current protocol version
CURRENT_VERSION = ProtocolVersion.V1_0

# Minimum supported version for backward compatibility
MIN_SUPPORTED_VERSION = ProtocolVersion.V1_0


def parse_version(version_str: str) -> Tuple[int, int]:
    """
    Parse version string to tuple.
    
    Args:
        version_str: Version string (e.g., "1.0")
        
    Returns:
        Tuple of (major, minor) version numbers
    """
    try:
        parts = version_str.split(".")
        major = int(parts[0])
        minor = int(parts[1]) if len(parts) > 1 else 0
        return (major, minor)
    except (ValueError, IndexError):
        raise ValueError(f"Invalid version format: {version_str}")


def version_compatible(client_version: str, server_version: str) -> bool:
    """
    Check if client and server versions are compatible.
    
    Args:
        client_version: Client protocol version
        server_version: Server protocol version
        
    Returns:
        True if versions are compatible
    """
    try:
        client_major, client_minor = parse_version(client_version)
        server_major, server_minor = parse_version(server_version)
        
        # Major versions must match
        if client_major != server_major:
            return False
        
        # Minor version differences are acceptable (backward compatible)
        return True
        
    except ValueError:
        return False


def negotiate_version(client_versions: list, server_versions: list) -> str:
    """
    Negotiate protocol version between client and server.
    
    Args:
        client_versions: List of versions supported by client
        server_versions: List of versions supported by server
        
    Returns:
        Negotiated version string
        
    Raises:
        ValueError: If no compatible version found
    """
    # Find the highest common version
    common_versions = set(client_versions) & set(server_versions)
    
    if not common_versions:
        raise ValueError(
            f"No compatible version found. "
            f"Client: {client_versions}, Server: {server_versions}"
        )
    
    # Sort and return highest version
    sorted_versions = sorted(
        common_versions,
        key=parse_version,
        reverse=True
    )
    
    return sorted_versions[0]
