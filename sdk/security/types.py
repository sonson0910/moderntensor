"""Common types and classes for security audit module."""
from typing import Dict, List, Any, Optional
from dataclasses import dataclass, field
from enum import Enum


class Severity(Enum):
    """Security issue severity levels."""
    CRITICAL = "critical"
    HIGH = "high"
    MEDIUM = "medium"
    LOW = "low"
    INFO = "info"


@dataclass
class SecurityIssue:
    """Represents a security issue found during audit."""
    severity: Severity
    category: str
    title: str
    description: str
    location: str
    recommendation: str
    cwe_id: Optional[str] = None  # Common Weakness Enumeration ID
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for reporting."""
        return {
            'severity': self.severity.value,
            'category': self.category,
            'title': self.title,
            'description': self.description,
            'location': self.location,
            'recommendation': self.recommendation,
            'cwe_id': self.cwe_id,
        }


@dataclass
class AuditReport:
    """Comprehensive security audit report."""
    timestamp: float
    duration: float
    total_checks: int
    issues: List[SecurityIssue] = field(default_factory=list)
    stats: Dict[str, int] = field(default_factory=dict)
    
    def add_issue(self, issue: SecurityIssue):
        """Add a security issue to the report."""
        self.issues.append(issue)
        
        # Update stats
        severity_key = f"{issue.severity.value}_count"
        self.stats[severity_key] = self.stats.get(severity_key, 0) + 1
    
    def get_critical_issues(self) -> List[SecurityIssue]:
        """Get all critical severity issues."""
        return [i for i in self.issues if i.severity == Severity.CRITICAL]
    
    def get_high_issues(self) -> List[SecurityIssue]:
        """Get all high severity issues."""
        return [i for i in self.issues if i.severity == Severity.HIGH]
    
    def has_critical_issues(self) -> bool:
        """Check if report contains critical issues."""
        return len(self.get_critical_issues()) > 0
    
    def summary(self) -> str:
        """Generate a summary of the audit."""
        critical = self.stats.get('critical_count', 0)
        high = self.stats.get('high_count', 0)
        medium = self.stats.get('medium_count', 0)
        low = self.stats.get('low_count', 0)
        info = self.stats.get('info_count', 0)
        
        return (
            f"Security Audit Summary:\n"
            f"  Duration: {self.duration:.2f}s\n"
            f"  Total Checks: {self.total_checks}\n"
            f"  Issues Found: {len(self.issues)}\n"
            f"    - Critical: {critical}\n"
            f"    - High: {high}\n"
            f"    - Medium: {medium}\n"
            f"    - Low: {low}\n"
            f"    - Info: {info}\n"
        )
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert report to dictionary."""
        return {
            'timestamp': self.timestamp,
            'duration': self.duration,
            'total_checks': self.total_checks,
            'issues': [issue.to_dict() for issue in self.issues],
            'stats': self.stats,
            'summary': self.summary(),
        }
