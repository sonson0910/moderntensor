"""
Security manager for Axon server.

Handles authentication, IP filtering, and security features.
"""

from typing import Set, Dict, Optional, Tuple
from datetime import datetime, timedelta
import hashlib
import hmac
import secrets
from collections import defaultdict
import asyncio
import logging

logger = logging.getLogger(__name__)


class SecurityManager:
    """Manages security features for Axon server."""
    
    def __init__(
        self,
        blacklist_ips: Optional[Set[str]] = None,
        whitelist_ips: Optional[Set[str]] = None,
        enable_whitelist: bool = False,
    ):
        """
        Initialize security manager.
        
        Args:
            blacklist_ips: Set of blacklisted IP addresses
            whitelist_ips: Set of whitelisted IP addresses
            enable_whitelist: If True, only whitelist IPs are allowed
        """
        self.blacklist_ips: Set[str] = blacklist_ips or set()
        self.whitelist_ips: Set[str] = whitelist_ips or set()
        self.enable_whitelist = enable_whitelist
        
        # Rate limiting tracking
        self.rate_limit_tracker: Dict[str, list] = defaultdict(list)
        
        # Connection tracking for DDoS protection
        self.active_connections: Dict[str, int] = defaultdict(int)
        
        # API key storage (uid -> api_key)
        self.api_keys: Dict[str, str] = {}
        
        # Failed authentication attempts
        self.failed_auth_attempts: Dict[str, int] = defaultdict(int)
        
        # Lock for thread-safe operations
        self.lock = asyncio.Lock()
    
    def is_ip_allowed(self, ip_address: str) -> Tuple[bool, str]:
        """
        Check if an IP address is allowed to connect.
        
        Args:
            ip_address: The IP address to check
            
        Returns:
            Tuple of (allowed, reason)
        """
        # Check blacklist first
        if ip_address in self.blacklist_ips:
            return False, "IP is blacklisted"
        
        # Check whitelist if enabled
        if self.enable_whitelist:
            if ip_address not in self.whitelist_ips:
                return False, "IP not in whitelist"
        
        return True, "OK"
    
    def add_to_blacklist(self, ip_address: str):
        """Add an IP to the blacklist."""
        self.blacklist_ips.add(ip_address)
        logger.warning(f"Added {ip_address} to blacklist")
    
    def remove_from_blacklist(self, ip_address: str):
        """Remove an IP from the blacklist."""
        self.blacklist_ips.discard(ip_address)
        logger.info(f"Removed {ip_address} from blacklist")
    
    def add_to_whitelist(self, ip_address: str):
        """Add an IP to the whitelist."""
        self.whitelist_ips.add(ip_address)
        logger.info(f"Added {ip_address} to whitelist")
    
    def remove_from_whitelist(self, ip_address: str):
        """Remove an IP from the whitelist."""
        self.whitelist_ips.discard(ip_address)
        logger.info(f"Removed {ip_address} from whitelist")
    
    async def check_rate_limit(
        self, 
        ip_address: str, 
        max_requests: int = 100, 
        window_seconds: int = 60
    ) -> Tuple[bool, int]:
        """
        Check if an IP has exceeded rate limits.
        
        Args:
            ip_address: The IP address to check
            max_requests: Maximum number of requests allowed in the window
            window_seconds: Time window in seconds
            
        Returns:
            Tuple of (allowed, requests_remaining)
        """
        async with self.lock:
            now = datetime.now()
            cutoff = now - timedelta(seconds=window_seconds)
            
            # Clean up old requests
            self.rate_limit_tracker[ip_address] = [
                ts for ts in self.rate_limit_tracker[ip_address]
                if ts > cutoff
            ]
            
            # Check if limit exceeded
            current_requests = len(self.rate_limit_tracker[ip_address])
            
            if current_requests >= max_requests:
                return False, 0
            
            # Add current request
            self.rate_limit_tracker[ip_address].append(now)
            
            remaining = max_requests - (current_requests + 1)
            return True, remaining
    
    def increment_active_connections(self, ip_address: str) -> int:
        """
        Increment active connection count for an IP.
        
        Args:
            ip_address: The IP address
            
        Returns:
            Current number of active connections
        """
        self.active_connections[ip_address] += 1
        return self.active_connections[ip_address]
    
    def decrement_active_connections(self, ip_address: str):
        """Decrement active connection count for an IP."""
        if ip_address in self.active_connections:
            self.active_connections[ip_address] = max(
                0, self.active_connections[ip_address] - 1
            )
    
    def check_connection_limit(self, ip_address: str, max_connections: int = 10) -> bool:
        """
        Check if an IP has too many active connections.
        
        Args:
            ip_address: The IP address to check
            max_connections: Maximum allowed concurrent connections
            
        Returns:
            True if connection allowed, False otherwise
        """
        return self.active_connections[ip_address] < max_connections
    
    def generate_api_key(self) -> str:
        """Generate a secure API key."""
        return secrets.token_urlsafe(32)
    
    def register_api_key(self, uid: str) -> str:
        """
        Register a new API key for a UID.
        
        Args:
            uid: Unique identifier
            
        Returns:
            Generated API key
        """
        api_key = self.generate_api_key()
        self.api_keys[uid] = api_key
        logger.info(f"Registered new API key for UID: {uid}")
        return api_key
    
    def verify_api_key(self, uid: str, api_key: str) -> bool:
        """
        Verify an API key for a UID.
        
        Args:
            uid: Unique identifier
            api_key: API key to verify
            
        Returns:
            True if valid, False otherwise
        """
        stored_key = self.api_keys.get(uid)
        if not stored_key:
            return False
        
        # Use constant-time comparison to prevent timing attacks
        return hmac.compare_digest(stored_key, api_key)
    
    def revoke_api_key(self, uid: str):
        """Revoke an API key for a UID."""
        if uid in self.api_keys:
            del self.api_keys[uid]
            logger.info(f"Revoked API key for UID: {uid}")
    
    def record_failed_auth(self, ip_address: str) -> int:
        """
        Record a failed authentication attempt.
        
        Args:
            ip_address: IP address that failed authentication
            
        Returns:
            Total number of failed attempts
        """
        self.failed_auth_attempts[ip_address] += 1
        attempts = self.failed_auth_attempts[ip_address]
        
        # Auto-blacklist after threshold
        if attempts >= 5:
            self.add_to_blacklist(ip_address)
            logger.warning(
                f"Auto-blacklisted {ip_address} after {attempts} failed auth attempts"
            )
        
        return attempts
    
    def reset_failed_auth(self, ip_address: str):
        """Reset failed authentication counter for an IP."""
        if ip_address in self.failed_auth_attempts:
            del self.failed_auth_attempts[ip_address]
    
    async def cleanup_old_data(self, max_age_hours: int = 24):
        """
        Clean up old tracking data.
        
        Args:
            max_age_hours: Maximum age of data to keep in hours
        """
        async with self.lock:
            cutoff = datetime.now() - timedelta(hours=max_age_hours)
            
            # Clean rate limit tracker
            for ip in list(self.rate_limit_tracker.keys()):
                self.rate_limit_tracker[ip] = [
                    ts for ts in self.rate_limit_tracker[ip]
                    if ts > cutoff
                ]
                if not self.rate_limit_tracker[ip]:
                    del self.rate_limit_tracker[ip]
            
            logger.info("Cleaned up old security tracking data")
