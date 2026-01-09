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
        FileNotFoundError: If coldkey file not found
        ValueError: If decryption fails (wrong password or corrupted data)
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
        raise ValueError(f"Failed to decrypt coldkey (wrong password or corrupted data): {str(e)}")


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
        Dictionary with hotkey information (name, address, index, public_key)
    
    Raises:
        FileNotFoundError: If hotkeys.json not found
        KeyError: If specified hotkey name not found in the file
    """
    wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
    coldkey_path = wallet_path / coldkey_name
    hotkeys_file = coldkey_path / "hotkeys.json"
    
    if not hotkeys_file.exists():
        raise FileNotFoundError(f"No hotkeys found for coldkey '{coldkey_name}'")
    
    # Load hotkeys
    with open(hotkeys_file, 'r') as f:
        hotkeys_data = json.load(f)
    
    hotkeys_list = hotkeys_data.get('hotkeys', [])
    
    # Find the hotkey by name
    for hotkey in hotkeys_list:
        if hotkey.get('name') == hotkey_name:
            return hotkey
    
    # If not found, raise error
    raise KeyError(f"Hotkey '{hotkey_name}' not found in coldkey '{coldkey_name}'")


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
