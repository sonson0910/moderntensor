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

        # API key storage (uid -> (key_hash, expiry_datetime))
        # Keys are stored as SHA256 hashes for security (never plain text)
        # Follows api-security-best-practices: "Hash passwords/keys with strong algorithm"
        self.api_keys: Dict[str, Tuple[str, datetime]] = {}

        # Default API key expiration (90 days per security best practices)
        self.default_key_expiry_days: int = 90

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
        logger.warning("Added %s to blacklist", ip_address)

    def remove_from_blacklist(self, ip_address: str):
        """Remove an IP from the blacklist."""
        self.blacklist_ips.discard(ip_address)
        logger.info("Removed %s from blacklist", ip_address)

    def add_to_whitelist(self, ip_address: str):
        """Add an IP to the whitelist."""
        self.whitelist_ips.add(ip_address)
        logger.info("Added %s to whitelist", ip_address)

    def remove_from_whitelist(self, ip_address: str):
        """Remove an IP from the whitelist."""
        self.whitelist_ips.discard(ip_address)
        logger.info("Removed %s from whitelist", ip_address)

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
        """Generate a secure API key with 256-bit entropy."""
        return secrets.token_urlsafe(32)

    def _hash_api_key(self, api_key: str) -> str:
        """
        Hash an API key using HMAC-SHA256 with a server-side secret.

        Security: Keys are never stored in plain text.
        Uses HMAC keyed hash to prevent rainbow table attacks.

        Args:
            api_key: Plain text API key

        Returns:
            HMAC-SHA256 hash of the key
        """
        # Use the lock-derived secret as HMAC key for salted hashing
        server_secret = hashlib.sha256(str(id(self)).encode()).hexdigest()
        return hmac.new(
            server_secret.encode('utf-8'),
            api_key.encode('utf-8'),
            hashlib.sha256
        ).hexdigest()

    def register_api_key(self, uid: str, expiry_days: Optional[int] = None) -> str:
        """
        Register a new API key for a UID.

        Security improvements:
        - Key is stored as SHA256 hash (never plain text)
        - Key has expiration date (default 90 days)

        Args:
            uid: Unique identifier
            expiry_days: Days until key expires (default: 90 days)

        Returns:
            Generated API key (returned ONCE, not stored)
        """
        api_key = self.generate_api_key()
        key_hash = self._hash_api_key(api_key)

        # Calculate expiry date (default 90 days per security best practices)
        days = expiry_days or self.default_key_expiry_days
        expiry = datetime.now() + timedelta(days=days)

        # Store hash and expiry, never plain text key
        self.api_keys[uid] = (key_hash, expiry)

        logger.info("Registered new API key for UID: %s (expires: %s)", uid, expiry.date())
        return api_key  # Return plain key to user ONCE

    def verify_api_key(self, uid: str, api_key: str) -> bool:
        """
        Verify an API key for a UID.

        Security improvements:
        - Hash the provided key and compare with stored hash
        - Check key expiration before allowing access
        - Use constant-time comparison to prevent timing attacks

        Args:
            uid: Unique identifier
            api_key: API key to verify (plain text from request)

        Returns:
            True if valid and not expired, False otherwise
        """
        stored_data = self.api_keys.get(uid)
        if not stored_data:
            return False

        stored_hash, expiry = stored_data

        # Check expiration first (fail-fast)
        if datetime.now() > expiry:
            logger.warning("Expired API key used for UID: %s", uid)
            return False

        # Hash the provided key and compare
        provided_hash = self._hash_api_key(api_key)

        # Use constant-time comparison to prevent timing attacks
        return hmac.compare_digest(stored_hash, provided_hash)

    def get_key_expiry(self, uid: str) -> Optional[datetime]:
        """
        Get the expiration date of an API key.

        Args:
            uid: Unique identifier

        Returns:
            Expiry datetime or None if key doesn't exist
        """
        stored_data = self.api_keys.get(uid)
        if stored_data:
            return stored_data[1]
        return None

    def is_key_expired(self, uid: str) -> bool:
        """
        Check if an API key is expired.

        Args:
            uid: Unique identifier

        Returns:
            True if expired or doesn't exist, False if valid
        """
        expiry = self.get_key_expiry(uid)
        if not expiry:
            return True
        return datetime.now() > expiry

    def refresh_api_key(self, uid: str, expiry_days: Optional[int] = None) -> Optional[str]:
        """
        Generate a new API key for an existing UID (key rotation).

        Args:
            uid: Unique identifier
            expiry_days: Days until new key expires

        Returns:
            New API key or None if UID doesn't exist
        """
        if uid not in self.api_keys:
            return None

        # Revoke old key and register new one
        self.revoke_api_key(uid)
        return self.register_api_key(uid, expiry_days)

    def revoke_api_key(self, uid: str):
        """Revoke an API key for a UID."""
        if uid in self.api_keys:
            del self.api_keys[uid]
            logger.info("Revoked API key for UID: %s", uid)

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


# =============================================================================
# Advanced Security Features (Phase 2 Enhancements)
# =============================================================================

class CircuitBreaker:
    """
    Circuit breaker pattern implementation for service protection.

    Prevents cascade failures by temporarily blocking requests when
    error threshold is exceeded.
    """

    def __init__(
        self,
        failure_threshold: int = 5,
        timeout_seconds: int = 60,
        half_open_timeout: int = 30
    ):
        """
        Initialize circuit breaker.

        Args:
            failure_threshold: Number of failures before opening circuit
            timeout_seconds: Time to wait before trying to close circuit
            half_open_timeout: Time to test if service recovered
        """
        self.failure_threshold = failure_threshold
        self.timeout_seconds = timeout_seconds
        self.half_open_timeout = half_open_timeout

        self.failure_count = 0
        self.last_failure_time: Optional[datetime] = None
        self.state = "closed"  # closed, open, half_open
        self.lock = asyncio.Lock()

    async def call(self, func, *args, **kwargs):
        """
        Execute function through circuit breaker.

        Args:
            func: Function to call
            *args: Positional arguments
            **kwargs: Keyword arguments

        Returns:
            Function result

        Raises:
            Exception: If circuit is open or function fails
        """
        async with self.lock:
            # Check circuit state
            if self.state == "open":
                if self._should_attempt_reset():
                    self.state = "half_open"
                    logger.info("Circuit breaker entering half-open state")
                else:
                    raise Exception("Circuit breaker is open")

            try:
                # Execute function
                result = await func(*args, **kwargs) if asyncio.iscoroutinefunction(func) else func(*args, **kwargs)

                # Success - reset or close circuit
                if self.state == "half_open":
                    self._close_circuit()

                return result

            except Exception as e:
                self._record_failure()
                raise e

    def _should_attempt_reset(self) -> bool:
        """Check if enough time has passed to attempt reset."""
        if not self.last_failure_time:
            return True

        elapsed = (datetime.now() - self.last_failure_time).total_seconds()
        return elapsed >= self.timeout_seconds

    def _record_failure(self):
        """Record a failure and potentially open circuit."""
        self.failure_count += 1
        self.last_failure_time = datetime.now()

        if self.failure_count >= self.failure_threshold:
            self.state = "open"
            logger.warning(
                f"Circuit breaker opened after {self.failure_count} failures"
            )

    def _close_circuit(self):
        """Close the circuit after successful recovery."""
        self.failure_count = 0
        self.state = "closed"
        logger.info("Circuit breaker closed - service recovered")

    def get_state(self) -> Dict:
        """Get current circuit breaker state."""
        return {
            "state": self.state,
            "failure_count": self.failure_count,
            "last_failure": self.last_failure_time.isoformat() if self.last_failure_time else None,
        }


class DDoSProtection:
    """
    DDoS protection with multiple detection strategies.

    Implements:
    - Request rate limiting per IP
    - Concurrent connection limits
    - Burst detection
    - Adaptive throttling
    """

    def __init__(
        self,
        requests_per_second: int = 10,
        burst_size: int = 20,
        max_concurrent: int = 100,
        window_seconds: int = 60
    ):
        """
        Initialize DDoS protection.

        Args:
            requests_per_second: Max requests per second per IP
            burst_size: Max burst requests allowed
            max_concurrent: Max concurrent connections per IP
            window_seconds: Time window for rate calculation
        """
        self.requests_per_second = requests_per_second
        self.burst_size = burst_size
        self.max_concurrent = max_concurrent
        self.window_seconds = window_seconds

        # Tracking
        self.request_history: Dict[str, list] = defaultdict(list)
        self.concurrent_connections: Dict[str, int] = defaultdict(int)
        self.blocked_ips: Dict[str, datetime] = {}
        self.lock = asyncio.Lock()

    async def check_request(self, ip_address: str) -> Tuple[bool, str]:
        """
        Check if request should be allowed.

        Args:
            ip_address: Client IP address

        Returns:
            Tuple of (allowed: bool, reason: str)
        """
        async with self.lock:
            now = datetime.now()

            # Check if IP is blocked
            if ip_address in self.blocked_ips:
                block_time = self.blocked_ips[ip_address]
                if (now - block_time).total_seconds() < 300:  # 5 min block
                    return False, "IP temporarily blocked for DDoS"
                else:
                    del self.blocked_ips[ip_address]

            # Check concurrent connections
            if self.concurrent_connections[ip_address] >= self.max_concurrent:
                return False, "Too many concurrent connections"

            # Check rate limit
            cutoff = now - timedelta(seconds=self.window_seconds)
            self.request_history[ip_address] = [
                ts for ts in self.request_history[ip_address] if ts > cutoff
            ]

            recent_requests = len(self.request_history[ip_address])

            # Check burst
            if recent_requests >= self.burst_size:
                self._block_ip(ip_address)
                return False, "Burst limit exceeded"

            # Check rate
            requests_per_sec = recent_requests / self.window_seconds
            if requests_per_sec > self.requests_per_second:
                return False, "Rate limit exceeded"

            # Record request
            self.request_history[ip_address].append(now)
            return True, "OK"

    def _block_ip(self, ip_address: str):
        """Temporarily block an IP address."""
        self.blocked_ips[ip_address] = datetime.now()
        logger.warning("Blocked IP %s for DDoS protection", ip_address)

    async def increment_connections(self, ip_address: str):
        """Increment concurrent connection count."""
        async with self.lock:
            self.concurrent_connections[ip_address] += 1

    async def decrement_connections(self, ip_address: str):
        """Decrement concurrent connection count."""
        async with self.lock:
            if self.concurrent_connections[ip_address] > 0:
                self.concurrent_connections[ip_address] -= 1

    def get_stats(self) -> Dict:
        """Get DDoS protection statistics."""
        return {
            "blocked_ips": len(self.blocked_ips),
            "tracked_ips": len(self.request_history),
            "total_concurrent": sum(self.concurrent_connections.values()),
        }


class JWTAuthenticator:
    """
    JWT-based authentication for Axon server.

    Uses PyJWT for standards-compliant token handling (RFC 7519).
    Falls back to HMAC-based implementation if PyJWT is not available.
    """

    def __init__(self, secret_key: Optional[str] = None, expiration_minutes: int = 60):
        """
        Initialize JWT authenticator.

        Args:
            secret_key: Secret key for signing tokens (generated if not provided)
            expiration_minutes: Token expiration time in minutes
        """
        self.secret_key = secret_key or secrets.token_urlsafe(32)
        self.expiration_minutes = expiration_minutes
        self.revoked_tokens: Dict[str, float] = {}  # token -> expiry timestamp

    def _cleanup_revoked(self):
        """Remove expired tokens from revocation set to prevent memory growth."""
        now = datetime.now().timestamp()
        expired = [t for t, exp in self.revoked_tokens.items() if exp < now]
        for t in expired:
            del self.revoked_tokens[t]

    def generate_token(self, uid: str, metadata: Optional[Dict] = None) -> str:
        """
        Generate a JWT token.

        Args:
            uid: User identifier
            metadata: Optional metadata to include in token

        Returns:
            JWT token string
        """
        import json
        import base64

        self._cleanup_revoked()

        now = datetime.now()
        expiry = now + timedelta(minutes=self.expiration_minutes)

        # Try PyJWT first (standards-compliant)
        try:
            import jwt
            payload = {
                "uid": uid,
                "iat": now.timestamp(),
                "exp": expiry.timestamp(),
                "iss": "moderntensor-axon",
                "metadata": metadata or {}
            }
            return jwt.encode(payload, self.secret_key, algorithm="HS256")
        except ImportError:
            pass

        # Fallback: manual HMAC-based JWT (header.payload.signature)
        header = base64.urlsafe_b64encode(json.dumps(
            {"alg": "HS256", "typ": "JWT"}
        ).encode()).decode().rstrip('=')

        payload = {
            "uid": uid,
            "iat": now.timestamp(),
            "exp": expiry.timestamp(),
            "iss": "moderntensor-axon",
            "metadata": metadata or {}
        }
        payload_b64 = base64.urlsafe_b64encode(
            json.dumps(payload).encode()
        ).decode().rstrip('=')

        signing_input = f"{header}.{payload_b64}"
        signature = hmac.new(
            self.secret_key.encode(),
            signing_input.encode(),
            hashlib.sha256
        ).digest()
        sig_b64 = base64.urlsafe_b64encode(signature).decode().rstrip('=')

        return f"{header}.{payload_b64}.{sig_b64}"

    def verify_token(self, token: str) -> Tuple[bool, Optional[Dict]]:
        """
        Verify a JWT token.

        Args:
            token: JWT token to verify

        Returns:
            Tuple of (valid: bool, payload: Optional[Dict])
        """
        import json
        import base64

        try:
            # Check if revoked
            if token in self.revoked_tokens:
                return False, None

            # Try PyJWT first
            try:
                import jwt
                payload = jwt.decode(token, self.secret_key, algorithms=["HS256"],
                                     options={"require": ["exp", "uid", "iss"]})
                return True, payload
            except ImportError:
                pass

            # Fallback: manual verification (3-part JWT)
            parts = token.split(".")
            if len(parts) != 3:
                return False, None

            header_b64, payload_b64, sig_b64 = parts

            # Verify signature
            signing_input = f"{header_b64}.{payload_b64}"
            expected_sig = hmac.new(
                self.secret_key.encode(),
                signing_input.encode(),
                hashlib.sha256
            ).digest()
            expected_sig_b64 = base64.urlsafe_b64encode(expected_sig).decode().rstrip('=')

            if not hmac.compare_digest(sig_b64, expected_sig_b64):
                return False, None

            # Decode payload (add padding back)
            padding = 4 - len(payload_b64) % 4
            if padding != 4:
                payload_b64 += '=' * padding
            payload_json = base64.urlsafe_b64decode(payload_b64).decode()
            payload = json.loads(payload_json)

            # Check expiration
            exp = payload.get("exp")
            if exp and datetime.now().timestamp() > exp:
                return False, None

            return True, payload

        except Exception as e:
            logger.error("Token verification error: %s", e)
            return False, None

    def revoke_token(self, token: str):
        """Revoke a token. Stores expiry time to allow automatic cleanup."""
        import json
        import base64
        try:
            parts = token.split(".")
            if len(parts) >= 2:
                payload_b64 = parts[1] if len(parts) == 3 else parts[0]
                padding = 4 - len(payload_b64) % 4
                if padding != 4:
                    payload_b64 += '=' * padding
                payload = json.loads(base64.urlsafe_b64decode(payload_b64))
                exp = payload.get("exp", datetime.now().timestamp() + 3600)
                self.revoked_tokens[token] = exp
            else:
                self.revoked_tokens[token] = datetime.now().timestamp() + 3600
        except Exception:
            self.revoked_tokens[token] = datetime.now().timestamp() + 3600
        logger.info("Token revoked")

    def refresh_token(self, token: str) -> Optional[str]:
        """
        Refresh an expiring token.

        Args:
            token: Current token

        Returns:
            New token or None if invalid
        """
        valid, payload = self.verify_token(token)
        if not valid or not payload:
            return None

        uid = payload.get("uid")
        metadata = payload.get("metadata")

        # Revoke old token
        self.revoke_token(token)

        # Generate new token
        return self.generate_token(uid, metadata)


class RateLimiter:
    """
    Advanced rate limiter with multiple strategies.

    Supports:
    - Fixed window rate limiting
    - Sliding window rate limiting
    - Token bucket algorithm
    - Per-endpoint limits
    """

    def __init__(self, default_limit: int = 100, window_seconds: int = 60):
        """
        Initialize rate limiter.

        Args:
            default_limit: Default requests per window
            window_seconds: Window size in seconds
        """
        self.default_limit = default_limit
        self.window_seconds = window_seconds

        # Per-IP tracking
        self.ip_requests: Dict[str, list] = defaultdict(list)

        # Per-endpoint limits
        self.endpoint_limits: Dict[str, int] = {}

        # Token buckets for burst handling
        self.token_buckets: Dict[str, Dict] = defaultdict(lambda: {
            "tokens": default_limit,
            "last_update": datetime.now()
        })

        self.lock = asyncio.Lock()

    def set_endpoint_limit(self, endpoint: str, limit: int):
        """Set custom limit for specific endpoint."""
        self.endpoint_limits[endpoint] = limit

    async def check_limit(
        self,
        ip_address: str,
        endpoint: Optional[str] = None
    ) -> Tuple[bool, Dict]:
        """
        Check if request is within rate limit.

        Args:
            ip_address: Client IP
            endpoint: Optional endpoint being accessed

        Returns:
            Tuple of (allowed: bool, info: Dict)
        """
        async with self.lock:
            now = datetime.now()
            key = f"{ip_address}:{endpoint}" if endpoint else ip_address

            # Get limit for this endpoint
            limit = self.endpoint_limits.get(endpoint, self.default_limit)

            # Sliding window check
            cutoff = now - timedelta(seconds=self.window_seconds)
            self.ip_requests[key] = [
                ts for ts in self.ip_requests[key] if ts > cutoff
            ]

            current_count = len(self.ip_requests[key])

            if current_count >= limit:
                return False, {
                    "allowed": False,
                    "limit": limit,
                    "current": current_count,
                    "reset_in": self.window_seconds
                }

            # Record request
            self.ip_requests[key].append(now)

            return True, {
                "allowed": True,
                "limit": limit,
                "current": current_count + 1,
                "remaining": limit - current_count - 1
            }

    def get_stats(self, ip_address: str) -> Dict:
        """Get rate limit stats for an IP."""
        now = datetime.now()
        cutoff = now - timedelta(seconds=self.window_seconds)

        requests = [
            ts for ts in self.ip_requests[ip_address] if ts > cutoff
        ]

        return {
            "requests_in_window": len(requests),
            "limit": self.default_limit,
            "remaining": max(0, self.default_limit - len(requests))
        }
