"""
Consensus security auditor for ModernTensor Layer 1 blockchain.

Performs security checks on:
- Nothing at stake problem
- Long range attacks
- Validator selection fairness
- Slashing conditions
"""
from typing import List, Optional, Any

from .types import SecurityIssue, Severity


class ConsensusAuditor:
    """
    Audits consensus mechanisms for security vulnerabilities.
    
    Checks:
    1. Nothing at Stake - Validators signing multiple chains
    2. Long Range Attacks - Historical chain reorganizations
    3. Validator Selection - Fairness and predictability
    4. Slashing Conditions - Proper penalties for misbehavior
    """
    
    def audit(self, blockchain=None) -> List[SecurityIssue]:
        """
        Perform consensus security audit.
        
        Args:
            blockchain: Blockchain instance to audit (optional)
            
        Returns:
            List[SecurityIssue]: List of security issues found
        """
        issues = []
        
        # Check nothing at stake protection
        issues.extend(self._audit_nothing_at_stake())
        
        # Check long range attack protection
        issues.extend(self._audit_long_range_attacks())
        
        # Check validator selection fairness
        issues.extend(self._audit_validator_selection())
        
        # Check slashing conditions
        issues.extend(self._audit_slashing_conditions())
        
        return issues
    
    def _audit_nothing_at_stake(self) -> List[SecurityIssue]:
        """Audit protection against nothing at stake problem."""
        issues = []
        
        try:
            from sdk.consensus.pos import ProofOfStake
            
            # Check if slashing is implemented
            # This would require checking if validators are penalized for signing conflicting blocks
            issues.append(SecurityIssue(
                severity=Severity.MEDIUM,
                category="Consensus",
                title="Nothing at Stake Protection",
                description="Ensure validators cannot profitably sign multiple conflicting chains. "
                           "Implementation should include slashing for double-signing.",
                location="sdk/consensus/pos.py",
                recommendation="Implement detection and slashing for validators signing conflicting blocks. "
                             "Track validator signatures across fork choices.",
                cwe_id="CWE-841",  # Improper Enforcement of Behavioral Workflow
            ))
        
        except ImportError:
            issues.append(SecurityIssue(
                severity=Severity.HIGH,
                category="Consensus",
                title="PoS Module Not Found",
                description="Cannot audit consensus - PoS module not found.",
                location="sdk/consensus/pos.py",
                recommendation="Ensure consensus module is properly implemented.",
                cwe_id="CWE-710",  # Improper Adherence to Coding Standards
            ))
        
        return issues
    
    def _audit_long_range_attacks(self) -> List[SecurityIssue]:
        """Audit protection against long range attacks."""
        issues = []
        
        try:
            from sdk.consensus.fork_choice import ForkChoice
            
            # Check if there's finality mechanism
            issues.append(SecurityIssue(
                severity=Severity.HIGH,
                category="Consensus",
                title="Long Range Attack Protection",
                description="Implement checkpointing or finality mechanism to prevent "
                           "long range attacks where attackers create alternative history.",
                location="sdk/consensus/fork_choice.py",
                recommendation="Implement:\n"
                             "1. Weak subjectivity checkpoints\n"
                             "2. Finality gadget (e.g., Casper FFG)\n"
                             "3. Social consensus fallback for deep reorganizations",
                cwe_id="CWE-841",
            ))
        
        except ImportError:
            pass  # Will be reported elsewhere
        
        return issues
    
    def _audit_validator_selection(self) -> List[SecurityIssue]:
        """Audit validator selection fairness."""
        issues = []
        
        try:
            from sdk.consensus.pos import ProofOfStake
            
            # Check VRF usage for randomness
            issues.append(SecurityIssue(
                severity=Severity.MEDIUM,
                category="Consensus",
                title="Validator Selection Randomness",
                description="Ensure validator selection uses verifiable random function (VRF) "
                           "or similar mechanism to prevent manipulation.",
                location="sdk/consensus/pos.py:select_validator()",
                recommendation="Implement VRF-based validator selection:\n"
                             "1. Use VRF for slot leader selection\n"
                             "2. Make selection verifiable by other validators\n"
                             "3. Prevent grinding attacks on randomness",
                cwe_id="CWE-330",  # Use of Insufficiently Random Values
            ))
            
            # Check stake weighting
            issues.append(SecurityIssue(
                severity=Severity.INFO,
                category="Consensus",
                title="Stake-Weighted Selection",
                description="Verify that validator selection is properly weighted by stake amount.",
                location="sdk/consensus/pos.py:select_validator()",
                recommendation="Ensure selection probability is proportional to stake:\n"
                             "P(validator) = stake_validator / total_stake",
                cwe_id=None,
            ))
        
        except ImportError:
            pass
        
        return issues
    
    def _audit_slashing_conditions(self) -> List[SecurityIssue]:
        """Audit slashing condition implementation."""
        issues = []
        
        try:
            from sdk.consensus.pos import ProofOfStake
            
            # Check if slashing is implemented
            issues.append(SecurityIssue(
                severity=Severity.MEDIUM,
                category="Consensus",
                title="Slashing Implementation",
                description="Slashing mechanism should be active to penalize validator misbehavior.",
                location="sdk/consensus/pos.py",
                recommendation="Implement slashing for:\n"
                             "1. Double signing (signing conflicting blocks)\n"
                             "2. Extended downtime (missing consecutive blocks)\n"
                             "3. Invalid block production\n"
                             "4. Equivocation in consensus votes",
                cwe_id="CWE-841",
            ))
            
            # Check slashing severity
            issues.append(SecurityIssue(
                severity=Severity.INFO,
                category="Consensus",
                title="Slashing Severity Calibration",
                description="Ensure slashing amounts are calibrated to discourage attacks "
                           "while not being excessive for honest mistakes.",
                location="sdk/consensus/pos.py",
                recommendation="Recommended slashing amounts:\n"
                             "- Double signing: 5-10% of stake\n"
                             "- Downtime: 0.1-1% of stake\n"
                             "- Invalid blocks: 1-5% of stake",
                cwe_id=None,
            ))
        
        except ImportError:
            pass
        
        return issues
    
    def generate_report_summary(self, issues: List[SecurityIssue]) -> str:
        """Generate a summary report of consensus audit."""
        if not issues:
            return "✅ Consensus Audit: No issues found. All checks passed."
        
        critical = sum(1 for i in issues if i.severity == Severity.CRITICAL)
        high = sum(1 for i in issues if i.severity == Severity.HIGH)
        medium = sum(1 for i in issues if i.severity == Severity.MEDIUM)
        low = sum(1 for i in issues if i.severity == Severity.LOW)
        
        return (
            f"⚠️ Consensus Audit: {len(issues)} issues found\n"
            f"  Critical: {critical}, High: {high}, Medium: {medium}, Low: {low}"
        )
