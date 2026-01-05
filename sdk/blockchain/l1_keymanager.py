"""
Layer 1 Key Management for ModernTensor

Replacement for pycardano HDWallet with Layer 1 blockchain key derivation.
Uses BIP39 for mnemonic and BIP32 for HD derivation.
"""
import hashlib
import json
import os
from typing import Optional, Dict, Any
from dataclasses import dataclass

from bip_utils import (
    Bip39MnemonicGenerator,
    Bip39SeedGenerator,
    Bip32Slip10Secp256k1,
    Bip39Languages,
    Bip39WordsNum,
)

from .crypto import KeyPair


@dataclass
class L1Address:
    """
    Layer 1 address representation.
    
    Attributes:
        address: 20-byte address
        address_hex: Hex string representation
    """
    address: bytes
    
    @property
    def address_hex(self) -> str:
        """Get hex representation with 0x prefix."""
        return "0x" + self.address.hex()
    
    def __str__(self) -> str:
        return self.address_hex
    
    def __repr__(self) -> str:
        return f"L1Address({self.address_hex})"


class L1HDWallet:
    """
    Hierarchical Deterministic Wallet for Layer 1 blockchain.
    
    Replaces pycardano.HDWallet with BIP32/BIP39 standard derivation.
    Uses secp256k1 curve (Ethereum-style).
    """
    
    def __init__(self, mnemonic: Optional[str] = None, passphrase: str = ""):
        """
        Initialize HD wallet from mnemonic or generate new one.
        
        Args:
            mnemonic: BIP39 mnemonic phrase (12-24 words)
            passphrase: Optional BIP39 passphrase
        """
        if mnemonic is None:
            # Generate new 24-word mnemonic
            mnemonic_obj = Bip39MnemonicGenerator().FromWordsNumber(Bip39WordsNum.WORDS_NUM_24)
            self.mnemonic = mnemonic_obj.ToStr()
        else:
            self.mnemonic = mnemonic
        
        # Generate seed from mnemonic + passphrase
        seed_bytes = Bip39SeedGenerator(self.mnemonic).Generate(passphrase)
        
        # Create master key using BIP32-SLIP10 with secp256k1
        self.master_key = Bip32Slip10Secp256k1.FromSeed(seed_bytes)
        
        # Cache for derived keys
        self._derived_keys: Dict[str, Any] = {}
    
    def derive_key(self, path: str) -> KeyPair:
        """
        Derive a key at a given BIP32 path.
        
        Args:
            path: BIP32 derivation path (e.g., "m/44'/0'/0'/0/0")
                 Standard paths:
                 - m/44'/0'/0'/0/0 - First account, first key
                 - m/44'/0'/0'/0/1 - First account, second key
                 
        Returns:
            KeyPair: Derived key pair
        """
        if path in self._derived_keys:
            return self._derived_keys[path]
        
        # Parse path and derive
        path_parts = path.replace("m/", "").split("/")
        
        derived = self.master_key
        for part in path_parts:
            if part.endswith("'") or part.endswith("h"):
                # Hardened derivation
                index = int(part[:-1]) + 0x80000000
            else:
                # Normal derivation
                index = int(part)
            
            derived = derived.ChildKey(index)
        
        # Extract private key bytes (32 bytes)
        private_key_bytes = derived.PrivateKey().Raw().ToBytes()
        
        # Create KeyPair
        keypair = KeyPair(private_key_bytes)
        
        # Cache it
        self._derived_keys[path] = keypair
        
        return keypair
    
    def derive_hotkey(self, index: int = 0) -> KeyPair:
        """
        Derive a hotkey at the given index.
        
        Uses path: m/44'/0'/0'/0/{index}
        
        Args:
            index: Derivation index
            
        Returns:
            KeyPair: Hotkey pair
        """
        path = f"m/44'/0'/0'/0/{index}"
        return self.derive_key(path)
    
    def get_root_address(self) -> L1Address:
        """
        Get the root address (first key in default path).
        
        Returns:
            L1Address: Root address
        """
        keypair = self.derive_key("m/44'/0'/0'/0/0")
        return L1Address(address=keypair.address())
    
    def get_hotkey_address(self, index: int = 0) -> L1Address:
        """
        Get hotkey address at given index.
        
        Args:
            index: Hotkey derivation index
            
        Returns:
            L1Address: Hotkey address
        """
        keypair = self.derive_hotkey(index)
        return L1Address(address=keypair.address())
    
    @classmethod
    def from_mnemonic(cls, mnemonic: str, passphrase: str = "") -> "L1HDWallet":
        """
        Create wallet from existing mnemonic.
        
        Args:
            mnemonic: BIP39 mnemonic phrase
            passphrase: Optional BIP39 passphrase
            
        Returns:
            L1HDWallet: Wallet instance
        """
        return cls(mnemonic=mnemonic, passphrase=passphrase)
    
    @classmethod
    def generate_mnemonic(cls, words_num: int = 24) -> str:
        """
        Generate a new BIP39 mnemonic.
        
        Args:
            words_num: Number of words (12, 15, 18, 21, or 24)
            
        Returns:
            str: Mnemonic phrase
        """
        words_map = {
            12: Bip39WordsNum.WORDS_NUM_12,
            15: Bip39WordsNum.WORDS_NUM_15,
            18: Bip39WordsNum.WORDS_NUM_18,
            21: Bip39WordsNum.WORDS_NUM_21,
            24: Bip39WordsNum.WORDS_NUM_24,
        }
        
        if words_num not in words_map:
            raise ValueError(f"Invalid words_num: {words_num}. Must be 12, 15, 18, 21, or 24")
        
        mnemonic_obj = Bip39MnemonicGenerator().FromWordsNumber(words_map[words_num])
        return mnemonic_obj.ToStr()


class L1Network:
    """
    Network type for Layer 1 blockchain.
    
    Replaces pycardano.Network.
    """
    MAINNET = "mainnet"
    TESTNET = "testnet"
    DEVNET = "devnet"
    
    def __init__(self, network_type: str = TESTNET):
        """
        Initialize network.
        
        Args:
            network_type: Network type (mainnet, testnet, devnet)
        """
        if network_type not in [self.MAINNET, self.TESTNET, self.DEVNET]:
            raise ValueError(f"Invalid network type: {network_type}")
        
        self.network_type = network_type
    
    def __str__(self) -> str:
        return self.network_type
    
    def __repr__(self) -> str:
        return f"L1Network({self.network_type})"


# Convenience instances
Network = L1Network  # For backward compatibility
MAINNET = L1Network(L1Network.MAINNET)
TESTNET = L1Network(L1Network.TESTNET)
DEVNET = L1Network(L1Network.DEVNET)
