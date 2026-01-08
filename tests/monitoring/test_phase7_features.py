"""
Tests for Phase 7 monitoring and security features.

Tests:
- OpenTelemetry distributed tracing
- Structured logging
- Alert rules and notifications
- Role-Based Access Control (RBAC)
"""

import pytest
import asyncio
import json
from datetime import datetime, timedelta

from sdk.monitoring.tracing import (
    DistributedTracer,
    TracingConfig,
)
from sdk.monitoring.logging import (
    StructuredLogger,
    JSONFormatter,
    LoggerFactory,
)
from sdk.monitoring.alerts import (
    Alert,
    AlertRule,
    AlertSeverity,
    AlertStatus,
    AlertManager,
)
from sdk.security.rbac import (
    Permission,
    Role,
    User,
    RoleManager,
    AccessControl,
)


# =============================================================================
# Tracing Tests
# =============================================================================

class TestDistributedTracer:
    """Test OpenTelemetry distributed tracing."""
    
    def test_tracer_initialization(self):
        """Test tracer initialization."""
        config = TracingConfig(
            service_name="test-service",
            service_version="1.0.0",
            environment="test",
        )
        
        tracer = DistributedTracer(config)
        assert tracer.config.service_name == "test-service"
        assert tracer.config.service_version == "1.0.0"


# =============================================================================
# Logging Tests
# =============================================================================

class TestStructuredLogger:
    """Test structured logging."""
    
    def test_logger_initialization(self):
        """Test logger initialization."""
        logger = StructuredLogger(
            name="test",
            service_name="test-service",
            json_format=True,
        )
        
        assert logger.name == "test"
        assert logger.service_name == "test-service"
        assert logger.json_format is True


# =============================================================================
# Alert Tests
# =============================================================================

class TestAlertRule:
    """Test alert rules."""
    
    def test_rule_creation(self):
        """Test rule creation."""
        rule = AlertRule(
            name="TestRule",
            severity=AlertSeverity.HIGH,
            condition=lambda data: data.get("value", 0) > 10,
            message="Test alert",
        )
        
        assert rule.name == "TestRule"
        assert rule.severity == AlertSeverity.HIGH


# =============================================================================
# RBAC Tests
# =============================================================================

class TestRoleManager:
    """Test role manager."""
    
    def test_initialization(self):
        """Test initialization with default roles."""
        manager = RoleManager()
        
        roles = manager.list_roles()
        assert Role.ADMIN in roles
        assert Role.VALIDATOR in roles
        assert Role.MINER in roles
        assert Role.OBSERVER in roles


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
