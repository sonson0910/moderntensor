# sdk/utils/ipfs_client.py
"""
Production-ready IPFS client for ModernTensor Layer 1.

This module provides integration with IPFS for decentralized storage of:
- Weight matrices
- Historical data
- Large state snapshots
"""

import logging
import json
import asyncio
from typing import Optional, Dict, Any
from dataclasses import dataclass

try:
    import aiohttp
except ImportError:
    aiohttp = None

logger = logging.getLogger(__name__)


@dataclass
class IPFSConfig:
    """Configuration for IPFS client."""
    host: str = "127.0.0.1"
    port: int = 5001
    protocol: str = "http"
    timeout: int = 300  # 5 minutes for large uploads
    gateway_url: Optional[str] = None  # For downloads via gateway


class IPFSClient:
    """
    Production-ready IPFS client using HTTP API.
    
    Supports:
    - File upload (add)
    - File download (cat)
    - Pin management
    - Async operations
    """
    
    def __init__(self, config: Optional[IPFSConfig] = None):
        """
        Initialize IPFS client.
        
        Args:
            config: IPFSConfig object, or None to use defaults
        """
        if aiohttp is None:
            raise ImportError(
                "aiohttp not installed. Install with: pip install aiohttp"
            )
        
        self.config = config or IPFSConfig()
        self.base_url = f"{self.config.protocol}://{self.config.host}:{self.config.port}/api/v0"
        self.session: Optional[aiohttp.ClientSession] = None
        logger.info(f"IPFS client initialized: {self.base_url}")
    
    async def __aenter__(self):
        """Async context manager entry."""
        await self.connect()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.close()
    
    async def connect(self):
        """Create aiohttp session."""
        if self.session is None:
            timeout = aiohttp.ClientTimeout(total=self.config.timeout)
            self.session = aiohttp.ClientSession(timeout=timeout)
            logger.debug("IPFS client session created")
    
    async def close(self):
        """Close aiohttp session."""
        if self.session:
            await self.session.close()
            self.session = None
            logger.debug("IPFS client session closed")
    
    async def add(self, data: bytes, metadata: Optional[Dict[str, Any]] = None) -> str:
        """
        Upload data to IPFS.
        
        Args:
            data: Binary data to upload
            metadata: Optional metadata to include (as JSON wrapper)
            
        Returns:
            IPFS CID (Content Identifier)
            
        Raises:
            ConnectionError: If cannot connect to IPFS node
            TimeoutError: If upload times out
        """
        if not self.session:
            await self.connect()
        
        # Wrap data with metadata if provided
        if metadata:
            upload_data = {
                'data': data.hex(),
                'metadata': metadata
            }
            data = json.dumps(upload_data).encode('utf-8')
        
        # Prepare multipart form data
        form = aiohttp.FormData()
        form.add_field('file', data, filename='data')
        
        try:
            async with self.session.post(
                f"{self.base_url}/add",
                data=form
            ) as response:
                if response.status != 200:
                    error_text = await response.text()
                    raise ConnectionError(
                        f"IPFS add failed: {response.status} - {error_text}"
                    )
                
                result = await response.json()
                cid = result['Hash']
                
                logger.info(f"Uploaded to IPFS: {cid} ({len(data)} bytes)")
                return cid
        
        except asyncio.TimeoutError:
            logger.error("IPFS upload timed out")
            raise TimeoutError("IPFS upload timed out")
        
        except aiohttp.ClientError as e:
            logger.error(f"IPFS upload failed: {e}")
            raise ConnectionError(f"IPFS upload failed: {e}")
    
    async def cat(self, cid: str) -> bytes:
        """
        Download data from IPFS.
        
        Args:
            cid: IPFS CID to download
            
        Returns:
            Binary data
            
        Raises:
            ConnectionError: If cannot connect to IPFS node
            FileNotFoundError: If CID not found
        """
        if not self.session:
            await self.connect()
        
        try:
            async with self.session.post(
                f"{self.base_url}/cat",
                params={'arg': cid}
            ) as response:
                if response.status == 404:
                    raise FileNotFoundError(f"IPFS CID not found: {cid}")
                
                if response.status != 200:
                    error_text = await response.text()
                    raise ConnectionError(
                        f"IPFS cat failed: {response.status} - {error_text}"
                    )
                
                data = await response.read()
                logger.info(f"Downloaded from IPFS: {cid} ({len(data)} bytes)")
                return data
        
        except asyncio.TimeoutError:
            logger.error(f"IPFS download timed out: {cid}")
            raise TimeoutError(f"IPFS download timed out")
        
        except aiohttp.ClientError as e:
            logger.error(f"IPFS download failed: {e}")
            raise ConnectionError(f"IPFS download failed: {e}")
    
    async def pin(self, cid: str) -> bool:
        """
        Pin a CID to prevent garbage collection.
        
        Args:
            cid: IPFS CID to pin
            
        Returns:
            True if successful
        """
        if not self.session:
            await self.connect()
        
        try:
            async with self.session.post(
                f"{self.base_url}/pin/add",
                params={'arg': cid}
            ) as response:
                if response.status != 200:
                    logger.warning(f"IPFS pin failed for {cid}: {response.status}")
                    return False
                
                logger.info(f"Pinned IPFS CID: {cid}")
                return True
        
        except Exception as e:
            logger.error(f"IPFS pin failed: {e}")
            return False
    
    async def unpin(self, cid: str) -> bool:
        """
        Unpin a CID to allow garbage collection.
        
        Args:
            cid: IPFS CID to unpin
            
        Returns:
            True if successful
        """
        if not self.session:
            await self.connect()
        
        try:
            async with self.session.post(
                f"{self.base_url}/pin/rm",
                params={'arg': cid}
            ) as response:
                if response.status != 200:
                    logger.warning(f"IPFS unpin failed for {cid}: {response.status}")
                    return False
                
                logger.info(f"Unpinned IPFS CID: {cid}")
                return True
        
        except Exception as e:
            logger.error(f"IPFS unpin failed: {e}")
            return False
    
    async def get_with_metadata(self, cid: str) -> tuple[bytes, Optional[Dict[str, Any]]]:
        """
        Download data with metadata from IPFS.
        
        Args:
            cid: IPFS CID to download
            
        Returns:
            Tuple of (data, metadata)
        """
        raw_data = await self.cat(cid)
        
        # Try to parse as JSON wrapper
        try:
            wrapper = json.loads(raw_data.decode('utf-8'))
            if isinstance(wrapper, dict) and 'data' in wrapper:
                data = bytes.fromhex(wrapper['data'])
                metadata = wrapper.get('metadata')
                return data, metadata
        except (json.JSONDecodeError, ValueError, UnicodeDecodeError):
            pass
        
        # Return as-is if not wrapped
        return raw_data, None
    
    async def is_online(self) -> bool:
        """
        Check if IPFS node is online.
        
        Returns:
            True if online
        """
        if not self.session:
            await self.connect()
        
        try:
            async with self.session.post(f"{self.base_url}/id") as response:
                return response.status == 200
        except Exception:
            return False


# Singleton instance for easy access
_global_ipfs_client: Optional[IPFSClient] = None


def get_ipfs_client(config: Optional[IPFSConfig] = None) -> IPFSClient:
    """
    Get global IPFS client instance.
    
    Args:
        config: Optional config (used only on first call)
        
    Returns:
        IPFSClient instance
    """
    global _global_ipfs_client
    
    if _global_ipfs_client is None:
        _global_ipfs_client = IPFSClient(config)
    
    return _global_ipfs_client
