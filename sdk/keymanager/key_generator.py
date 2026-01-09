"""
Key Generation Module

Provides functionality for generating and deriving cryptographic keys.
"""

from typing import Dict, Any
from bip_utils import (
    Bip39MnemonicGenerator, Bip39SeedGenerator, Bip39WordsNum,
    Bip44, Bip44Coins, Bip44Changes
)
from eth_account import Account
import secrets


class KeyGenerator:
    """
    Key generator for ModernTensor wallets
    
    Handles mnemonic generation, key derivation, and keypair creation.
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
        Derive a hotkey from mnemonic using HD derivation
        
        Args:
            mnemonic: BIP39 mnemonic phrase
            index: Derivation index
        
        Returns:
            Dictionary with address, public_key, and private_key
        """
        # Generate seed from mnemonic
        seed_bytes = Bip39SeedGenerator(mnemonic).Generate()
        
        # Create BIP44 context for Ethereum (could use custom coin type)
        bip44_ctx = Bip44.FromSeed(seed_bytes, Bip44Coins.ETHEREUM)
        
        # Derive account: m/44'/60'/0'/0/index
        bip44_acc_ctx = bip44_ctx.Purpose().Coin().Account(0).Change(
            Bip44Changes.CHAIN_EXT
        )
        bip44_addr_ctx = bip44_acc_ctx.AddressIndex(index)
        
        # Get private key
        private_key_bytes = bip44_addr_ctx.PrivateKey().Raw().ToBytes()
        private_key_hex = private_key_bytes.hex()
        
        # Create account from private key
        account = Account.from_key(private_key_hex)
        
        return {
            'address': account.address,
            'public_key': account._key_obj.public_key.to_hex(),
            'private_key': private_key_hex
        }
    
    def generate_keypair(self) -> Dict[str, str]:
        """
        Generate a random keypair (for testing)
        
        Returns:
            Dictionary with address, public_key, and private_key
        """
        # Generate random private key
        private_key = secrets.token_hex(32)
        
        # Create account
        account = Account.from_key(private_key)
        
        return {
            'address': account.address,
            'public_key': account._key_obj.public_key.to_hex(),
            'private_key': private_key
        }
