"""
Monitoring and metrics module for ModernTensor blockchain.

Provides comprehensive monitoring capabilities including:
- Prometheus metrics collection
- OpenTelemetry distributed tracing
- Structured logging
- Alert rules and notifications
"""

from .metrics import (
    MetricsCollector,
    blockchain_metrics,
    network_metrics,
    consensus_metrics,
)
from .tracing import (
    DistributedTracer,
    TracingConfig,
    get_tracer,
    configure_tracing,
    trace_blockchain_operation,
    trace_network_operation,
    trace_axon_request,
    trace_dendrite_query,
)
from .logging import (
    StructuredLogger,
    JSONFormatter,
    LoggerFactory,
    get_logger,
)
from .alerts import (
    Alert,
    AlertRule,
    AlertSeverity,
    AlertStatus,
    AlertManager,
    NotificationChannel,
    EmailNotificationChannel,
    WebhookNotificationChannel,
    SlackNotificationChannel,
    get_alert_manager,
    create_blockchain_alert_rules,
    create_network_alert_rules,
    create_security_alert_rules,
)

__all__ = [
    # Metrics
    "MetricsCollector",
    "blockchain_metrics",
    "network_metrics",
    "consensus_metrics",
    # Tracing
    "DistributedTracer",
    "TracingConfig",
    "get_tracer",
    "configure_tracing",
    "trace_blockchain_operation",
    "trace_network_operation",
    "trace_axon_request",
    "trace_dendrite_query",
    # Logging
    "StructuredLogger",
    "JSONFormatter",
    "LoggerFactory",
    "get_logger",
    # Alerts
    "Alert",
    "AlertRule",
    "AlertSeverity",
    "AlertStatus",
    "AlertManager",
    "NotificationChannel",
    "EmailNotificationChannel",
    "WebhookNotificationChannel",
    "SlackNotificationChannel",
    "get_alert_manager",
    "create_blockchain_alert_rules",
    "create_network_alert_rules",
    "create_security_alert_rules",
]
