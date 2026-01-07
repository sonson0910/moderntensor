"""
Standalone tests for the Axon server implementation.

These tests can run independently without full SDK dependencies.
"""

import sys
import os

# Add parent directory to path to import sdk.axon
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

import pytest
import asyncio
from fastapi.testclient import TestClient

from sdk.axon.axon import Axon
from sdk.axon.config import AxonConfig
from sdk.axon.security import SecurityManager


class TestAxonConfig:
    """Test Axon configuration."""
    
    def test_default_config(self):
        """Test default configuration values."""
        config = AxonConfig()
        assert config.host == "0.0.0.0"
        assert config.port == 8091
        assert config.authentication_enabled is True
        assert config.rate_limiting_enabled is True
        print("✓ Default config test passed")
    
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
        print("✓ Custom config test passed")
    
    def test_invalid_port(self):
        """Test that invalid port raises error."""
        try:
            AxonConfig(port=70000)  # Port too high
            assert False, "Should have raised ValueError"
        except ValueError:
            print("✓ Invalid port validation test passed")


class TestSecurityManager:
    """Test SecurityManager functionality."""
    
    def test_initialization(self):
        """Test security manager initialization."""
        manager = SecurityManager()
        assert manager.blacklist_ips == set()
        assert manager.whitelist_ips == set()
        assert not manager.enable_whitelist
        print("✓ Security manager initialization test passed")
    
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
        print("✓ Blacklist test passed")
    
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
        print("✓ Whitelist test passed")
    
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
        print("✓ API key test passed")


class TestAxonServer:
    """Test Axon server functionality."""
    
    def test_initialization(self):
        """Test Axon initialization."""
        config = AxonConfig(uid="test-axon")
        axon = Axon(config=config)
        
        assert axon.config.uid == "test-axon"
        assert axon.security_manager is not None
        assert axon.app is not None
        assert not axon.is_running
        print("✓ Axon initialization test passed")
    
    def test_health_endpoint(self):
        """Test health check endpoint."""
        axon = Axon(config=AxonConfig(uid="test-axon", authentication_enabled=False))
        client = TestClient(axon.app)
        
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "healthy"
        assert "uptime" in data
        assert data["uid"] == "test-axon"
        print("✓ Health endpoint test passed")
    
    def test_metrics_endpoint(self):
        """Test metrics endpoint."""
        axon = Axon(config=AxonConfig(authentication_enabled=False))
        client = TestClient(axon.app)
        
        response = client.get("/metrics")
        assert response.status_code == 200
        data = response.json()
        assert "total_requests" in data
        assert "uptime_seconds" in data
        print("✓ Metrics endpoint test passed")
    
    def test_info_endpoint(self):
        """Test info endpoint."""
        config = AxonConfig(uid="test-axon", port=9000, authentication_enabled=False)
        axon = Axon(config=config)
        client = TestClient(axon.app)
        
        response = client.get("/info")
        assert response.status_code == 200
        data = response.json()
        assert data["uid"] == "test-axon"
        assert data["port"] == 9000
        assert "version" in data
        print("✓ Info endpoint test passed")
    
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
        print("✓ API key management test passed")


def run_tests():
    """Run all tests."""
    print("\n" + "="*60)
    print("Running Axon Server Tests")
    print("="*60 + "\n")
    
    # Test AxonConfig
    print("Testing AxonConfig...")
    config_tests = TestAxonConfig()
    config_tests.test_default_config()
    config_tests.test_custom_config()
    config_tests.test_invalid_port()
    
    # Test SecurityManager
    print("\nTesting SecurityManager...")
    security_tests = TestSecurityManager()
    security_tests.test_initialization()
    security_tests.test_blacklist()
    security_tests.test_whitelist()
    security_tests.test_api_key_generation()
    
    # Test Axon Server
    print("\nTesting Axon Server...")
    axon_tests = TestAxonServer()
    axon_tests.test_initialization()
    axon_tests.test_health_endpoint()
    axon_tests.test_metrics_endpoint()
    axon_tests.test_info_endpoint()
    axon_tests.test_api_key_management()
    
    print("\n" + "="*60)
    print("✅ All tests passed!")
    print("="*60 + "\n")


if __name__ == "__main__":
    run_tests()
