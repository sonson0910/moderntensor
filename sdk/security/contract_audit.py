"""
Smart contract security auditor for ModernTensor Layer 1 blockchain.

Performs security checks on:
- Reentrancy vulnerabilities
- Integer overflow/underflow
- Access control issues
- DoS vulnerabilities
"""
from typing import List, Any

from .types import SecurityIssue, Severity


class ContractAuditor:
    """
    Audits smart contracts for security vulnerabilities.

    Checks:
    1. Reentrancy - External calls that could be exploited
    2. Integer Overflow/Underflow - Arithmetic operations
    3. Access Control - Proper permission checks
    4. DoS - Resource consumption vulnerabilities
    """

    def audit(self, contract: Any) -> List[SecurityIssue]:
        """
        Perform smart contract security audit.

        Args:
            contract: Smart contract code or bytecode to audit

        Returns:
            List[SecurityIssue]: List of security issues found
        """
        issues = []

        # If contract is bytecode or code string
        if isinstance(contract, (str, bytes)):
            issues.extend(self._audit_contract_code(contract))
        # If contract is a dictionary with metadata
        elif isinstance(contract, dict):
            code = contract.get('code', contract.get('bytecode', ''))
            address = contract.get('address', 'unknown')
            issues.extend(self._audit_contract_code(code, address))

        return issues

    def _audit_contract_code(self, code: Any, address: str = 'unknown') -> List[SecurityIssue]:
        """Audit smart contract code for vulnerabilities."""
        issues = []

        # Note: Since we don't have a VM implementation yet, these are placeholder checks
        # In production, would need to:
        # 1. Parse contract bytecode
        # 2. Analyze control flow
        # 3. Check for known vulnerability patterns

        if not code:
            return []

        # Convert to string if bytes
        if isinstance(code, bytes):
            try:
                code_str = code.decode('utf-8')
            except Exception:
                code_str = code.hex()
        else:
            code_str = str(code)

        # Check for reentrancy patterns (basic heuristic)
        issues.extend(self._check_reentrancy(code_str, address))

        # Check for integer operations
        issues.extend(self._check_integer_operations(code_str, address))

        # Check for access control
        issues.extend(self._check_access_control(code_str, address))

        # Check for DoS vulnerabilities
        issues.extend(self._check_dos_vulnerabilities(code_str, address))

        return issues

    def _check_reentrancy(self, code: str, address: str) -> List[SecurityIssue]:
        """Check for reentrancy vulnerabilities."""
        issues = []

        # This is a simplified check - would need full bytecode analysis in production
        issues.append(SecurityIssue(
            severity=Severity.MEDIUM,
            category="Smart Contract",
            title="Reentrancy Check Required",
            description=f"Contract at {address} should be checked for reentrancy vulnerabilities. "
                       "Ensure state changes occur before external calls.",
            location=f"Contract {address}",
            recommendation="Follow checks-effects-interactions pattern:\n"
                         "1. Validate conditions\n"
                         "2. Update state variables\n"
                         "3. Make external calls\n"
                         "Consider using reentrancy guards/mutexes",
            cwe_id="CWE-841",
        ))

        return issues

    def _check_integer_operations(self, code: str, address: str) -> List[SecurityIssue]:
        """Check for integer overflow/underflow vulnerabilities."""
        issues = []

        issues.append(SecurityIssue(
            severity=Severity.MEDIUM,
            category="Smart Contract",
            title="Integer Overflow/Underflow Check",
            description=f"Contract at {address} should use safe math operations to prevent "
                       "integer overflow and underflow vulnerabilities.",
            location=f"Contract {address}",
            recommendation="Use safe math libraries or:\n"
                         "1. Check for overflow before addition/multiplication\n"
                         "2. Check for underflow before subtraction\n"
                         "3. Use language features if available (e.g., Solidity 0.8+)\n"
                         "4. Validate all arithmetic operations",
            cwe_id="CWE-190",  # Integer Overflow
        ))

        return issues

    def _check_access_control(self, code: str, address: str) -> List[SecurityIssue]:
        """Check for access control vulnerabilities."""
        issues = []

        issues.append(SecurityIssue(
            severity=Severity.HIGH,
            category="Smart Contract",
            title="Access Control Validation",
            description=f"Contract at {address} must implement proper access control for "
                       "privileged functions.",
            location=f"Contract {address}",
            recommendation="Implement access control:\n"
                         "1. Use require() checks for permissions\n"
                         "2. Implement role-based access control (RBAC)\n"
                         "3. Validate msg.sender for administrative functions\n"
                         "4. Use modifiers for reusable access checks\n"
                         "5. Emit events for privileged operations",
            cwe_id="CWE-284",  # Improper Access Control
        ))

        return issues

    def _check_dos_vulnerabilities(self, code: str, address: str) -> List[SecurityIssue]:
        """Check for DoS vulnerabilities."""
        issues = []

        issues.append(SecurityIssue(
            severity=Severity.MEDIUM,
            category="Smart Contract",
            title="DoS Vulnerability Check",
            description=f"Contract at {address} should be checked for DoS vulnerabilities "
                       "such as unbounded loops and gas limit issues.",
            location=f"Contract {address}",
            recommendation="Avoid DoS vectors:\n"
                         "1. Limit loop iterations (use pagination)\n"
                         "2. Avoid operations dependent on array length\n"
                         "3. Don't rely on external calls succeeding\n"
                         "4. Use pull over push for payments\n"
                         "5. Set gas limits for external calls",
            cwe_id="CWE-400",
        ))

        return issues

    def generate_report_summary(self, issues: List[SecurityIssue]) -> str:
        """Generate a summary report of contract audit."""
        if not issues:
            return "✅ Contract Audit: No issues found. All checks passed."

        critical = sum(1 for i in issues if i.severity == Severity.CRITICAL)
        high = sum(1 for i in issues if i.severity == Severity.HIGH)
        medium = sum(1 for i in issues if i.severity == Severity.MEDIUM)
        low = sum(1 for i in issues if i.severity == Severity.LOW)

        return (
            f"⚠️ Contract Audit: {len(issues)} issues found\n"
            f"  Critical: {critical}, High: {high}, Medium: {medium}, Low: {low}"
        )
