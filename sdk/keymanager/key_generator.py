"""
Key Generation Module

Provides functionality for generating and deriving cryptographic keys.
Uses native Python cryptography matching Luxtensor implementation.
"""

from typing import Dict, Any
from bip_utils import (
    Bip39MnemonicGenerator, Bip39SeedGenerator, Bip39WordsNum,
    Bip44, Bip44Coins, Bip44Changes
)
from ecdsa import SigningKey, SECP256k1
import secrets


def _derive_address_from_private_key(private_key_hex: str) -> tuple[str, str]:
    """
    Derive address and public key from private key (matching Luxtensor).
    
    Args:
        private_key_hex: Private key in hex
    
    Returns:
        Tuple of (address, public_key_hex)
    """
    from sdk.transactions import derive_address_from_private_key
    from Crypto.Hash import keccak
    
    # Get address using Luxtensor's method
    address = derive_address_from_private_key(private_key_hex)
    
    # Get public key
    private_key_bytes = bytes.fromhex(private_key_hex)
    signing_key = SigningKey.from_string(private_key_bytes, curve=SECP256k1)
    verifying_key = signing_key.get_verifying_key()
    
    # Uncompressed public key format (0x04 + X + Y)
    public_key_bytes = b'\x04' + verifying_key.to_string()
    public_key_hex = '0x' + public_key_bytes.hex()
    
    return address, public_key_hex


class KeyGenerator:
    """
    Key generator for ModernTensor wallets
    
    Handles mnemonic generation, key derivation, and keypair creation.
    Uses native Python cryptography matching Luxtensor's Rust implementation.
    """
    
    def generate_mnemonic(self, words: int = 12) -> str:
        """
        Generate a new BIP39 mnemonic phrase
        
        Args:
            words: Number of words (12 or 24)
        
        Returns:
            Mnemonic phrase as string
        """
        if words == 12:
            word_num = Bip39WordsNum.WORDS_NUM_12
        elif words == 24:
            word_num = Bip39WordsNum.WORDS_NUM_24
        else:
            raise ValueError("Words must be 12 or 24")
        
        mnemonic = Bip39MnemonicGenerator().FromWordsNumber(word_num)
        return str(mnemonic)
    
    def validate_mnemonic(self, mnemonic: str) -> bool:
        """
        Validate a BIP39 mnemonic phrase
        
        Args:
            mnemonic: Mnemonic phrase to validate
        
        Returns:
            True if valid, False otherwise
        """
        try:
            from bip_utils import Bip39MnemonicValidator
            return Bip39MnemonicValidator().IsValid(mnemonic)
        except Exception:
            return False
    
    def derive_hotkey(self, mnemonic: str, index: int) -> Dict[str, str]:
        """
        Derive a hotkey from mnemonic using HD derivation (matching Luxtensor).
        
        Args:
            mnemonic: BIP39 mnemonic phrase
            index: Derivation index
        
        Returns:
            Dictionary with address, public_key, and private_key
        """
        # Generate seed from mnemonic
        seed_bytes = Bip39SeedGenerator(mnemonic).Generate()
        
        # Create BIP44 context for Ethereum (compatible with Luxtensor)
        bip44_ctx = Bip44.FromSeed(seed_bytes, Bip44Coins.ETHEREUM)
        
        # Derive account: m/44'/60'/0'/0/index
        bip44_acc_ctx = bip44_ctx.Purpose().Coin().Account(0).Change(
            Bip44Changes.CHAIN_EXT
        )
        bip44_addr_ctx = bip44_acc_ctx.AddressIndex(index)
        
        # Get private key
        private_key_bytes = bip44_addr_ctx.PrivateKey().Raw().ToBytes()
        private_key_hex = private_key_bytes.hex()
        
        # Derive address and public key using Luxtensor's crypto
        address, public_key_hex = _derive_address_from_private_key(private_key_hex)
        
        return {
            'address': address,
            'public_key': public_key_hex,
            'private_key': private_key_hex
        }
    
    def generate_keypair(self) -> Dict[str, str]:
        """
        Generate a random keypair (for testing) using Luxtensor crypto.
        
        Returns:
            Dictionary with address, public_key, and private_key
        """
        # Generate random private key
        private_key = secrets.token_hex(32)
        
        # Derive address and public key using Luxtensor's crypto
        address, public_key_hex = _derive_address_from_private_key(private_key)
        
        return {
            'address': address,
            'public_key': public_key_hex,
            'private_key': private_key
        }
