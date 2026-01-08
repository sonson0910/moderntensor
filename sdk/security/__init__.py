"""
Security audit and validation tools for ModernTensor Layer 1 blockchain.

This module provides comprehensive security auditing capabilities including:
- Cryptographic validation
- Consensus security checks
- Network security analysis
- Smart contract vulnerability scanning
- Role-Based Access Control (RBAC)
"""

from .types import SecurityIssue, Severity, AuditReport
from .audit import SecurityAuditor
from .crypto_audit import CryptoAuditor
from .consensus_audit import ConsensusAuditor
from .network_audit import NetworkAuditor
from .contract_audit import ContractAuditor
from .rbac import (
    Permission,
    Role,
    User,
    RoleManager,
    AccessControl,
    get_access_control,
)

__all__ = [
    # Audit
    'SecurityIssue',
    'Severity',
    'AuditReport',
    'SecurityAuditor',
    'CryptoAuditor',
    'ConsensusAuditor',
    'NetworkAuditor',
    'ContractAuditor',
    # RBAC
    'Permission',
    'Role',
    'User',
    'RoleManager',
    'AccessControl',
    'get_access_control',
]
