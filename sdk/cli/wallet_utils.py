"""
Wallet Utilities Module

Provides helper functions for loading and managing wallet keys in CLI commands.
"""

import json
from pathlib import Path
from typing import Dict, Optional

from sdk.cli.utils import get_default_wallet_path, prompt_password
from sdk.keymanager.encryption import decrypt_data
from sdk.keymanager.key_generator import KeyGenerator


def load_coldkey_mnemonic(coldkey_name: str, base_dir: Optional[str] = None) -> str:
    """
    Load and decrypt coldkey mnemonic
    
    Args:
        coldkey_name: Name of the coldkey
        base_dir: Optional base directory for wallets
    
    Returns:
        Decrypted mnemonic phrase
    
    Raises:
        Exception: If coldkey not found or decryption fails
    """
    wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
    coldkey_path = wallet_path / coldkey_name
    coldkey_file = coldkey_path / "coldkey.enc"
    
    if not coldkey_file.exists():
        raise FileNotFoundError(f"Coldkey '{coldkey_name}' not found at {coldkey_path}")
    
    # Prompt for password
    password = prompt_password(f"Enter password for coldkey '{coldkey_name}'")
    
    # Load and decrypt
    with open(coldkey_file, 'rb') as f:
        encrypted_data = f.read()
    
    try:
        decrypted_data = decrypt_data(encrypted_data, password)
        return decrypted_data.decode('utf-8')
    except Exception as e:
        raise Exception(f"Failed to decrypt coldkey: {str(e)}")


def load_hotkey_info(
    coldkey_name: str,
    hotkey_name: str,
    base_dir: Optional[str] = None
) -> Dict[str, str]:
    """
    Load hotkey information
    
    Args:
        coldkey_name: Name of the coldkey
        hotkey_name: Name of the hotkey
        base_dir: Optional base directory for wallets
    
    Returns:
        Dictionary with hotkey information (address, index)
    
    Raises:
        Exception: If hotkey not found
    """
    wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
    coldkey_path = wallet_path / coldkey_name
    hotkeys_file = coldkey_path / "hotkeys.json"
    
    if not hotkeys_file.exists():
        raise FileNotFoundError(f"No hotkeys found for coldkey '{coldkey_name}'")
    
    # Load hotkeys
    with open(hotkeys_file, 'r') as f:
        hotkeys = json.load(f)
    
    if hotkey_name not in hotkeys:
        raise KeyError(f"Hotkey '{hotkey_name}' not found in coldkey '{coldkey_name}'")
    
    return hotkeys[hotkey_name]


def derive_hotkey_from_coldkey(
    coldkey_name: str,
    hotkey_name: str,
    base_dir: Optional[str] = None
) -> Dict[str, str]:
    """
    Derive hotkey private key from coldkey mnemonic
    
    Args:
        coldkey_name: Name of the coldkey
        hotkey_name: Name of the hotkey
        base_dir: Optional base directory for wallets
    
    Returns:
        Dictionary with address, public_key, and private_key
    
    Raises:
        Exception: If keys not found or derivation fails
    """
    # Load coldkey mnemonic
    mnemonic = load_coldkey_mnemonic(coldkey_name, base_dir)
    
    # Load hotkey info to get derivation index
    hotkey_info = load_hotkey_info(coldkey_name, hotkey_name, base_dir)
    index = hotkey_info['index']
    
    # Derive hotkey
    kg = KeyGenerator()
    hotkey = kg.derive_hotkey(mnemonic, index)
    
    return hotkey


def get_hotkey_address(
    coldkey_name: str,
    hotkey_name: str,
    base_dir: Optional[str] = None
) -> str:
    """
    Get hotkey address without loading private key
    
    Args:
        coldkey_name: Name of the coldkey
        hotkey_name: Name of the hotkey
        base_dir: Optional base directory for wallets
    
    Returns:
        Hotkey address
    
    Raises:
        Exception: If hotkey not found
    """
    hotkey_info = load_hotkey_info(coldkey_name, hotkey_name, base_dir)
    return hotkey_info['address']
