"""
Alert rules and notification system for ModernTensor.

This module provides alerting capabilities with multiple notification
channels including email, webhooks, and Slack integration.
"""

import logging
import json
import smtplib
from email.mime.text import MIMEText
from email.mime.multipart import MIMEMultipart
from datetime import datetime, timedelta
from typing import Optional, Dict, Any, List, Callable
from enum import Enum
from dataclasses import dataclass, field
from collections import defaultdict
import asyncio

try:
    import aiohttp
    AIOHTTP_AVAILABLE = True
except ImportError:
    AIOHTTP_AVAILABLE = False

logger = logging.getLogger(__name__)


class AlertSeverity(str, Enum):
    """Alert severity levels."""
    CRITICAL = "critical"
    HIGH = "high"
    MEDIUM = "medium"
    LOW = "low"
    INFO = "info"


class AlertStatus(str, Enum):
    """Alert status."""
    FIRING = "firing"
    RESOLVED = "resolved"
    ACKNOWLEDGED = "acknowledged"


@dataclass
class Alert:
    """Alert representation."""
    
    name: str
    severity: AlertSeverity
    status: AlertStatus = AlertStatus.FIRING
    message: str = ""
    labels: Dict[str, str] = field(default_factory=dict)
    annotations: Dict[str, str] = field(default_factory=dict)
    fired_at: datetime = field(default_factory=datetime.now)
    resolved_at: Optional[datetime] = None
    acknowledged_at: Optional[datetime] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert alert to dictionary."""
        return {
            "name": self.name,
            "severity": self.severity.value,
            "status": self.status.value,
            "message": self.message,
            "labels": self.labels,
            "annotations": self.annotations,
            "fired_at": self.fired_at.isoformat(),
            "resolved_at": self.resolved_at.isoformat() if self.resolved_at else None,
            "acknowledged_at": self.acknowledged_at.isoformat() if self.acknowledged_at else None,
        }


class AlertRule:
    """
    Alert rule definition.
    
    Defines conditions and thresholds for triggering alerts.
    """
    
    def __init__(
        self,
        name: str,
        severity: AlertSeverity,
        condition: Callable[[Any], bool],
        message: str,
        labels: Optional[Dict[str, str]] = None,
        annotations: Optional[Dict[str, str]] = None,
        cooldown_seconds: int = 300,  # 5 minutes default
    ):
        """
        Initialize alert rule.
        
        Args:
            name: Rule name
            severity: Alert severity
            condition: Callable that returns True if alert should fire
            message: Alert message
            labels: Additional labels
            annotations: Additional annotations
            cooldown_seconds: Minimum time between alerts
        """
        self.name = name
        self.severity = severity
        self.condition = condition
        self.message = message
        self.labels = labels or {}
        self.annotations = annotations or {}
        self.cooldown_seconds = cooldown_seconds
        self.last_fired: Optional[datetime] = None
    
    def evaluate(self, data: Any) -> Optional[Alert]:
        """
        Evaluate rule against data.
        
        Args:
            data: Data to evaluate
            
        Returns:
            Alert if condition met and not in cooldown, None otherwise
        """
        # Check cooldown
        if self.last_fired:
            elapsed = (datetime.now() - self.last_fired).total_seconds()
            if elapsed < self.cooldown_seconds:
                return None
        
        # Evaluate condition
        try:
            if self.condition(data):
                self.last_fired = datetime.now()
                return Alert(
                    name=self.name,
                    severity=self.severity,
                    message=self.message,
                    labels=self.labels,
                    annotations=self.annotations,
                )
        except Exception as e:
            logger.error(f"Error evaluating alert rule {self.name}: {e}")
        
        return None


class NotificationChannel:
    """Base class for notification channels."""
    
    async def send(self, alert: Alert):
        """
        Send alert notification.
        
        Args:
            alert: Alert to send
        """
        raise NotImplementedError


class EmailNotificationChannel(NotificationChannel):
    """Email notification channel."""
    
    def __init__(
        self,
        smtp_host: str,
        smtp_port: int,
        smtp_user: str,
        smtp_password: str,
        from_email: str,
        to_emails: List[str],
        use_tls: bool = True,
    ):
        """
        Initialize email notification channel.
        
        Args:
            smtp_host: SMTP server host
            smtp_port: SMTP server port
            smtp_user: SMTP username
            smtp_password: SMTP password
            from_email: From email address
            to_emails: List of recipient emails
            use_tls: Use TLS encryption
        """
        self.smtp_host = smtp_host
        self.smtp_port = smtp_port
        self.smtp_user = smtp_user
        self.smtp_password = smtp_password
        self.from_email = from_email
        self.to_emails = to_emails
        self.use_tls = use_tls
    
    async def send(self, alert: Alert):
        """Send alert via email."""
        try:
            # Create message
            msg = MIMEMultipart('alternative')
            msg['Subject'] = f"[{alert.severity.value.upper()}] {alert.name}"
            msg['From'] = self.from_email
            msg['To'] = ', '.join(self.to_emails)
            
            # Email body
            body = f"""
Alert: {alert.name}
Severity: {alert.severity.value}
Status: {alert.status.value}
Time: {alert.fired_at.isoformat()}

Message:
{alert.message}

Labels:
{json.dumps(alert.labels, indent=2)}

Annotations:
{json.dumps(alert.annotations, indent=2)}
            """.strip()
            
            msg.attach(MIMEText(body, 'plain'))
            
            # Send email
            with smtplib.SMTP(self.smtp_host, self.smtp_port) as server:
                if self.use_tls:
                    server.starttls()
                server.login(self.smtp_user, self.smtp_password)
                server.send_message(msg)
            
            logger.info(f"Sent email alert: {alert.name}")
            
        except Exception as e:
            logger.error(f"Failed to send email alert: {e}")


class WebhookNotificationChannel(NotificationChannel):
    """Webhook notification channel."""
    
    def __init__(
        self,
        url: str,
        headers: Optional[Dict[str, str]] = None,
        timeout: int = 10,
    ):
        """
        Initialize webhook notification channel.
        
        Args:
            url: Webhook URL
            headers: Optional HTTP headers
            timeout: Request timeout in seconds
        """
        self.url = url
        self.headers = headers or {}
        self.timeout = timeout
    
    async def send(self, alert: Alert):
        """Send alert via webhook."""
        if not AIOHTTP_AVAILABLE:
            logger.warning("aiohttp not installed, cannot send webhook")
            return
        
        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    self.url,
                    json=alert.to_dict(),
                    headers=self.headers,
                    timeout=aiohttp.ClientTimeout(total=self.timeout),
                ) as response:
                    if response.status >= 400:
                        logger.error(
                            f"Webhook returned status {response.status}: "
                            f"{await response.text()}"
                        )
                    else:
                        logger.info(f"Sent webhook alert: {alert.name}")
        
        except Exception as e:
            logger.error(f"Failed to send webhook alert: {e}")


class SlackNotificationChannel(NotificationChannel):
    """Slack notification channel."""
    
    def __init__(
        self,
        webhook_url: str,
        channel: Optional[str] = None,
        username: str = "ModernTensor Alerts",
    ):
        """
        Initialize Slack notification channel.
        
        Args:
            webhook_url: Slack webhook URL
            channel: Optional channel override
            username: Bot username
        """
        self.webhook_url = webhook_url
        self.channel = channel
        self.username = username
    
    def _severity_color(self, severity: AlertSeverity) -> str:
        """Get color for severity."""
        colors = {
            AlertSeverity.CRITICAL: "#FF0000",  # Red
            AlertSeverity.HIGH: "#FF8C00",      # Orange
            AlertSeverity.MEDIUM: "#FFD700",    # Yellow
            AlertSeverity.LOW: "#00CED1",       # Cyan
            AlertSeverity.INFO: "#808080",      # Gray
        }
        return colors.get(severity, "#808080")
    
    async def send(self, alert: Alert):
        """Send alert to Slack."""
        if not AIOHTTP_AVAILABLE:
            logger.warning("aiohttp not installed, cannot send Slack message")
            return
        
        try:
            # Build Slack message
            payload = {
                "username": self.username,
                "attachments": [{
                    "color": self._severity_color(alert.severity),
                    "title": alert.name,
                    "text": alert.message,
                    "fields": [
                        {
                            "title": "Severity",
                            "value": alert.severity.value.upper(),
                            "short": True,
                        },
                        {
                            "title": "Status",
                            "value": alert.status.value,
                            "short": True,
                        },
                        {
                            "title": "Time",
                            "value": alert.fired_at.strftime("%Y-%m-%d %H:%M:%S UTC"),
                            "short": False,
                        },
                    ],
                    "footer": "ModernTensor Alerting",
                    "ts": int(alert.fired_at.timestamp()),
                }]
            }
            
            if self.channel:
                payload["channel"] = self.channel
            
            # Add labels if present
            if alert.labels:
                payload["attachments"][0]["fields"].append({
                    "title": "Labels",
                    "value": json.dumps(alert.labels, indent=2),
                    "short": False,
                })
            
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    self.webhook_url,
                    json=payload,
                    timeout=aiohttp.ClientTimeout(total=10),
                ) as response:
                    if response.status >= 400:
                        logger.error(
                            f"Slack webhook returned status {response.status}: "
                            f"{await response.text()}"
                        )
                    else:
                        logger.info(f"Sent Slack alert: {alert.name}")
        
        except Exception as e:
            logger.error(f"Failed to send Slack alert: {e}")


class AlertManager:
    """
    Alert manager for ModernTensor.
    
    Manages alert rules, evaluates conditions, and sends notifications
    through configured channels.
    """
    
    def __init__(self):
        """Initialize alert manager."""
        self.rules: List[AlertRule] = []
        self.channels: List[NotificationChannel] = []
        self.active_alerts: Dict[str, Alert] = {}
        self.alert_history: List[Alert] = []
        self.max_history = 1000
    
    def add_rule(self, rule: AlertRule):
        """
        Add alert rule.
        
        Args:
            rule: Alert rule to add
        """
        self.rules.append(rule)
        logger.info(f"Added alert rule: {rule.name}")
    
    def add_channel(self, channel: NotificationChannel):
        """
        Add notification channel.
        
        Args:
            channel: Notification channel to add
        """
        self.channels.append(channel)
        logger.info(f"Added notification channel: {channel.__class__.__name__}")
    
    async def evaluate_rules(self, data: Dict[str, Any]):
        """
        Evaluate all rules against data.
        
        Args:
            data: Data to evaluate rules against
        """
        for rule in self.rules:
            alert = rule.evaluate(data)
            if alert:
                await self._handle_alert(alert)
    
    async def _handle_alert(self, alert: Alert):
        """
        Handle triggered alert.
        
        Args:
            alert: Alert to handle
        """
        # Add to active alerts
        self.active_alerts[alert.name] = alert
        
        # Add to history
        self.alert_history.append(alert)
        if len(self.alert_history) > self.max_history:
            self.alert_history.pop(0)
        
        # Send notifications
        await self._send_notifications(alert)
    
    async def _send_notifications(self, alert: Alert):
        """
        Send alert to all channels.
        
        Args:
            alert: Alert to send
        """
        tasks = [channel.send(alert) for channel in self.channels]
        await asyncio.gather(*tasks, return_exceptions=True)
    
    def resolve_alert(self, name: str):
        """
        Resolve an active alert.
        
        Args:
            name: Alert name
        """
        if name in self.active_alerts:
            alert = self.active_alerts[name]
            alert.status = AlertStatus.RESOLVED
            alert.resolved_at = datetime.now()
            del self.active_alerts[name]
            logger.info(f"Resolved alert: {name}")
    
    def acknowledge_alert(self, name: str):
        """
        Acknowledge an active alert.
        
        Args:
            name: Alert name
        """
        if name in self.active_alerts:
            alert = self.active_alerts[name]
            alert.status = AlertStatus.ACKNOWLEDGED
            alert.acknowledged_at = datetime.now()
            logger.info(f"Acknowledged alert: {name}")
    
    def get_active_alerts(
        self,
        severity: Optional[AlertSeverity] = None
    ) -> List[Alert]:
        """
        Get active alerts.
        
        Args:
            severity: Optional severity filter
            
        Returns:
            List of active alerts
        """
        alerts = list(self.active_alerts.values())
        
        if severity:
            alerts = [a for a in alerts if a.severity == severity]
        
        return alerts
    
    def get_alert_stats(self) -> Dict[str, Any]:
        """
        Get alert statistics.
        
        Returns:
            Alert statistics
        """
        severity_counts = defaultdict(int)
        for alert in self.active_alerts.values():
            severity_counts[alert.severity.value] += 1
        
        return {
            "active_alerts": len(self.active_alerts),
            "total_rules": len(self.rules),
            "notification_channels": len(self.channels),
            "alerts_by_severity": dict(severity_counts),
            "alert_history_size": len(self.alert_history),
        }


# Pre-defined alert rules for common scenarios

def create_blockchain_alert_rules() -> List[AlertRule]:
    """
    Create common blockchain alert rules.
    
    Returns:
        List of alert rules
    """
    return [
        AlertRule(
            name="HighBlockProductionTime",
            severity=AlertSeverity.HIGH,
            condition=lambda data: data.get("block_time", 0) > 30,
            message="Block production time exceeds 30 seconds",
            labels={"category": "blockchain"},
        ),
        AlertRule(
            name="LowValidatorCount",
            severity=AlertSeverity.CRITICAL,
            condition=lambda data: data.get("validator_count", 0) < 3,
            message="Validator count dropped below minimum threshold",
            labels={"category": "blockchain"},
        ),
        AlertRule(
            name="HighTransactionFailureRate",
            severity=AlertSeverity.MEDIUM,
            condition=lambda data: data.get("tx_failure_rate", 0) > 0.1,
            message="Transaction failure rate exceeds 10%",
            labels={"category": "blockchain"},
        ),
    ]


def create_network_alert_rules() -> List[AlertRule]:
    """
    Create common network alert rules.
    
    Returns:
        List of alert rules
    """
    return [
        AlertRule(
            name="LowPeerCount",
            severity=AlertSeverity.HIGH,
            condition=lambda data: data.get("peer_count", 0) < 5,
            message="Peer count dropped below 5",
            labels={"category": "network"},
        ),
        AlertRule(
            name="HighNetworkLatency",
            severity=AlertSeverity.MEDIUM,
            condition=lambda data: data.get("avg_latency_ms", 0) > 1000,
            message="Average network latency exceeds 1000ms",
            labels={"category": "network"},
        ),
    ]


def create_security_alert_rules() -> List[AlertRule]:
    """
    Create common security alert rules.
    
    Returns:
        List of alert rules
    """
    return [
        AlertRule(
            name="HighFailedAuthAttempts",
            severity=AlertSeverity.CRITICAL,
            condition=lambda data: data.get("failed_auth_attempts", 0) > 10,
            message="High number of failed authentication attempts detected",
            labels={"category": "security"},
        ),
        AlertRule(
            name="DDoSAttackDetected",
            severity=AlertSeverity.CRITICAL,
            condition=lambda data: data.get("requests_per_second", 0) > 1000,
            message="Possible DDoS attack detected",
            labels={"category": "security"},
        ),
    ]


# Global alert manager instance
global_alert_manager = AlertManager()


def get_alert_manager() -> AlertManager:
    """
    Get the global alert manager.
    
    Returns:
        Global AlertManager instance
    """
    return global_alert_manager
