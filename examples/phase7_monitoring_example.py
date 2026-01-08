"""
Example usage of Phase 7 monitoring and security features.

This script demonstrates how to use:
- OpenTelemetry distributed tracing
- Structured logging
- Alert rules and notifications
- Role-Based Access Control (RBAC)
- Prometheus metrics
"""

import asyncio
import logging
from datetime import datetime

# Import monitoring components
from sdk.monitoring import (
    # Tracing
    configure_tracing,
    TracingConfig,
    get_tracer,
    trace_blockchain_operation,
    trace_network_operation,
    # Logging
    get_logger,
    LoggerFactory,
    # Alerts
    AlertManager,
    AlertRule,
    AlertSeverity,
    SlackNotificationChannel,
    WebhookNotificationChannel,
    create_blockchain_alert_rules,
    create_network_alert_rules,
    create_security_alert_rules,
    # Metrics
    MetricsCollector,
    blockchain_metrics,
)

# Import security components
from sdk.security import (
    # RBAC
    AccessControl,
    Permission,
    Role,
    get_access_control,
)


# =============================================================================
# 1. Configure Distributed Tracing
# =============================================================================

def setup_tracing():
    """Configure OpenTelemetry distributed tracing."""
    print("\n=== Setting up Distributed Tracing ===")
    
    # Configure tracing
    config = TracingConfig(
        service_name="moderntensor-example",
        service_version="1.0.0",
        environment="development",
        otlp_endpoint="localhost:4317",  # OTLP collector endpoint
        console_export=True,  # Enable console output for demo
    )
    
    configure_tracing(config)
    
    tracer = get_tracer()
    print(f"✅ Tracing configured: {tracer.enabled}")
    
    return tracer


# =============================================================================
# 2. Configure Structured Logging
# =============================================================================

def setup_logging():
    """Configure structured logging."""
    print("\n=== Setting up Structured Logging ===")
    
    # Configure global logger settings
    LoggerFactory.configure(
        service_name="moderntensor-example",
        environment="development",
        level=logging.INFO,
        json_format=True,
    )
    
    # Get logger
    logger = get_logger("example")
    
    # Add context
    logger.add_context(
        node_id="node-001",
        region="us-west-2",
    )
    
    print("✅ Structured logging configured")
    
    return logger


# =============================================================================
# 3. Configure Alert System
# =============================================================================

async def setup_alerts():
    """Configure alert rules and notifications."""
    print("\n=== Setting up Alert System ===")
    
    # Create alert manager
    alert_manager = AlertManager()
    
    # Add notification channels
    
    # Slack notification (replace with your webhook URL)
    slack_channel = SlackNotificationChannel(
        webhook_url="https://hooks.slack.com/services/YOUR/WEBHOOK/URL",
        channel="#alerts",
        username="ModernTensor Alerts"
    )
    alert_manager.add_channel(slack_channel)
    
    # Webhook notification
    webhook_channel = WebhookNotificationChannel(
        url="http://localhost:8080/webhooks/alerts",
        headers={"Authorization": "Bearer YOUR_TOKEN"}
    )
    alert_manager.add_channel(webhook_channel)
    
    # Add pre-defined alert rules
    for rule in create_blockchain_alert_rules():
        alert_manager.add_rule(rule)
    
    for rule in create_network_alert_rules():
        alert_manager.add_rule(rule)
    
    for rule in create_security_alert_rules():
        alert_manager.add_rule(rule)
    
    # Add custom alert rule
    custom_rule = AlertRule(
        name="HighMemoryUsage",
        severity=AlertSeverity.MEDIUM,
        condition=lambda data: data.get("memory_usage_percent", 0) > 80,
        message="Memory usage exceeds 80%",
        labels={"category": "system"},
        cooldown_seconds=300,
    )
    alert_manager.add_rule(custom_rule)
    
    print(f"✅ Alert manager configured with {len(alert_manager.rules)} rules")
    
    return alert_manager


# =============================================================================
# 4. Configure RBAC
# =============================================================================

def setup_rbac():
    """Configure Role-Based Access Control."""
    print("\n=== Setting up RBAC ===")
    
    # Get access control instance
    ac = get_access_control()
    
    # Create users
    admin_user = ac.create_user(
        uid="admin-001",
        roles=[Role.ADMIN],
        metadata={"name": "Admin User", "email": "admin@example.com"}
    )
    
    validator_user = ac.create_user(
        uid="validator-001",
        roles=[Role.VALIDATOR],
        metadata={"name": "Validator Node", "stake": "1000"}
    )
    
    miner_user = ac.create_user(
        uid="miner-001",
        roles=[Role.MINER],
        metadata={"name": "Miner Node", "hash_rate": "100"}
    )
    
    observer_user = ac.create_user(
        uid="observer-001",
        roles=[Role.OBSERVER],
        metadata={"name": "Observer", "purpose": "monitoring"}
    )
    
    # Grant custom permission
    ac.grant_permission("observer-001", Permission.VIEW_LOGS)
    
    print(f"✅ RBAC configured with {len(ac.list_users())} users")
    
    # Display permissions
    for user in ac.list_users():
        perms = ac.get_user_permissions(user.uid)
        print(f"  - {user.uid}: {len(perms)} permissions")
    
    return ac


# =============================================================================
# 5. Example Operations with Tracing and Logging
# =============================================================================

@trace_blockchain_operation("process_block")
async def process_block(logger, block_number: int):
    """Example blockchain operation with tracing."""
    logger.info(
        f"Processing block {block_number}",
        block_number=block_number,
        operation="process_block"
    )
    
    # Simulate block processing
    await asyncio.sleep(0.1)
    
    # Log blockchain event
    logger.log_blockchain_event(
        event_type="block_processed",
        block_number=block_number,
        transactions=10,
        gas_used=250000,
    )
    
    # Update metrics
    blockchain_metrics.update_blockchain_metrics(
        height=block_number,
        transactions_count=10,
        block_size=1024 * 512,  # 512 KB
        accounts_count=1000,
    )


@trace_network_operation("sync_peer")
async def sync_with_peer(logger, peer_id: str):
    """Example network operation with tracing."""
    logger.info(
        f"Syncing with peer {peer_id}",
        peer_id=peer_id,
        operation="sync_peer"
    )
    
    # Simulate peer sync
    await asyncio.sleep(0.05)
    
    # Log network event
    logger.log_network_event(
        event_type="peer_synced",
        peer_id=peer_id,
        blocks_synced=100,
    )


def check_permission_example(ac: AccessControl, logger):
    """Example permission checking."""
    logger.info("Checking user permissions")
    
    # Check admin permissions
    if ac.has_permission("admin-001", Permission.ADMIN_FULL):
        logger.info("Admin has full permissions", user="admin-001")
    
    # Check validator permissions
    if ac.has_permission("validator-001", Permission.VALIDATE_BLOCKS):
        logger.info("Validator can validate blocks", user="validator-001")
    
    # Check observer permissions (should fail for write operations)
    if not ac.has_permission("observer-001", Permission.WRITE_BLOCKCHAIN):
        logger.warning(
            "Observer cannot write to blockchain",
            user="observer-001",
            permission="write_blockchain"
        )


# =============================================================================
# 6. Alert Evaluation Example
# =============================================================================

async def trigger_alerts(alert_manager: AlertManager, logger):
    """Example of triggering alerts."""
    logger.info("Evaluating alert conditions")
    
    # Simulate various scenarios
    
    # 1. Normal blockchain operation
    await alert_manager.evaluate_rules({
        "block_time": 5.0,
        "validator_count": 10,
        "tx_failure_rate": 0.01,
        "peer_count": 20,
        "avg_latency_ms": 100,
        "failed_auth_attempts": 2,
        "requests_per_second": 100,
        "memory_usage_percent": 60,
    })
    
    # 2. Trigger high block time alert
    logger.warning("Simulating high block production time")
    await alert_manager.evaluate_rules({
        "block_time": 35.0,  # > 30 seconds threshold
        "validator_count": 10,
    })
    
    # 3. Trigger low validator count alert
    logger.warning("Simulating low validator count")
    await alert_manager.evaluate_rules({
        "block_time": 5.0,
        "validator_count": 2,  # < 3 threshold
    })
    
    # 4. Trigger security alert
    logger.error("Simulating security threat")
    await alert_manager.evaluate_rules({
        "failed_auth_attempts": 15,  # > 10 threshold
        "requests_per_second": 1500,  # > 1000 threshold (DDoS)
    })
    
    # Display active alerts
    active_alerts = alert_manager.get_active_alerts()
    logger.info(f"Active alerts: {len(active_alerts)}")
    
    for alert in active_alerts:
        logger.log_security_event(
            event_type="alert_fired",
            severity=alert.severity.value,
            alert_name=alert.name,
            alert_message=alert.message,
        )


# =============================================================================
# Main Example
# =============================================================================

async def main():
    """Run Phase 7 features example."""
    print("=" * 80)
    print("ModernTensor Phase 7 - Monitoring & Security Example")
    print("=" * 80)
    
    # Setup all components
    tracer = setup_tracing()
    logger = setup_logging()
    alert_manager = await setup_alerts()
    ac = setup_rbac()
    
    print("\n" + "=" * 80)
    print("Running Example Operations")
    print("=" * 80)
    
    # Example 1: Process some blocks
    logger.info("Starting blockchain operations")
    for i in range(1, 6):
        await process_block(logger, block_number=1000 + i)
    
    # Example 2: Sync with peers
    logger.info("Starting network operations")
    for peer in ["peer-001", "peer-002", "peer-003"]:
        await sync_with_peer(logger, peer_id=peer)
    
    # Example 3: Check permissions
    check_permission_example(ac, logger)
    
    # Example 4: Trigger and evaluate alerts
    await trigger_alerts(alert_manager, logger)
    
    # Example 5: Log various event types
    logger.log_performance_metric(
        metric_name="block_processing_time",
        value=5.2,
        unit="ms",
        block_number=1005,
    )
    
    logger.log_api_request(
        method="POST",
        path="/api/v1/transactions",
        status_code=200,
        duration_ms=45.3,
        ip_address="192.168.1.100",
        user_agent="ModernTensor-Client/1.0",
    )
    
    # Display final stats
    print("\n" + "=" * 80)
    print("Final Statistics")
    print("=" * 80)
    
    alert_stats = alert_manager.get_alert_stats()
    print(f"\n📊 Alert Statistics:")
    print(f"  - Active Alerts: {alert_stats['active_alerts']}")
    print(f"  - Total Rules: {alert_stats['total_rules']}")
    print(f"  - Notification Channels: {alert_stats['notification_channels']}")
    print(f"  - Alerts by Severity: {alert_stats['alerts_by_severity']}")
    
    rbac_stats = ac.get_stats()
    print(f"\n🔐 RBAC Statistics:")
    print(f"  - Total Users: {rbac_stats['total_users']}")
    print(f"  - Total Roles: {rbac_stats['total_roles']}")
    print(f"  - Users by Role: {rbac_stats['users_by_role']}")
    
    print("\n✅ Example completed successfully!")
    print("=" * 80)


if __name__ == "__main__":
    asyncio.run(main())
