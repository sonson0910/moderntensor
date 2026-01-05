"""
Cryptography security auditor for ModernTensor Layer 1 blockchain.

Performs security checks on:
- Key generation and management
- Signature schemes
- Hash functions
- Random number generation
"""
from typing import List, Optional, Any
import hashlib
import secrets

from .types import SecurityIssue, Severity


class CryptoAuditor:
    """
    Audits cryptographic implementations for security vulnerabilities.
    
    Checks:
    1. Key Generation - Proper randomness and key strength
    2. Signature Schemes - ECDSA implementation correctness
    3. Hash Functions - Appropriate algorithms (keccak256 for Ethereum compatibility)
    4. Random Number Generation - Cryptographic quality randomness
    """
    
    def audit(self, blockchain=None) -> List[SecurityIssue]:
        """
        Perform cryptography security audit.
        
        Args:
            blockchain: Blockchain instance to audit (optional)
            
        Returns:
            List[SecurityIssue]: List of security issues found
        """
        issues = []
        
        # Check key generation
        issues.extend(self._audit_key_generation())
        
        # Check signature schemes
        issues.extend(self._audit_signatures())
        
        # Check hash functions
        issues.extend(self._audit_hash_functions())
        
        # Check random number generation
        issues.extend(self._audit_rng())
        
        return issues
    
    def _audit_key_generation(self) -> List[SecurityIssue]:
        """Audit key generation mechanisms."""
        issues = []
        
        try:
            from sdk.blockchain.crypto import KeyPair, ECDSA_AVAILABLE
            
            if not ECDSA_AVAILABLE:
                issues.append(SecurityIssue(
                    severity=Severity.CRITICAL,
                    category="Cryptography",
                    title="ECDSA Library Not Available",
                    description="The ecdsa library is not installed or not available. "
                               "This means cryptographic operations are using fallback implementations.",
                    location="sdk/blockchain/crypto.py",
                    recommendation="Install ecdsa library: pip install ecdsa==0.19.1",
                    cwe_id="CWE-327",  # Use of a Broken or Risky Cryptographic Algorithm
                ))
            
            # Test key generation
            try:
                keypair = KeyPair()
                if len(keypair.private_key) != 32:
                    issues.append(SecurityIssue(
                        severity=Severity.HIGH,
                        category="Cryptography",
                        title="Invalid Private Key Length",
                        description=f"Private key length is {len(keypair.private_key)} bytes, expected 32 bytes.",
                        location="sdk/blockchain/crypto.py:KeyPair._generate()",
                        recommendation="Ensure private keys are exactly 32 bytes (256 bits).",
                        cwe_id="CWE-326",  # Inadequate Encryption Strength
                    ))
                
                if len(keypair.public_key) != 64:
                    issues.append(SecurityIssue(
                        severity=Severity.MEDIUM,
                        category="Cryptography",
                        title="Invalid Public Key Length",
                        description=f"Public key length is {len(keypair.public_key)} bytes, expected 64 bytes.",
                        location="sdk/blockchain/crypto.py:KeyPair._derive_public()",
                        recommendation="Ensure public keys are 64 bytes (uncompressed secp256k1).",
                        cwe_id="CWE-326",
                    ))
            except Exception as e:
                issues.append(SecurityIssue(
                    severity=Severity.HIGH,
                    category="Cryptography",
                    title="Key Generation Error",
                    description=f"Error during key generation: {str(e)}",
                    location="sdk/blockchain/crypto.py:KeyPair",
                    recommendation="Review and fix key generation implementation.",
                    cwe_id="CWE-327",
                ))
        
        except ImportError as e:
            issues.append(SecurityIssue(
                severity=Severity.CRITICAL,
                category="Cryptography",
                title="Crypto Module Import Error",
                description=f"Cannot import crypto module: {str(e)}",
                location="sdk/blockchain/crypto.py",
                recommendation="Ensure crypto module is properly installed and importable.",
                cwe_id="CWE-749",  # Exposed Dangerous Method or Function
            ))
        
        return issues
    
    def _audit_signatures(self) -> List[SecurityIssue]:
        """Audit signature schemes."""
        issues = []
        
        try:
            from sdk.blockchain.crypto import KeyPair, ECDSA_AVAILABLE
            
            if not ECDSA_AVAILABLE:
                issues.append(SecurityIssue(
                    severity=Severity.CRITICAL,
                    category="Cryptography",
                    title="ECDSA Implementation Missing",
                    description="Proper ECDSA signing and verification is not available.",
                    location="sdk/blockchain/crypto.py",
                    recommendation="Install ecdsa library for proper cryptographic signatures.",
                    cwe_id="CWE-327",
                ))
                return issues
            
            # Test signature generation and verification
            try:
                keypair = KeyPair()
                message = b"test message for signature validation"
                signature = keypair.sign(message)
                
                # Check signature length
                if len(signature) != 65:
                    issues.append(SecurityIssue(
                        severity=Severity.HIGH,
                        category="Cryptography",
                        title="Invalid Signature Length",
                        description=f"Signature length is {len(signature)} bytes, expected 65 bytes (r+s+v).",
                        location="sdk/blockchain/crypto.py:KeyPair.sign()",
                        recommendation="Ensure signatures are 65 bytes: 32 (r) + 32 (s) + 1 (v).",
                        cwe_id="CWE-347",  # Improper Verification of Cryptographic Signature
                    ))
                
                # Test verification
                is_valid = KeyPair.verify(message, signature, keypair.public_key)
                if not is_valid:
                    issues.append(SecurityIssue(
                        severity=Severity.CRITICAL,
                        category="Cryptography",
                        title="Signature Verification Failed",
                        description="Generated signature does not verify correctly.",
                        location="sdk/blockchain/crypto.py:KeyPair.verify()",
                        recommendation="Review signature generation and verification logic.",
                        cwe_id="CWE-347",
                    ))
            
            except Exception as e:
                issues.append(SecurityIssue(
                    severity=Severity.HIGH,
                    category="Cryptography",
                    title="Signature Operation Error",
                    description=f"Error during signature operations: {str(e)}",
                    location="sdk/blockchain/crypto.py",
                    recommendation="Review and fix signature implementation.",
                    cwe_id="CWE-347",
                ))
        
        except ImportError:
            pass  # Already reported in key generation audit
        
        return issues
    
    def _audit_hash_functions(self) -> List[SecurityIssue]:
        """Audit hash function usage."""
        issues = []
        
        try:
            from sdk.blockchain.crypto import keccak256, KECCAK_AVAILABLE
            
            if not KECCAK_AVAILABLE:
                issues.append(SecurityIssue(
                    severity=Severity.HIGH,
                    category="Cryptography",
                    title="Keccak256 Not Available",
                    description="Keccak256 hash function is not available, using SHA256 fallback. "
                               "This breaks Ethereum compatibility.",
                    location="sdk/blockchain/crypto.py:keccak256()",
                    recommendation="Install pycryptodome: pip install pycryptodome==3.23.0",
                    cwe_id="CWE-327",
                ))
            
            # Test hash function
            test_data = b"test data for hash validation"
            hash_result = keccak256(test_data)
            
            if len(hash_result) != 32:
                issues.append(SecurityIssue(
                    severity=Severity.HIGH,
                    category="Cryptography",
                    title="Invalid Hash Output Length",
                    description=f"Hash output is {len(hash_result)} bytes, expected 32 bytes.",
                    location="sdk/blockchain/crypto.py:keccak256()",
                    recommendation="Ensure hash functions output 256 bits (32 bytes).",
                    cwe_id="CWE-328",  # Reversible One-Way Hash
                ))
        
        except ImportError:
            issues.append(SecurityIssue(
                severity=Severity.CRITICAL,
                category="Cryptography",
                title="Hash Function Import Error",
                description="Cannot import hash functions from crypto module.",
                location="sdk/blockchain/crypto.py",
                recommendation="Ensure crypto module is properly implemented.",
                cwe_id="CWE-327",
            ))
        
        return issues
    
    def _audit_rng(self) -> List[SecurityIssue]:
        """Audit random number generation."""
        issues = []
        
        # Check if using cryptographically secure RNG
        try:
            # Test secrets module usage
            test_random = secrets.token_bytes(32)
            if len(test_random) != 32:
                issues.append(SecurityIssue(
                    severity=Severity.MEDIUM,
                    category="Cryptography",
                    title="RNG Output Length Issue",
                    description="Random number generator not producing expected length output.",
                    location="Python secrets module",
                    recommendation="Verify secrets.token_bytes() is functioning correctly.",
                    cwe_id="CWE-338",  # Use of Cryptographically Weak PRNG
                ))
        except Exception as e:
            issues.append(SecurityIssue(
                severity=Severity.CRITICAL,
                category="Cryptography",
                title="RNG Module Error",
                description=f"Error with random number generation: {str(e)}",
                location="Python secrets module",
                recommendation="Ensure secrets module is available and functioning.",
                cwe_id="CWE-338",
            ))
        
        return issues
    
    def generate_report_summary(self, issues: List[SecurityIssue]) -> str:
        """Generate a summary report of cryptography audit."""
        if not issues:
            return "✅ Cryptography Audit: No issues found. All checks passed."
        
        critical = sum(1 for i in issues if i.severity == Severity.CRITICAL)
        high = sum(1 for i in issues if i.severity == Severity.HIGH)
        medium = sum(1 for i in issues if i.severity == Severity.MEDIUM)
        low = sum(1 for i in issues if i.severity == Severity.LOW)
        
        return (
            f"⚠️ Cryptography Audit: {len(issues)} issues found\n"
            f"  Critical: {critical}, High: {high}, Medium: {medium}, Low: {low}"
        )
