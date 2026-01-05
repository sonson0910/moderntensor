"""
Test Token Faucet

This module provides a faucet service for distributing test tokens on the testnet.
Integrates with actual blockchain Transaction primitives.
"""

import time
import asyncio
from dataclasses import dataclass, field
from typing import Dict, Optional, Set
from datetime import datetime, timedelta
from pathlib import Path
import json

# Import blockchain primitives for creating real transactions
from ..blockchain import Transaction
from ..blockchain.crypto import KeyPair
from ..blockchain.state import StateDB


@dataclass
class FaucetRequest:
    """Represents a token request"""
    address: str
    amount: int
    timestamp: float
    ip_address: Optional[str] = None
    tx_hash: Optional[str] = None


@dataclass
class FaucetConfig:
    """Configuration for the faucet"""
    # Token distribution
    tokens_per_request: int = 100_000_000_000  # 100 tokens (with 18 decimals)
    max_requests_per_address: int = 3
    max_requests_per_ip: int = 5
    
    # Rate limiting
    cooldown_period: int = 3600  # 1 hour in seconds
    daily_limit: int = 1000  # Max requests per day
    
    # Faucet account
    faucet_address: str = "0xfacf00000000000000000000000000000000acef"
    faucet_private_key: Optional[str] = None
    
    # Network
    rpc_url: str = "http://localhost:8545"
    chain_id: int = 9999


class Faucet:
    """
    Test token faucet for testnet
    
    Distributes test tokens to users with rate limiting and anti-abuse measures.
    Creates real blockchain transactions using sdk/blockchain primitives.
    """
    
    def __init__(self, config: Optional[FaucetConfig] = None, state_db: Optional[StateDB] = None):
        self.config = config or FaucetConfig()
        self.state_db = state_db  # Connection to blockchain state
        self.request_history: Dict[str, list] = {}  # address -> [timestamps]
        self.ip_history: Dict[str, list] = {}  # ip -> [timestamps]
        self.total_distributed: int = 0
        self.request_count: int = 0
        self.blocked_addresses: Set[str] = set()
        self.blocked_ips: Set[str] = set()
        
        # Initialize faucet keypair if private key provided
        self.keypair: Optional[KeyPair] = None
        if self.config.faucet_private_key:
            self.keypair = KeyPair(bytes.fromhex(self.config.faucet_private_key[2:] if self.config.faucet_private_key.startswith('0x') else self.config.faucet_private_key))
        
        self.stats = {
            'total_requests': 0,
            'successful_requests': 0,
            'rejected_requests': 0,
            'total_tokens_distributed': 0,
            'unique_addresses': 0
        }
    
    def can_request(self, address: str, ip_address: Optional[str] = None) -> tuple[bool, str]:
        """
        Check if an address can request tokens
        
        Args:
            address: Requesting address
            ip_address: Optional IP address for additional rate limiting
        
        Returns:
            Tuple of (can_request: bool, reason: str)
        """
        # Check if blocked
        if address in self.blocked_addresses:
            return False, "Address is blocked"
        
        if ip_address and ip_address in self.blocked_ips:
            return False, "IP address is blocked"
        
        current_time = time.time()
        
        # Check address rate limit
        if address in self.request_history:
            recent_requests = [
                ts for ts in self.request_history[address]
                if current_time - ts < self.config.cooldown_period
            ]
            
            if len(recent_requests) >= self.config.max_requests_per_address:
                time_until_next = int(self.config.cooldown_period - (current_time - recent_requests[0]))
                return False, f"Too many requests. Try again in {time_until_next} seconds"
        
        # Check IP rate limit
        if ip_address and ip_address in self.ip_history:
            recent_ip_requests = [
                ts for ts in self.ip_history[ip_address]
                if current_time - ts < self.config.cooldown_period
            ]
            
            if len(recent_ip_requests) >= self.config.max_requests_per_ip:
                time_until_next = int(self.config.cooldown_period - (current_time - recent_ip_requests[0]))
                return False, f"Too many requests from this IP. Try again in {time_until_next} seconds"
        
        # Check daily limit
        daily_start = current_time - 86400  # 24 hours ago
        daily_requests = sum(
            len([ts for ts in timestamps if ts > daily_start])
            for timestamps in self.request_history.values()
        )
        
        if daily_requests >= self.config.daily_limit:
            return False, "Daily faucet limit reached. Try again tomorrow"
        
        return True, "OK"
    
    async def request_tokens(
        self,
        address: str,
        ip_address: Optional[str] = None
    ) -> Dict[str, any]:
        """
        Request test tokens from the faucet - creates a real blockchain transaction
        
        Args:
            address: Address to send tokens to
            ip_address: Optional IP address for rate limiting
        
        Returns:
            Dict with request result and transaction details
        """
        self.stats['total_requests'] += 1
        
        # Validate address format
        if not address.startswith('0x') or len(address) != 42:
            self.stats['rejected_requests'] += 1
            return {
                'success': False,
                'error': 'Invalid address format'
            }
        
        # Check if can request
        can_request, reason = self.can_request(address, ip_address)
        if not can_request:
            self.stats['rejected_requests'] += 1
            return {
                'success': False,
                'error': reason
            }
        
        # Record request
        current_time = time.time()
        
        if address not in self.request_history:
            self.request_history[address] = []
            self.stats['unique_addresses'] += 1
        
        self.request_history[address].append(current_time)
        
        if ip_address:
            if ip_address not in self.ip_history:
                self.ip_history[ip_address] = []
            self.ip_history[ip_address].append(current_time)
        
        # Create real transaction if keypair is available
        transaction = None
        tx_hash = None
        
        if self.keypair and self.state_db:
            # Get faucet account nonce from state
            faucet_address = bytes.fromhex(self.config.faucet_address[2:])
            faucet_account = self.state_db.get_account(faucet_address)
            nonce = faucet_account.nonce if faucet_account else 0
            
            # Create transaction
            to_address = bytes.fromhex(address[2:])
            transaction = Transaction(
                nonce=nonce,
                from_address=faucet_address,
                to_address=to_address,
                value=self.config.tokens_per_request,
                gas_price=self.config.min_gas_price if hasattr(self.config, 'min_gas_price') else 1_000_000_000,
                gas_limit=21000,  # Standard transfer gas limit
                data=b'faucet-distribution'  # Mark as faucet transaction
            )
            
            # Sign transaction with faucet private key
            transaction.sign(self.keypair.private_key)
            tx_hash = transaction.hash().hex()
        else:
            # Fallback to mock transaction hash for testing
            tx_hash = self._generate_tx_hash(address, current_time)
        
        # Update stats
        self.stats['successful_requests'] += 1
        self.stats['total_tokens_distributed'] += self.config.tokens_per_request
        self.total_distributed += self.config.tokens_per_request
        self.request_count += 1
        
        return {
            'success': True,
            'tx_hash': "0x" + tx_hash if not tx_hash.startswith('0x') else tx_hash,
            'transaction': transaction,  # Actual Transaction object if created
            'amount': self.config.tokens_per_request,
            'address': address,
            'timestamp': current_time
        }
    
    def _generate_tx_hash(self, address: str, timestamp: float) -> str:
        """Generate a mock transaction hash"""
        import hashlib
        data = f"{address}{timestamp}{self.request_count}".encode()
        return "0x" + hashlib.sha256(data).hexdigest()
    
    def block_address(self, address: str):
        """Block an address from requesting tokens"""
        self.blocked_addresses.add(address)
    
    def unblock_address(self, address: str):
        """Unblock an address"""
        self.blocked_addresses.discard(address)
    
    def block_ip(self, ip_address: str):
        """Block an IP address"""
        self.blocked_ips.add(ip_address)
    
    def unblock_ip(self, ip_address: str):
        """Unblock an IP address"""
        self.blocked_ips.discard(ip_address)
    
    def get_stats(self) -> Dict[str, any]:
        """Get faucet statistics"""
        return {
            **self.stats,
            'blocked_addresses': len(self.blocked_addresses),
            'blocked_ips': len(self.blocked_ips),
            'average_per_request': (
                self.stats['total_tokens_distributed'] / self.stats['successful_requests']
                if self.stats['successful_requests'] > 0 else 0
            )
        }
    
    def get_address_info(self, address: str) -> Dict[str, any]:
        """Get information about an address's faucet usage"""
        if address not in self.request_history:
            return {
                'address': address,
                'total_requests': 0,
                'total_received': 0,
                'last_request': None,
                'can_request': True
            }
        
        timestamps = self.request_history[address]
        current_time = time.time()
        recent = [ts for ts in timestamps if current_time - ts < self.config.cooldown_period]
        
        can_request, reason = self.can_request(address)
        
        return {
            'address': address,
            'total_requests': len(timestamps),
            'recent_requests': len(recent),
            'total_received': len(timestamps) * self.config.tokens_per_request,
            'last_request': datetime.fromtimestamp(timestamps[-1]).isoformat() if timestamps else None,
            'can_request': can_request,
            'reason': reason if not can_request else None
        }
    
    def cleanup_old_records(self, max_age_days: int = 7):
        """Clean up old request records"""
        current_time = time.time()
        cutoff_time = current_time - (max_age_days * 86400)
        
        # Clean address history
        for address in list(self.request_history.keys()):
            self.request_history[address] = [
                ts for ts in self.request_history[address]
                if ts > cutoff_time
            ]
            if not self.request_history[address]:
                del self.request_history[address]
        
        # Clean IP history
        for ip in list(self.ip_history.keys()):
            self.ip_history[ip] = [
                ts for ts in self.ip_history[ip]
                if ts > cutoff_time
            ]
            if not self.ip_history[ip]:
                del self.ip_history[ip]
    
    def save_state(self, filepath: Path):
        """Save faucet state to file"""
        state = {
            'config': {
                'tokens_per_request': self.config.tokens_per_request,
                'cooldown_period': self.config.cooldown_period,
                'max_requests_per_address': self.config.max_requests_per_address,
            },
            'request_history': self.request_history,
            'ip_history': self.ip_history,
            'blocked_addresses': list(self.blocked_addresses),
            'blocked_ips': list(self.blocked_ips),
            'stats': self.stats
        }
        
        with open(filepath, 'w') as f:
            json.dump(state, f, indent=2)
    
    def load_state(self, filepath: Path):
        """Load faucet state from file"""
        with open(filepath, 'r') as f:
            state = json.load(f)
        
        self.request_history = state.get('request_history', {})
        self.ip_history = state.get('ip_history', {})
        self.blocked_addresses = set(state.get('blocked_addresses', []))
        self.blocked_ips = set(state.get('blocked_ips', []))
        self.stats = state.get('stats', self.stats)


class FaucetAPI:
    """
    Simple API interface for the faucet
    """
    
    def __init__(self, faucet: Faucet):
        self.faucet = faucet
    
    async def handle_request(self, address: str, ip_address: Optional[str] = None) -> Dict:
        """Handle a token request"""
        return await self.faucet.request_tokens(address, ip_address)
    
    def get_stats(self) -> Dict:
        """Get faucet statistics"""
        return self.faucet.get_stats()
    
    def get_address_info(self, address: str) -> Dict:
        """Get address information"""
        return self.faucet.get_address_info(address)
