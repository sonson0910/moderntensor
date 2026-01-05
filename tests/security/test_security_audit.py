"""Tests for security audit module."""
import pytest
from sdk.security import SecurityAuditor, Severity
from sdk.security.crypto_audit import CryptoAuditor
from sdk.security.consensus_audit import ConsensusAuditor
from sdk.security.network_audit import NetworkAuditor
from sdk.security.contract_audit import ContractAuditor


class TestSecurityAuditor:
    """Test the main security auditor."""
    
    def test_auditor_initialization(self):
        """Test security auditor can be initialized."""
        auditor = SecurityAuditor()
        assert auditor is not None
        assert auditor.crypto_auditor is not None
        assert auditor.consensus_auditor is not None
        assert auditor.network_auditor is not None
        assert auditor.contract_auditor is not None
    
    def test_audit_all(self):
        """Test comprehensive audit."""
        auditor = SecurityAuditor()
        report = auditor.audit_all()
        
        assert report is not None
        assert report.timestamp > 0
        assert report.duration >= 0
        assert isinstance(report.issues, list)
        assert isinstance(report.stats, dict)
    
    def test_audit_report_summary(self):
        """Test audit report summary generation."""
        auditor = SecurityAuditor()
        report = auditor.audit_all()
        
        summary = report.summary()
        assert "Security Audit Summary" in summary
        assert "Duration" in summary
        assert "Total Checks" in summary


class TestCryptoAuditor:
    """Test cryptography auditor."""
    
    def test_crypto_audit(self):
        """Test cryptography audit."""
        auditor = CryptoAuditor()
        issues = auditor.audit()
        
        assert isinstance(issues, list)
        # Should find some issues or none
        for issue in issues:
            assert issue.severity in [Severity.CRITICAL, Severity.HIGH, Severity.MEDIUM, Severity.LOW, Severity.INFO]
            assert issue.category == "Cryptography"
    
    def test_crypto_report_summary(self):
        """Test crypto audit report summary."""
        auditor = CryptoAuditor()
        issues = auditor.audit()
        summary = auditor.generate_report_summary(issues)
        
        assert summary is not None
        assert "Cryptography Audit" in summary


class TestConsensusAuditor:
    """Test consensus auditor."""
    
    def test_consensus_audit(self):
        """Test consensus audit."""
        auditor = ConsensusAuditor()
        issues = auditor.audit()
        
        assert isinstance(issues, list)
        for issue in issues:
            assert issue.category == "Consensus"
    
    def test_consensus_report_summary(self):
        """Test consensus audit report summary."""
        auditor = ConsensusAuditor()
        issues = auditor.audit()
        summary = auditor.generate_report_summary(issues)
        
        assert summary is not None
        assert "Consensus Audit" in summary


class TestNetworkAuditor:
    """Test network auditor."""
    
    def test_network_audit(self):
        """Test network audit."""
        auditor = NetworkAuditor()
        issues = auditor.audit()
        
        assert isinstance(issues, list)
        for issue in issues:
            assert issue.category == "Network"
    
    def test_network_report_summary(self):
        """Test network audit report summary."""
        auditor = NetworkAuditor()
        issues = auditor.audit()
        summary = auditor.generate_report_summary(issues)
        
        assert summary is not None
        assert "Network Audit" in summary


class TestContractAuditor:
    """Test contract auditor."""
    
    def test_contract_audit_empty(self):
        """Test contract audit with no contract."""
        auditor = ContractAuditor()
        issues = auditor.audit(None)
        
        assert isinstance(issues, list)
        assert len(issues) == 0
    
    def test_contract_audit_with_code(self):
        """Test contract audit with sample code."""
        auditor = ContractAuditor()
        sample_contract = {
            'address': '0x1234',
            'code': b'sample bytecode'
        }
        issues = auditor.audit(sample_contract)
        
        assert isinstance(issues, list)
        # Should find some potential issues
        for issue in issues:
            assert issue.category == "Smart Contract"
    
    def test_contract_report_summary(self):
        """Test contract audit report summary."""
        auditor = ContractAuditor()
        issues = auditor.audit({'code': b'test'})
        summary = auditor.generate_report_summary(issues)
        
        assert summary is not None
        assert "Contract Audit" in summary
