"""
Tests for the Axon server implementation.

Tests cover configuration, security, middleware, and core functionality.
"""

import pytest
import asyncio
from fastapi import Request
from fastapi.testclient import TestClient

from sdk.axon import Axon, AxonConfig
from sdk.axon.security import SecurityManager
from sdk.axon.middleware import (
    AuthenticationMiddleware,
    RateLimitMiddleware,
    BlacklistMiddleware,
)


class TestAxonConfig:
    """Test Axon configuration."""
    
    def test_default_config(self):
        """Test default configuration values."""
        config = AxonConfig()
        assert config.host == "0.0.0.0"
        assert config.port == 8091
        assert config.authentication_enabled is True
        assert config.rate_limiting_enabled is True
    
    def test_custom_config(self):
        """Test custom configuration."""
        config = AxonConfig(
            host="127.0.0.1",
            port=9000,
            uid="test-axon",
            rate_limit_requests=50,
        )
        assert config.host == "127.0.0.1"
        assert config.port == 9000
        assert config.uid == "test-axon"
        assert config.rate_limit_requests == 50
    
    def test_invalid_port(self):
        """Test that invalid port raises error."""
        with pytest.raises(ValueError):
            AxonConfig(port=70000)  # Port too high
    
    def test_ssl_validation(self):
        """Test SSL configuration validation."""
        with pytest.raises(ValueError):
            # SSL enabled but no cert files
            AxonConfig(ssl_enabled=True)


class TestSecurityManager:
    """Test SecurityManager functionality."""
    
    def test_initialization(self):
        """Test security manager initialization."""
        manager = SecurityManager()
        assert manager.blacklist_ips == set()
        assert manager.whitelist_ips == set()
        assert not manager.enable_whitelist
    
    def test_blacklist(self):
        """Test IP blacklisting."""
        manager = SecurityManager()
        
        # Add to blacklist
        manager.add_to_blacklist("192.168.1.100")
        allowed, reason = manager.is_ip_allowed("192.168.1.100")
        assert not allowed
        assert "blacklisted" in reason
        
        # Remove from blacklist
        manager.remove_from_blacklist("192.168.1.100")
        allowed, reason = manager.is_ip_allowed("192.168.1.100")
        assert allowed
    
    def test_whitelist(self):
        """Test IP whitelisting."""
        manager = SecurityManager(
            whitelist_ips={"192.168.1.100"},
            enable_whitelist=True,
        )
        
        # Whitelisted IP should be allowed
        allowed, reason = manager.is_ip_allowed("192.168.1.100")
        assert allowed
        
        # Non-whitelisted IP should be blocked
        allowed, reason = manager.is_ip_allowed("192.168.1.200")
        assert not allowed
        assert "whitelist" in reason
    
    @pytest.mark.asyncio
    async def test_rate_limiting(self):
        """Test rate limiting functionality."""
        manager = SecurityManager()
        
        # Should allow first requests
        for i in range(5):
            allowed, remaining = await manager.check_rate_limit(
                "192.168.1.100", max_requests=10, window_seconds=60
            )
            assert allowed
            assert remaining == 10 - (i + 1) - 1
        
        # Should block after limit
        for _ in range(6):
            await manager.check_rate_limit(
                "192.168.1.100", max_requests=10, window_seconds=60
            )
        
        allowed, remaining = await manager.check_rate_limit(
            "192.168.1.100", max_requests=10, window_seconds=60
        )
        assert not allowed
        assert remaining == 0
    
    def test_connection_tracking(self):
        """Test connection tracking."""
        manager = SecurityManager()
        
        # Increment connections
        count = manager.increment_active_connections("192.168.1.100")
        assert count == 1
        
        count = manager.increment_active_connections("192.168.1.100")
        assert count == 2
        
        # Check limit
        assert manager.check_connection_limit("192.168.1.100", max_connections=3)
        assert not manager.check_connection_limit("192.168.1.100", max_connections=2)
        
        # Decrement connections
        manager.decrement_active_connections("192.168.1.100")
        assert manager.active_connections["192.168.1.100"] == 1
    
    def test_api_key_generation(self):
        """Test API key generation and verification."""
        manager = SecurityManager()
        
        # Register API key
        api_key = manager.register_api_key("test-uid")
        assert api_key is not None
        assert len(api_key) > 20  # Should be reasonably long
        
        # Verify valid key
        assert manager.verify_api_key("test-uid", api_key)
        
        # Verify invalid key
        assert not manager.verify_api_key("test-uid", "invalid-key")
        assert not manager.verify_api_key("wrong-uid", api_key)
        
        # Revoke key
        manager.revoke_api_key("test-uid")
        assert not manager.verify_api_key("test-uid", api_key)
    
    def test_failed_auth_tracking(self):
        """Test failed authentication tracking and auto-blacklist."""
        manager = SecurityManager()
        ip = "192.168.1.100"
        
        # Record failed attempts
        for i in range(4):
            attempts = manager.record_failed_auth(ip)
            assert attempts == i + 1
            # Should not be blacklisted yet
            allowed, _ = manager.is_ip_allowed(ip)
            assert allowed
        
        # 5th attempt should trigger auto-blacklist
        attempts = manager.record_failed_auth(ip)
        assert attempts == 5
        allowed, _ = manager.is_ip_allowed(ip)
        assert not allowed  # Now blacklisted


class TestAxon:
    """Test Axon server functionality."""
    
    def test_initialization(self):
        """Test Axon initialization."""
        config = AxonConfig(uid="test-axon")
        axon = Axon(config=config)
        
        assert axon.config.uid == "test-axon"
        assert axon.security_manager is not None
        assert axon.app is not None
        assert not axon.is_running
    
    def test_attach_handler(self):
        """Test attaching request handlers."""
        axon = Axon()
        
        async def test_handler(request: Request):
            return {"status": "ok"}
        
        # Attach handler
        result = axon.attach("/test", test_handler, methods=["POST"])
        assert result is axon  # Should return self for chaining
        assert "/test" in axon.handlers
    
    def test_api_key_management(self):
        """Test API key registration and revocation."""
        axon = Axon()
        
        # Register key
        api_key = axon.register_api_key("test-uid")
        assert api_key is not None
        
        # Verify it works
        assert axon.security_manager.verify_api_key("test-uid", api_key)
        
        # Revoke key
        axon.revoke_api_key("test-uid")
        assert not axon.security_manager.verify_api_key("test-uid", api_key)
    
    def test_ip_management(self):
        """Test IP blacklist/whitelist management."""
        axon = Axon()
        
        # Blacklist IP
        axon.blacklist_ip("192.168.1.100")
        allowed, _ = axon.security_manager.is_ip_allowed("192.168.1.100")
        assert not allowed
        
        # Whitelist IP
        axon.whitelist_ip("192.168.1.200")
        assert "192.168.1.200" in axon.security_manager.whitelist_ips
    
    def test_health_endpoint(self):
        """Test health check endpoint."""
        axon = Axon(config=AxonConfig(uid="test-axon"))
        client = TestClient(axon.app)
        
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "healthy"
        assert "uptime" in data
        assert data["uid"] == "test-axon"
    
    def test_metrics_endpoint(self):
        """Test metrics endpoint."""
        axon = Axon()
        client = TestClient(axon.app)
        
        response = client.get("/metrics")
        assert response.status_code == 200
        data = response.json()
        assert "total_requests" in data
        assert "uptime_seconds" in data
    
    def test_info_endpoint(self):
        """Test info endpoint."""
        config = AxonConfig(uid="test-axon", port=9000)
        axon = Axon(config=config)
        client = TestClient(axon.app)
        
        response = client.get("/info")
        assert response.status_code == 200
        data = response.json()
        assert data["uid"] == "test-axon"
        assert data["port"] == 9000
        assert "version" in data
    
    def test_custom_endpoint(self):
        """Test custom endpoint with handler."""
        axon = Axon(config=AxonConfig(authentication_enabled=False))
        
        async def custom_handler(request: Request):
            data = await request.json()
            return {"received": data.get("message")}
        
        axon.attach("/custom", custom_handler, methods=["POST"])
        client = TestClient(axon.app)
        
        response = client.post("/custom", json={"message": "hello"})
        assert response.status_code == 200
        data = response.json()
        assert data["received"] == "hello"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
