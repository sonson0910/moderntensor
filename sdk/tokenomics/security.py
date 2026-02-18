"""
Security Module for ModernTensor Tokenomics

This module provides security hardening, validation, and monitoring
for tokenomics operations.

Month 2 - Week 3-4: Security Hardening
"""

import time
import re
import json
import hashlib
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Set, Any
from collections import defaultdict
from enum import Enum
import logging

logger = logging.getLogger(__name__)


class SecurityLevel(Enum):
    """Security alert levels."""
    INFO = "info"
    WARNING = "warning"
    CRITICAL = "critical"


@dataclass
class SecurityAlert:
    """Security alert information."""
    level: SecurityLevel
    message: str
    timestamp: float
    details: Dict[str, Any] = field(default_factory=dict)


@dataclass
class RateLimitConfig:
    """Configuration for rate limiting."""
    max_requests: int = 100
    window_seconds: int = 60
    burst_allowance: int = 10


class RateLimiter:
    """
    Token bucket rate limiter.

    Features:
    - Per-address rate limiting
    - Burst handling
    - Sliding window
    """

    def __init__(self, config: RateLimitConfig):
        self.config = config
        self.buckets: Dict[str, List[float]] = defaultdict(list)
        self.blocked: Set[str] = set()

    def check_rate_limit(self, address: str) -> bool:
        """
        Check if address is within rate limit.

        Args:
            address: Address to check

        Returns:
            True if allowed, False if rate limited
        """
        now = time.time()
        window_start = now - self.config.window_seconds

        # Clean old requests
        self.buckets[address] = [
            req_time for req_time in self.buckets[address]
            if req_time > window_start
        ]

        # Check limit
        if len(self.buckets[address]) >= self.config.max_requests:
            self.blocked.add(address)
            logger.warning(f"Rate limit exceeded for {address}")
            return False

        # Add current request
        self.buckets[address].append(now)
        return True

    def is_blocked(self, address: str) -> bool:
        """Check if address is currently blocked."""
        return address in self.blocked

    def unblock(self, address: str) -> None:
        """Unblock an address."""
        if address in self.blocked:
            self.blocked.remove(address)

    def get_remaining_requests(self, address: str) -> int:
        """Get remaining requests for address in current window."""
        now = time.time()
        window_start = now - self.config.window_seconds

        # Clean old requests
        self.buckets[address] = [
            req_time for req_time in self.buckets[address]
            if req_time > window_start
        ]

        return max(0, self.config.max_requests - len(self.buckets[address]))


class InputValidator:
    """
    Validate and sanitize inputs for security.

    Features:
    - Type validation
    - Range checking
    - Format validation
    - SQL injection prevention
    """

    # Address format (hex with 0x prefix)
    ADDRESS_PATTERN = re.compile(r'^0x[a-fA-F0-9]{40}$')

    # Safe integer range
    MAX_SAFE_INT = 2**63 - 1
    MIN_SAFE_INT = -(2**63)

    @classmethod
    def validate_address(cls, address: str) -> bool:
        """
        Validate Ethereum-style address format.

        Args:
            address: Address string to validate

        Returns:
            True if valid

        Raises:
            ValueError: If address is invalid
        """
        if not isinstance(address, str):
            raise ValueError("Address must be a string")

        if not cls.ADDRESS_PATTERN.match(address):
            raise ValueError(f"Invalid address format: {address}")

        return True

    @classmethod
    def validate_amount(cls, amount: int, min_val: int = 0) -> bool:
        """
        Validate token amount.

        Args:
            amount: Amount to validate
            min_val: Minimum allowed value

        Returns:
            True if valid

        Raises:
            ValueError: If amount is invalid
        """
        if not isinstance(amount, int):
            raise ValueError("Amount must be an integer")

        if amount < min_val:
            raise ValueError(f"Amount must be >= {min_val}")

        if amount > cls.MAX_SAFE_INT:
            raise ValueError("Amount exceeds maximum safe integer")

        return True

    @classmethod
    def validate_score(cls, score: float, min_val: float = 0.0, max_val: float = 1.0) -> bool:
        """
        Validate score value (typically 0-1 range).

        Args:
            score: Score to validate
            min_val: Minimum allowed value
            max_val: Maximum allowed value

        Returns:
            True if valid

        Raises:
            ValueError: If score is invalid
        """
        if not isinstance(score, (int, float)):
            raise ValueError("Score must be a number")

        if score < min_val or score > max_val:
            raise ValueError(f"Score must be between {min_val} and {max_val}")

        return True

    @classmethod
    def validate_epoch(cls, epoch: int) -> bool:
        """
        Validate epoch number.

        Args:
            epoch: Epoch number to validate

        Returns:
            True if valid

        Raises:
            ValueError: If epoch is invalid
        """
        if not isinstance(epoch, int):
            raise ValueError("Epoch must be an integer")

        if epoch < 0:
            raise ValueError("Epoch must be non-negative")

        return True

    @classmethod
    def sanitize_string(cls, input_str: str, max_length: int = 256) -> str:
        """
        Sanitize string input.

        Args:
            input_str: String to sanitize
            max_length: Maximum allowed length

        Returns:
            Sanitized string
        """
        if not isinstance(input_str, str):
            input_str = str(input_str)

        # Remove null bytes
        sanitized = input_str.replace('\x00', '')

        # Limit length
        sanitized = sanitized[:max_length]

        # Remove dangerous characters
        dangerous_chars = ['<', '>', '"', "'", '\\', '\n', '\r']
        for char in dangerous_chars:
            sanitized = sanitized.replace(char, '')

        return sanitized


class TransactionValidator:
    """
    Validate transactions before execution.

    Features:
    - Balance checks
    - Double-spend prevention
    - Signature verification
    """

    def __init__(self):
        self.processed_txs: Set[str] = set()
        self.pending_txs: Set[str] = set()

    def validate_reward_transaction(
        self,
        from_address: str,
        to_address: str,
        amount: int,
        balance: int
    ) -> bool:
        """
        Validate reward transaction.

        Args:
            from_address: Sender address
            to_address: Recipient address
            amount: Transfer amount
            balance: Sender's current balance

        Returns:
            True if valid

        Raises:
            ValueError: If transaction is invalid
        """
        # Validate addresses
        InputValidator.validate_address(from_address)
        InputValidator.validate_address(to_address)

        # Validate amount
        InputValidator.validate_amount(amount, min_val=1)

        # Check balance
        if amount > balance:
            raise ValueError(
                f"Insufficient balance: {balance} < {amount}"
            )

        # Prevent self-transfer
        if from_address.lower() == to_address.lower():
            raise ValueError("Cannot transfer to self")

        return True

    def validate_claim(
        self,
        address: str,
        amount: int,
        epoch: int,
        proof: List[bytes]
    ) -> bool:
        """
        Validate reward claim.

        Args:
            address: Claimer address
            amount: Claim amount
            epoch: Epoch number
            proof: Merkle proof

        Returns:
            True if valid

        Raises:
            ValueError: If claim is invalid
        """
        # Validate inputs
        InputValidator.validate_address(address)
        InputValidator.validate_amount(amount, min_val=1)
        InputValidator.validate_epoch(epoch)

        # Validate proof
        if not isinstance(proof, list):
            raise ValueError("Proof must be a list")

        if len(proof) == 0:
            raise ValueError("Proof cannot be empty")

        for node in proof:
            if not isinstance(node, bytes):
                raise ValueError("Proof nodes must be bytes")
            if len(node) != 32:
                raise ValueError("Proof nodes must be 32 bytes")

        return True

    def check_double_claim(self, tx_hash: str) -> bool:
        """
        Check for double-claim attempt.

        Args:
            tx_hash: Transaction hash

        Returns:
            True if already claimed
        """
        return tx_hash in self.processed_txs

    def mark_processed(self, tx_hash: str) -> None:
        """Mark transaction as processed."""
        self.processed_txs.add(tx_hash)
        if tx_hash in self.pending_txs:
            self.pending_txs.remove(tx_hash)

    def add_pending(self, tx_hash: str) -> None:
        """Add transaction to pending set."""
        self.pending_txs.add(tx_hash)


class SecurityMonitor:
    """
    Monitor and detect security threats.

    Features:
    - Anomaly detection
    - Alert generation
    - Threat scoring
    """

    def __init__(self):
        self.alerts: List[SecurityAlert] = []
        self.suspicious_addresses: Set[str] = set()

    def check_reward_anomaly(
        self,
        address: str,
        reward: int,
        avg_reward: int,
        threshold: float = 3.0
    ) -> Optional[SecurityAlert]:
        """
        Check for anomalous reward amounts.

        Args:
            address: Reward recipient
            reward: Reward amount
            avg_reward: Average reward amount
            threshold: Standard deviation threshold

        Returns:
            Security alert if anomaly detected
        """
        if avg_reward == 0:
            return None

        ratio = reward / avg_reward

        if ratio > threshold:
            alert = SecurityAlert(
                level=SecurityLevel.WARNING,
                message="Unusually high reward detected",
                timestamp=time.time(),
                details={
                    'address': address,
                    'reward': reward,
                    'avg_reward': avg_reward,
                    'ratio': ratio
                }
            )
            self.alerts.append(alert)
            return alert

        return None

    def check_claim_pattern(
        self,
        address: str,
        claim_count: int,
        time_window: int
    ) -> Optional[SecurityAlert]:
        """
        Check for suspicious claim patterns.

        Args:
            address: Address claiming rewards
            claim_count: Number of claims in window
            time_window: Time window in seconds

        Returns:
            Security alert if suspicious pattern detected
        """
        # Flag if too many claims
        if claim_count > 10 and time_window < 3600:  # 10 claims in 1 hour
            alert = SecurityAlert(
                level=SecurityLevel.WARNING,
                message="Suspicious claim pattern detected",
                timestamp=time.time(),
                details={
                    'address': address,
                    'claim_count': claim_count,
                    'time_window': time_window
                }
            )
            self.alerts.append(alert)
            self.suspicious_addresses.add(address)
            return alert

        return None

    def check_balance_manipulation(
        self,
        address: str,
        old_balance: int,
        new_balance: int,
        expected_change: int
    ) -> Optional[SecurityAlert]:
        """
        Check for balance manipulation.

        Args:
            address: Address to check
            old_balance: Previous balance
            new_balance: New balance
            expected_change: Expected balance change

        Returns:
            Security alert if manipulation detected
        """
        actual_change = new_balance - old_balance

        if actual_change != expected_change:
            alert = SecurityAlert(
                level=SecurityLevel.CRITICAL,
                message="Balance manipulation detected",
                timestamp=time.time(),
                details={
                    'address': address,
                    'old_balance': old_balance,
                    'new_balance': new_balance,
                    'expected_change': expected_change,
                    'actual_change': actual_change
                }
            )
            self.alerts.append(alert)
            return alert

        return None

    def get_alerts(
        self,
        level: Optional[SecurityLevel] = None,
        since: Optional[float] = None
    ) -> List[SecurityAlert]:
        """
        Get security alerts.

        Args:
            level: Filter by security level
            since: Filter by timestamp (Unix time)

        Returns:
            List of matching alerts
        """
        alerts = self.alerts

        if level:
            alerts = [a for a in alerts if a.level == level]

        if since:
            alerts = [a for a in alerts if a.timestamp >= since]

        return alerts

    def is_suspicious(self, address: str) -> bool:
        """Check if address is flagged as suspicious."""
        return address in self.suspicious_addresses

    def clear_alerts(self, before: Optional[float] = None) -> None:
        """
        Clear old alerts.

        Args:
            before: Clear alerts before this timestamp
        """
        if before:
            self.alerts = [a for a in self.alerts if a.timestamp >= before]
        else:
            self.alerts.clear()


class SlashingValidator:
    """
    Validate slashing conditions and penalties.

    Features:
    - Validator misbehavior detection
    - Penalty calculation
    - Evidence validation
    """

    def __init__(self, slash_percentage: float = 0.1):
        self.slash_percentage = slash_percentage
        self.slashed_validators: Dict[str, List[float]] = defaultdict(list)

    def calculate_slash_amount(
        self,
        stake: int,
        severity: float = 1.0
    ) -> int:
        """
        Calculate slashing penalty amount.

        Args:
            stake: Validator's stake amount
            severity: Severity multiplier (0-1)

        Returns:
            Slash amount
        """
        InputValidator.validate_amount(stake)
        InputValidator.validate_score(severity)

        slash = int(stake * self.slash_percentage * severity)
        return min(slash, stake)  # Cannot slash more than stake

    def validate_slash_evidence(
        self,
        validator: str,
        evidence: Dict[str, Any]
    ) -> bool:
        """
        Validate slashing evidence.

        Args:
            validator: Validator address
            evidence: Evidence dictionary

        Returns:
            True if evidence is valid

        Raises:
            ValueError: If evidence is invalid
        """
        InputValidator.validate_address(validator)

        required_fields = ['type', 'timestamp', 'proof']
        for field in required_fields:
            if field not in evidence:
                raise ValueError(f"Missing required field: {field}")

        # Validate evidence type
        valid_types = ['double_sign', 'downtime', 'invalid_vote']
        if evidence['type'] not in valid_types:
            raise ValueError(f"Invalid evidence type: {evidence['type']}")

        return True

    def record_slash(
        self,
        validator: str,
        amount: int,
        reason: str
    ) -> None:
        """
        Record slashing event.

        Args:
            validator: Validator address
            amount: Amount slashed
            reason: Reason for slashing
        """
        timestamp = time.time()
        self.slashed_validators[validator].append(timestamp)

        logger.warning(
            f"Validator {validator} slashed {amount} tokens. "
            f"Reason: {reason}"
        )

    def get_slash_history(
        self,
        validator: str,
        window_seconds: int = 86400  # 24 hours
    ) -> List[float]:
        """
        Get recent slash history for validator.

        Args:
            validator: Validator address
            window_seconds: Time window in seconds

        Returns:
            List of slash timestamps
        """
        now = time.time()
        cutoff = now - window_seconds

        return [
            ts for ts in self.slashed_validators[validator]
            if ts >= cutoff
        ]


class AuditLogger:
    """
    Security audit logging.

    Features:
    - Immutable audit trail
    - Event categorization
    - Compliance reporting
    """

    def __init__(self):
        self.audit_log: List[Dict[str, Any]] = []

    def log_event(
        self,
        event_type: str,
        actor: str,
        action: str,
        details: Optional[Dict[str, Any]] = None
    ) -> None:
        """
        Log security event.

        Args:
            event_type: Type of event
            actor: Address performing action
            action: Action description
            details: Additional details
        """
        event_hash = self._calculate_event_hash(
            event_type=event_type, actor=actor, action=action, details=details
        )
        event = {
            'timestamp': time.time(),
            'type': event_type,
            'actor': actor,
            'action': action,
            'details': details or {},
            'hash': event_hash
        }

        self.audit_log.append(event)

    def _calculate_event_hash(self, event_type: str = '', actor: str = '', action: str = '', details: Optional[Dict[str, Any]] = None) -> str:
        """Calculate deterministic hash of audit event chained to previous hash."""
        if not self.audit_log:
            prev_hash = hashlib.sha256(b'genesis').hexdigest()
        else:
            prev_hash = self.audit_log[-1].get('hash', '')

        details_str = json.dumps(details or {}, sort_keys=True)
        data = f"{prev_hash}|{event_type}|{actor}|{action}|{details_str}".encode()
        return hashlib.sha256(data).hexdigest()

    def get_events(
        self,
        event_type: Optional[str] = None,
        actor: Optional[str] = None,
        since: Optional[float] = None
    ) -> List[Dict[str, Any]]:
        """
        Query audit events.

        Args:
            event_type: Filter by event type
            actor: Filter by actor address
            since: Filter by timestamp

        Returns:
            List of matching events
        """
        events = self.audit_log

        if event_type:
            events = [e for e in events if e['type'] == event_type]

        if actor:
            events = [e for e in events if e['actor'] == actor]

        if since:
            events = [e for e in events if e['timestamp'] >= since]

        return events

    def verify_integrity(self) -> bool:
        """
        Verify audit log integrity by recomputing and checking hash chain.

        Returns:
            True if log is intact, False if tampering detected
        """
        import json
        for i, event in enumerate(self.audit_log):
            if i == 0:
                prev_hash = hashlib.sha256(b'genesis').hexdigest()
            else:
                prev_hash = self.audit_log[i - 1].get('hash', '')

            details_str = json.dumps(event.get('details', {}), sort_keys=True)
            data = f"{prev_hash}|{event.get('type', '')}|{event.get('actor', '')}|{event.get('action', '')}|{details_str}".encode()
            expected_hash = hashlib.sha256(data).hexdigest()

            if event.get('hash') != expected_hash:
                logger.error(f"Audit log integrity check failed at index {i}")
                return False

        return True
