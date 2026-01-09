"""
Wallet management commands for ModernTensor CLI

Commands:
- create-coldkey: Create a new coldkey wallet
- restore-coldkey: Restore coldkey from mnemonic
- generate-hotkey: Generate a new hotkey
- import-hotkey: Import an encrypted hotkey
- regen-hotkey: Regenerate hotkey from derivation index
- list: List all wallets
- list-hotkeys: List hotkeys for a coldkey
- show-hotkey: Show hotkey information
- show-address: Show address information
- query-address: Query address balance and info from network
- register-hotkey: Register hotkey on the network
"""

import click
from pathlib import Path
from typing import Optional

from sdk.cli.utils import (
    print_error, print_success, print_info, print_warning,
    confirm_action, get_default_wallet_path, ensure_directory,
    prompt_password, console, create_table
)
from sdk.cli.config import get_network_config


@click.group(name='wallet', short_help='Manage wallets and keys')
@click.pass_context
def wallet(ctx):
    """
    Wallet management commands
    
    Manage coldkeys (root wallets) and hotkeys (derived keys) for the ModernTensor network.
    """
    pass


@wallet.command('create-coldkey')
@click.option('--name', required=True, help='Name for the coldkey')
@click.option('--base-dir', type=click.Path(), default=None,
              help='Base directory for wallets (default: ~/.moderntensor/wallets)')
@click.pass_context
def create_coldkey(ctx, name: str, base_dir: Optional[str]):
    """
    Create a new coldkey wallet
    
    Generates a new mnemonic phrase and encrypts it with a password.
    
    ⚠️  IMPORTANT: Save the mnemonic phrase securely! You'll need it to restore your wallet.
    
    Example:
        mtcli wallet create-coldkey --name my_coldkey
    """
    try:
        from sdk.keymanager.key_generator import KeyGenerator
        
        # Determine wallet directory
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        ensure_directory(wallet_path)
        
        coldkey_path = wallet_path / name
        
        # Check if coldkey already exists
        if coldkey_path.exists():
            print_error(f"Coldkey '{name}' already exists at {coldkey_path}")
            return
        
        print_info(f"Creating new coldkey: {name}")
        
        # Get password
        password = prompt_password("Enter password to encrypt coldkey", confirm=True)
        
        # Generate coldkey
        print_info("Generating mnemonic phrase...")
        kg = KeyGenerator()
        mnemonic = kg.generate_mnemonic()
        
        # Display mnemonic
        console.print("\n" + "="*80, style="bold yellow")
        console.print("🔑 YOUR MNEMONIC PHRASE - SAVE THIS SECURELY!", style="bold red")
        console.print("="*80 + "\n", style="bold yellow")
        console.print(mnemonic, style="bold green")
        console.print("\n" + "="*80, style="bold yellow")
        console.print("⚠️  Write this down and store it safely!", style="bold red")
        console.print("⚠️  Anyone with this phrase can access your wallet!", style="bold red")
        console.print("="*80 + "\n", style="bold yellow")
        
        # Confirm user saved mnemonic
        if not confirm_action("Have you written down your mnemonic phrase?"):
            print_warning("Coldkey creation cancelled")
            return
        
        # Encrypt and save
        print_info("Encrypting and saving coldkey...")
        ensure_directory(coldkey_path)
        
        # Save encrypted mnemonic
        from sdk.keymanager.encryption import encrypt_data
        encrypted_mnemonic = encrypt_data(mnemonic.encode(), password)
        
        with open(coldkey_path / "coldkey.enc", 'wb') as f:
            f.write(encrypted_mnemonic)
        
        # Create metadata file
        import json
        metadata = {
            'name': name,
            'type': 'coldkey',
            'created_at': str(Path(coldkey_path / "coldkey.enc").stat().st_mtime)
        }
        with open(coldkey_path / "metadata.json", 'w') as f:
            json.dump(metadata, f, indent=2)
        
        print_success(f"Coldkey '{name}' created successfully at {coldkey_path}")
        
    except Exception as e:
        print_error(f"Failed to create coldkey: {str(e)}")
        raise


@wallet.command('restore-coldkey')
@click.option('--name', required=True, help='Name for the restored coldkey')
@click.option('--base-dir', type=click.Path(), default=None,
              help='Base directory for wallets')
@click.option('--mnemonic', help='Mnemonic phrase (will prompt if not provided)')
@click.pass_context
def restore_coldkey(ctx, name: str, base_dir: Optional[str], mnemonic: Optional[str]):
    """
    Restore coldkey from mnemonic phrase
    
    Recreates a coldkey wallet from its mnemonic phrase.
    
    Example:
        mtcli wallet restore-coldkey --name restored_key
    """
    try:
        from sdk.keymanager.key_generator import KeyGenerator
        from sdk.keymanager.encryption import encrypt_data
        
        # Determine wallet directory
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        ensure_directory(wallet_path)
        
        coldkey_path = wallet_path / name
        
        # Check if coldkey already exists
        if coldkey_path.exists():
            print_error(f"Coldkey '{name}' already exists at {coldkey_path}")
            return
        
        print_info(f"Restoring coldkey: {name}")
        
        # Get mnemonic
        if not mnemonic:
            mnemonic = click.prompt("Enter your mnemonic phrase", type=str)
        
        # Validate mnemonic
        kg = KeyGenerator()
        if not kg.validate_mnemonic(mnemonic):
            print_error("Invalid mnemonic phrase")
            return
        
        # Get new password
        password = prompt_password("Enter password to encrypt restored coldkey", confirm=True)
        
        # Encrypt and save
        print_info("Encrypting and saving coldkey...")
        ensure_directory(coldkey_path)
        
        encrypted_mnemonic = encrypt_data(mnemonic.encode(), password)
        
        with open(coldkey_path / "coldkey.enc", 'wb') as f:
            f.write(encrypted_mnemonic)
        
        # Create metadata file
        import json
        metadata = {
            'name': name,
            'type': 'coldkey',
            'restored': True,
            'created_at': str(Path(coldkey_path / "coldkey.enc").stat().st_mtime)
        }
        with open(coldkey_path / "metadata.json", 'w') as f:
            json.dump(metadata, f, indent=2)
        
        print_success(f"Coldkey '{name}' restored successfully at {coldkey_path}")
        
    except Exception as e:
        print_error(f"Failed to restore coldkey: {str(e)}")
        raise


@wallet.command('list')
@click.option('--base-dir', type=click.Path(), default=None,
              help='Base directory for wallets')
@click.pass_context
def list_wallets(ctx, base_dir: Optional[str]):
    """
    List all coldkeys
    
    Displays all coldkey wallets found in the wallet directory.
    
    Example:
        mtcli wallet list
    """
    try:
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        
        if not wallet_path.exists():
            print_warning(f"Wallet directory does not exist: {wallet_path}")
            return
        
        # Find all coldkeys
        coldkeys = []
        for item in wallet_path.iterdir():
            if item.is_dir() and (item / "coldkey.enc").exists():
                coldkeys.append(item.name)
        
        if not coldkeys:
            print_warning("No coldkeys found")
            return
        
        # Display coldkeys
        table = create_table("Coldkeys", ["Name", "Path"])
        for coldkey in sorted(coldkeys):
            table.add_row(coldkey, str(wallet_path / coldkey))
        
        console.print(table)
        print_info(f"Found {len(coldkeys)} coldkey(s)")
        
    except Exception as e:
        print_error(f"Failed to list wallets: {str(e)}")
        raise


@wallet.command('generate-hotkey')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey-name', required=True, help='Name for the new hotkey')
@click.option('--base-dir', type=click.Path(), default=None,
              help='Base directory for wallets')
@click.pass_context
def generate_hotkey(ctx, coldkey: str, hotkey_name: str, base_dir: Optional[str]):
    """
    Generate a new hotkey derived from coldkey
    
    Creates a new hotkey using HD derivation from the coldkey.
    
    Example:
        mtcli wallet generate-hotkey --coldkey my_coldkey --hotkey-name miner_hk1
    """
    try:
        from sdk.keymanager.key_generator import KeyGenerator
        from sdk.keymanager.encryption import decrypt_data, encrypt_data
        
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        
        # Check coldkey exists
        if not (coldkey_path / "coldkey.enc").exists():
            print_error(f"Coldkey '{coldkey}' not found at {coldkey_path}")
            return
        
        print_info(f"Generating hotkey '{hotkey_name}' from coldkey '{coldkey}'")
        
        # Get password
        password = prompt_password("Enter coldkey password")
        
        # Load and decrypt coldkey
        with open(coldkey_path / "coldkey.enc", 'rb') as f:
            encrypted_mnemonic = f.read()
        
        try:
            mnemonic = decrypt_data(encrypted_mnemonic, password).decode()
        except Exception:
            print_error("Incorrect password")
            return
        
        # Load existing hotkeys to determine next index
        import json
        hotkeys_file = coldkey_path / "hotkeys.json"
        if hotkeys_file.exists():
            with open(hotkeys_file, 'r') as f:
                hotkeys_data = json.load(f)
        else:
            hotkeys_data = {'hotkeys': []}
        
        # Determine next derivation index
        next_index = len(hotkeys_data['hotkeys'])
        
        # Generate hotkey
        print_info(f"Generating hotkey with derivation index: {next_index}")
        kg = KeyGenerator()
        hotkey_data = kg.derive_hotkey(mnemonic, next_index)
        
        # Save hotkey
        hotkey_info = {
            'name': hotkey_name,
            'index': next_index,
            'address': hotkey_data['address'],
            'public_key': hotkey_data['public_key']
        }
        
        # Encrypt private key
        encrypted_private_key = encrypt_data(hotkey_data['private_key'].encode(), password)
        hotkey_info['encrypted_private_key'] = encrypted_private_key.hex()
        
        hotkeys_data['hotkeys'].append(hotkey_info)
        
        with open(hotkeys_file, 'w') as f:
            json.dump(hotkeys_data, f, indent=2)
        
        print_success(f"Hotkey '{hotkey_name}' generated successfully")
        print_info(f"Derivation index: {next_index}")
        print_info(f"Address: {hotkey_data['address']}")
        
    except Exception as e:
        print_error(f"Failed to generate hotkey: {str(e)}")
        raise


# Placeholder commands for other wallet operations
@wallet.command('import-hotkey')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey-name', required=True, help='Name for the imported hotkey')
@click.option('--encrypted-hotkey', required=True, help='Encrypted hotkey string')
@click.option('--base-dir', type=click.Path(), default=None)
def import_hotkey(coldkey: str, hotkey_name: str, encrypted_hotkey: str, base_dir: Optional[str]):
    """Import an encrypted hotkey"""
    print_warning("Command not yet implemented")
    # TODO: Implement hotkey import


@wallet.command('regen-hotkey')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey-name', required=True, help='Name for the regenerated hotkey')
@click.option('--index', required=True, type=int, help='Derivation index')
@click.option('--base-dir', type=click.Path(), default=None)
def regen_hotkey(coldkey: str, hotkey_name: str, index: int, base_dir: Optional[str]):
    """Regenerate hotkey from derivation index"""
    print_warning("Command not yet implemented")
    # TODO: Implement hotkey regeneration


@wallet.command('list-hotkeys')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--base-dir', type=click.Path(), default=None)
def list_hotkeys(coldkey: str, base_dir: Optional[str]):
    """List all hotkeys for a coldkey"""
    print_warning("Command not yet implemented")
    # TODO: Implement hotkey listing


@wallet.command('show-hotkey')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--base-dir', type=click.Path(), default=None)
def show_hotkey(coldkey: str, hotkey: str, base_dir: Optional[str]):
    """Show hotkey information"""
    print_warning("Command not yet implemented")
    # TODO: Implement show hotkey


@wallet.command('show-address')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
def show_address(coldkey: str, hotkey: str, base_dir: Optional[str], network: str):
    """Show address information"""
    print_warning("Command not yet implemented")
    # TODO: Implement show address


@wallet.command('query-address')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
def query_address(coldkey: str, base_dir: Optional[str], network: str):
    """Query address balance and info from network"""
    print_warning("Command not yet implemented")
    # TODO: Implement query address


@wallet.command('register-hotkey')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--initial-stake', required=True, type=int, help='Initial stake amount')
@click.option('--api-endpoint', required=True, help='Miner API endpoint')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.option('--yes', is_flag=True, help='Skip confirmation')
def register_hotkey(coldkey: str, hotkey: str, subnet_uid: int, initial_stake: int,
                   api_endpoint: str, base_dir: Optional[str], network: str, yes: bool):
    """Register hotkey as a miner on the network"""
    print_warning("Command not yet implemented")
    # TODO: Implement register hotkey
