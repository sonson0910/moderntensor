"""
Main security auditor coordinating all security checks.
"""
from typing import List
import time

from .types import SecurityIssue, AuditReport
from .crypto_audit import CryptoAuditor
from .consensus_audit import ConsensusAuditor
from .network_audit import NetworkAuditor
from .contract_audit import ContractAuditor


class SecurityAuditor:
    """
    Main security auditor that coordinates all security checks.
    
    Performs comprehensive security auditing across:
    - Cryptography (key management, signatures, hashing)
    - Consensus (PoS security, validator selection, slashing)
    - Network (eclipse attacks, DDoS, Sybil resistance)
    - Smart contracts (reentrancy, overflow, access control)
    """
    
    def __init__(self):
        """Initialize the security auditor with all sub-auditors."""
        self.crypto_auditor = CryptoAuditor()
        self.consensus_auditor = ConsensusAuditor()
        self.network_auditor = NetworkAuditor()
        self.contract_auditor = ContractAuditor()
    
    def audit_all(self, blockchain=None, network=None, contracts=None) -> AuditReport:
        """
        Perform comprehensive security audit.
        
        Args:
            blockchain: Blockchain instance to audit
            network: Network instance to audit
            contracts: List of smart contracts to audit
            
        Returns:
            AuditReport: Comprehensive audit report
        """
        start_time = time.time()
        report = AuditReport(
            timestamp=start_time,
            duration=0.0,
            total_checks=0,
        )
        
        # Run cryptography audit
        crypto_issues = self.crypto_auditor.audit(blockchain)
        for issue in crypto_issues:
            report.add_issue(issue)
        report.total_checks += len(crypto_issues)
        
        # Run consensus audit
        consensus_issues = self.consensus_auditor.audit(blockchain)
        for issue in consensus_issues:
            report.add_issue(issue)
        report.total_checks += len(consensus_issues)
        
        # Run network audit
        if network:
            network_issues = self.network_auditor.audit(network)
            for issue in network_issues:
                report.add_issue(issue)
            report.total_checks += len(network_issues)
        
        # Run contract audit
        if contracts:
            for contract in contracts:
                contract_issues = self.contract_auditor.audit(contract)
                for issue in contract_issues:
                    report.add_issue(issue)
                report.total_checks += len(contract_issues)
        
        # Finalize report
        report.duration = time.time() - start_time
        
        return report
    
    def audit_cryptography(self, blockchain=None) -> List[SecurityIssue]:
        """Run only cryptography audits."""
        return self.crypto_auditor.audit(blockchain)
    
    def audit_consensus(self, blockchain=None) -> List[SecurityIssue]:
        """Run only consensus audits."""
        return self.consensus_auditor.audit(blockchain)
    
    def audit_network(self, network=None) -> List[SecurityIssue]:
        """Run only network audits."""
        return self.network_auditor.audit(network)
    
    def audit_contracts(self, contracts=None) -> List[SecurityIssue]:
        """Run only smart contract audits."""
        if not contracts:
            return []
        
        all_issues = []
        for contract in contracts:
            issues = self.contract_auditor.audit(contract)
            all_issues.extend(issues)
        return all_issues
