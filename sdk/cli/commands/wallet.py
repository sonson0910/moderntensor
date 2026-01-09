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

# Constants
MDT_TO_BASE_UNITS = 1_000_000_000  # 1 MDT = 1 billion base units
DEFAULT_GAS_PRICE = 1_000_000_000  # 1 Gwei


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
@click.option('--hotkey-file', required=True, type=click.Path(exists=True), help='Path to encrypted hotkey file')
@click.option('--base-dir', type=click.Path(), default=None)
@click.pass_context
def import_hotkey(ctx, coldkey: str, hotkey_name: str, hotkey_file: str, base_dir: Optional[str]):
    """
    Import an encrypted hotkey from file
    
    Imports a previously exported hotkey file and adds it to the specified coldkey.
    The hotkey file should contain encrypted hotkey data.
    
    Example:
        mtcli wallet import-hotkey --coldkey my_coldkey --hotkey-name imported_hk --hotkey-file ./my_hotkey.enc
    """
    try:
        import json
        from sdk.keymanager.encryption import decrypt_data
        
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        
        # Check coldkey exists
        if not (coldkey_path / "coldkey.enc").exists():
            print_error(f"Coldkey '{coldkey}' not found at {coldkey_path}")
            return
        
        print_info(f"Importing hotkey '{hotkey_name}' from {hotkey_file}")
        
        # Read encrypted hotkey file
        hotkey_file_path = Path(hotkey_file)
        with open(hotkey_file_path, 'rb') as f:
            encrypted_data = f.read()
        
        # Prompt for password to decrypt
        password = prompt_password("Enter password for hotkey file")
        
        try:
            decrypted_data = decrypt_data(encrypted_data, password)
            hotkey_data = json.loads(decrypted_data.decode('utf-8'))
        except Exception as e:
            print_error(f"Failed to decrypt hotkey file: {str(e)}")
            print_info("Make sure the password is correct")
            return
        
        # Validate hotkey data structure
        required_fields = ['address', 'public_key', 'index']
        for field in required_fields:
            if field not in hotkey_data:
                print_error(f"Invalid hotkey file: missing '{field}' field")
                return
        
        # Load or create hotkeys.json
        hotkeys_file = coldkey_path / "hotkeys.json"
        if hotkeys_file.exists():
            with open(hotkeys_file, 'r') as f:
                hotkeys_data = json.load(f)
        else:
            hotkeys_data = {'hotkeys': []}
        
        # Check if hotkey name already exists
        for hk in hotkeys_data['hotkeys']:
            if hk['name'] == hotkey_name:
                print_error(f"Hotkey '{hotkey_name}' already exists in coldkey '{coldkey}'")
                if not confirm_action("Overwrite existing hotkey?"):
                    return
                # Remove existing
                hotkeys_data['hotkeys'] = [h for h in hotkeys_data['hotkeys'] if h['name'] != hotkey_name]
                break
        
        # Add imported hotkey
        new_hotkey = {
            'name': hotkey_name,
            'index': hotkey_data['index'],
            'address': hotkey_data['address'],
            'public_key': hotkey_data['public_key']
        }
        hotkeys_data['hotkeys'].append(new_hotkey)
        
        # Save updated hotkeys
        with open(hotkeys_file, 'w') as f:
            json.dump(hotkeys_data, f, indent=2)
        
        print_success(f"Hotkey '{hotkey_name}' imported successfully")
        print_info(f"Address: {hotkey_data['address']}")
        print_info(f"Derivation index: {hotkey_data['index']}")
        
    except Exception as e:
        print_error(f"Failed to import hotkey: {str(e)}")


@wallet.command('regen-hotkey')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey-name', required=True, help='Name for the regenerated hotkey')
@click.option('--index', required=True, type=int, help='Derivation index')
@click.option('--base-dir', type=click.Path(), default=None)
@click.pass_context
def regen_hotkey(ctx, coldkey: str, hotkey_name: str, index: int, base_dir: Optional[str]):
    """
    Regenerate hotkey from derivation index
    
    Derives a hotkey from the coldkey mnemonic using a specific derivation index.
    Useful for recovering lost hotkeys or generating keys at specific indices.
    
    Example:
        mtcli wallet regen-hotkey --coldkey my_coldkey --hotkey-name recovered_hk --index 5
    """
    try:
        import json
        from sdk.keymanager.key_generator import KeyGenerator
        from sdk.keymanager.encryption import decrypt_data
        
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        coldkey_file = coldkey_path / "coldkey.enc"
        
        # Check coldkey exists
        if not coldkey_file.exists():
            print_error(f"Coldkey '{coldkey}' not found at {coldkey_path}")
            return
        
        print_info(f"Regenerating hotkey '{hotkey_name}' at derivation index {index}")
        
        # Load and decrypt coldkey mnemonic
        password = prompt_password(f"Enter password for coldkey '{coldkey}'")
        
        with open(coldkey_file, 'rb') as f:
            encrypted_data = f.read()
        
        try:
            decrypted_data = decrypt_data(encrypted_data, password)
            mnemonic = decrypted_data.decode('utf-8')
        except Exception as e:
            print_error(f"Failed to decrypt coldkey: {str(e)}")
            print_info("Make sure the password is correct")
            return
        
        # Generate hotkey at specified index
        print_info(f"Deriving hotkey at index {index}...")
        kg = KeyGenerator()
        hotkey_data = kg.derive_hotkey(mnemonic, index)
        
        # Load or create hotkeys.json
        hotkeys_file = coldkey_path / "hotkeys.json"
        if hotkeys_file.exists():
            with open(hotkeys_file, 'r') as f:
                hotkeys_data = json.load(f)
        else:
            hotkeys_data = {'hotkeys': []}
        
        # Check if hotkey name already exists
        for hk in hotkeys_data['hotkeys']:
            if hk['name'] == hotkey_name:
                print_warning(f"Hotkey '{hotkey_name}' already exists")
                if not confirm_action("Overwrite existing hotkey?"):
                    return
                # Remove existing
                hotkeys_data['hotkeys'] = [h for h in hotkeys_data['hotkeys'] if h['name'] != hotkey_name]
                break
        
        # Check if index already used by another hotkey
        for hk in hotkeys_data['hotkeys']:
            if hk['index'] == index:
                print_warning(f"Derivation index {index} is already used by hotkey '{hk['name']}'")
                if not confirm_action("Continue anyway?"):
                    return
        
        # Add regenerated hotkey
        hotkey_info = {
            'name': hotkey_name,
            'index': index,
            'address': hotkey_data['address'],
            'public_key': hotkey_data['public_key']
        }
        hotkeys_data['hotkeys'].append(hotkey_info)
        
        # Save updated hotkeys
        with open(hotkeys_file, 'w') as f:
            json.dump(hotkeys_data, f, indent=2)
        
        print_success(f"Hotkey '{hotkey_name}' regenerated successfully")
        print_info(f"Derivation index: {index}")
        print_info(f"Address: {hotkey_data['address']}")
        print_info(f"Public key: {hotkey_data['public_key']}")
        
    except Exception as e:
        print_error(f"Failed to regenerate hotkey: {str(e)}")


@wallet.command('list-hotkeys')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--base-dir', type=click.Path(), default=None)
@click.pass_context
def list_hotkeys(ctx, coldkey: str, base_dir: Optional[str]):
    """
    List all hotkeys for a coldkey
    
    Displays all hotkeys associated with the specified coldkey.
    
    Example:
        mtcli wallet list-hotkeys --coldkey my_coldkey
    """
    try:
        import json
        
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        
        # Check coldkey exists
        if not (coldkey_path / "coldkey.enc").exists():
            print_error(f"Coldkey '{coldkey}' not found at {coldkey_path}")
            return
        
        # Load hotkeys
        hotkeys_file = coldkey_path / "hotkeys.json"
        if not hotkeys_file.exists():
            print_warning(f"No hotkeys found for coldkey '{coldkey}'")
            return
        
        with open(hotkeys_file, 'r') as f:
            hotkeys_data = json.load(f)
        
        hotkeys = hotkeys_data.get('hotkeys', [])
        
        if not hotkeys:
            print_warning("No hotkeys found")
            return
        
        # Display hotkeys
        table = create_table(
            f"Hotkeys for coldkey: {coldkey}",
            ["Name", "Index", "Address"]
        )
        
        for hotkey in hotkeys:
            table.add_row(
                hotkey['name'],
                str(hotkey['index']),
                hotkey['address']
            )
        
        console.print(table)
        print_info(f"Found {len(hotkeys)} hotkey(s)")
        
    except Exception as e:
        print_error(f"Failed to list hotkeys: {str(e)}")
        raise


@wallet.command('show-hotkey')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--base-dir', type=click.Path(), default=None)
@click.pass_context
def show_hotkey(ctx, coldkey: str, hotkey: str, base_dir: Optional[str]):
    """
    Show hotkey information
    
    Displays detailed information for a specific hotkey.
    
    Example:
        mtcli wallet show-hotkey --coldkey my_coldkey --hotkey miner_hk1
    """
    try:
        import json
        
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        
        # Check coldkey exists
        if not (coldkey_path / "coldkey.enc").exists():
            print_error(f"Coldkey '{coldkey}' not found")
            return
        
        # Load hotkeys
        hotkeys_file = coldkey_path / "hotkeys.json"
        if not hotkeys_file.exists():
            print_error(f"No hotkeys found for coldkey '{coldkey}'")
            return
        
        with open(hotkeys_file, 'r') as f:
            hotkeys_data = json.load(f)
        
        # Find the hotkey
        hotkey_info = None
        for hk in hotkeys_data.get('hotkeys', []):
            if hk['name'] == hotkey:
                hotkey_info = hk
                break
        
        if not hotkey_info:
            print_error(f"Hotkey '{hotkey}' not found")
            return
        
        # Display hotkey info
        from rich.panel import Panel
        
        info_text = f"""[bold cyan]Hotkey:[/bold cyan] {hotkey_info['name']}
[bold cyan]Derivation Index:[/bold cyan] {hotkey_info['index']}
[bold cyan]Address:[/bold cyan] {hotkey_info['address']}
[bold cyan]Public Key:[/bold cyan] {hotkey_info['public_key']}
[bold cyan]Coldkey:[/bold cyan] {coldkey}"""
        
        panel = Panel(info_text, title="Hotkey Information", border_style="cyan")
        console.print(panel)
        
    except Exception as e:
        print_error(f"Failed to show hotkey: {str(e)}")
        raise


@wallet.command('show-address')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.pass_context
def show_address(ctx, coldkey: str, hotkey: str, base_dir: Optional[str], network: str):
    """
    Show address information for a hotkey
    
    Displays the address derived from the coldkey/hotkey pair for the specified network.
    
    Example:
        mtcli wallet show-address --coldkey my_coldkey --hotkey miner_hk1 --network testnet
    """
    try:
        import json
        from rich.panel import Panel
        
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        
        # Check coldkey exists
        if not (coldkey_path / "coldkey.enc").exists():
            print_error(f"Coldkey '{coldkey}' not found")
            return
        
        # Load hotkeys
        hotkeys_file = coldkey_path / "hotkeys.json"
        if not hotkeys_file.exists():
            print_error(f"No hotkeys found for coldkey '{coldkey}'")
            return
        
        with open(hotkeys_file, 'r') as f:
            hotkeys_data = json.load(f)
        
        # Find the hotkey
        hotkey_info = None
        for hk in hotkeys_data.get('hotkeys', []):
            if hk['name'] == hotkey:
                hotkey_info = hk
                break
        
        if not hotkey_info:
            print_error(f"Hotkey '{hotkey}' not found")
            return
        
        # Get network config
        network_config = get_network_config(network)
        
        # Display address info
        info_text = f"""[bold cyan]Network:[/bold cyan] {network_config.name}
[bold cyan]RPC URL:[/bold cyan] {network_config.rpc_url}
[bold cyan]Chain ID:[/bold cyan] {network_config.chain_id}

[bold green]Payment Address:[/bold green] {hotkey_info['address']}
[bold green]Public Key:[/bold green] {hotkey_info['public_key']}

[bold yellow]Derivation Path:[/bold yellow] m/44'/60'/0'/0/{hotkey_info['index']}
[bold yellow]Coldkey:[/bold yellow] {coldkey}
[bold yellow]Hotkey:[/bold yellow] {hotkey}"""
        
        if network_config.explorer_url:
            info_text += f"\n\n[bold cyan]Explorer:[/bold cyan] {network_config.explorer_url}/address/{hotkey_info['address']}"
        
        panel = Panel(info_text, title="Address Information", border_style="green")
        console.print(panel)
        
    except Exception as e:
        print_error(f"Failed to show address: {str(e)}")
        raise


@wallet.command('query-address')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', help='Hotkey name (optional, queries coldkey address if not provided)')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.pass_context
def query_address(ctx, coldkey: str, hotkey: Optional[str], base_dir: Optional[str], network: str):
    """
    Query address balance and info from network
    
    Queries the blockchain for balance, nonce, and stake information
    for the address associated with the coldkey/hotkey pair.
    
    Example:
        mtcli wallet query-address --coldkey my_coldkey --network testnet
        mtcli wallet query-address --coldkey my_coldkey --hotkey miner_hk1 --network testnet
    """
    try:
        import json
        from rich.panel import Panel
        from sdk.luxtensor_client import LuxtensorClient
        
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        
        # Check coldkey exists
        if not (coldkey_path / "coldkey.enc").exists():
            print_error(f"Coldkey '{coldkey}' not found")
            return
        
        # Get address to query
        address = None
        
        if hotkey:
            # Load hotkeys
            hotkeys_file = coldkey_path / "hotkeys.json"
            if not hotkeys_file.exists():
                print_error(f"No hotkeys found for coldkey '{coldkey}'")
                return
            
            with open(hotkeys_file, 'r') as f:
                hotkeys_data = json.load(f)
            
            # Find the hotkey
            hotkey_info = None
            for hk in hotkeys_data.get('hotkeys', []):
                if hk['name'] == hotkey:
                    hotkey_info = hk
                    break
            
            if not hotkey_info:
                print_error(f"Hotkey '{hotkey}' not found")
                return
            
            address = hotkey_info['address']
            query_name = f"{coldkey}/{hotkey}"
        else:
            # For coldkey only, we would need to derive the main address
            # For now, show a message that hotkey is required
            print_warning("Querying coldkey address directly is not yet supported.")
            print_info("Please specify a hotkey with --hotkey option")
            return
        
        # Get network config
        network_config = get_network_config(network)
        
        print_info(f"Querying address {address} on {network_config.name}...")
        
        # Create client and query
        try:
            client = LuxtensorClient(network_config.rpc_url)
            
            # Get account info
            balance = client.get_balance(address)
            nonce = client.get_nonce(address)
            
            # Try to get stake info
            try:
                stake = client.get_stake(address)
            except:
                stake = 0
            
            # Format balance (assuming 9 decimals like most cryptos)
            from sdk.cli.utils import format_balance
            balance_formatted = format_balance(balance, decimals=9)
            stake_formatted = format_balance(stake, decimals=9)
            
            # Display results
            info_text = f"""[bold cyan]Address:[/bold cyan] {address}
[bold cyan]Network:[/bold cyan] {network_config.name}
[bold cyan]Wallet:[/bold cyan] {query_name}

[bold green]Balance:[/bold green] {balance_formatted} MDT ({balance} base units)
[bold green]Stake:[/bold green] {stake_formatted} MDT ({stake} base units)
[bold yellow]Nonce:[/bold yellow] {nonce}"""
            
            if network_config.explorer_url:
                info_text += f"\n\n[bold cyan]Explorer:[/bold cyan] {network_config.explorer_url}/address/{address}"
            
            panel = Panel(info_text, title="Address Query Results", border_style="green")
            console.print(panel)
            
            print_success("Query completed successfully")
            
        except Exception as e:
            print_error(f"Failed to query blockchain: {str(e)}")
            print_info(f"Make sure the RPC endpoint is accessible: {network_config.rpc_url}")
            raise
        
    except Exception as e:
        print_error(f"Failed to query address: {str(e)}")
        raise


@wallet.command('register-hotkey')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID to register on')
@click.option('--initial-stake', type=float, default=0.0, help='Initial stake amount in MDT (optional)')
@click.option('--api-endpoint', help='Miner/Validator API endpoint (optional)')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name (mainnet/testnet)')
@click.option('--yes', is_flag=True, help='Skip confirmation prompt')
@click.pass_context
def register_hotkey(ctx, coldkey: str, hotkey: str, subnet_uid: int, initial_stake: float,
                   api_endpoint: Optional[str], base_dir: Optional[str], network: str, yes: bool):
    """
    Register hotkey as a miner/validator on the network
    
    Registers the hotkey on the specified subnet, making it eligible to participate
    in consensus and earn rewards. Optionally includes initial stake and API endpoint.
    
    **Note:** Registration transaction encoding is not yet implemented. This command
    builds and signs the transaction but uses placeholder data. Full functionality
    will be available when Luxtensor registration pallet is finalized.
    
    Examples:
        # Basic registration
        mtcli wallet register-hotkey --coldkey my_coldkey --hotkey miner_hk1 --subnet-uid 1
        
        # With initial stake and API endpoint
        mtcli wallet register-hotkey \
          --coldkey my_coldkey \
          --hotkey validator_hk \
          --subnet-uid 1 \
          --initial-stake 1000 \
          --api-endpoint "http://my-server.com:8080" \
          --network testnet
    
    Note:
        - Registration requires gas fees
        - Initial stake is optional (can be added later with stake add command)
        - API endpoint is optional (required for validators)
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        from sdk.keymanager.transaction_signer import TransactionSigner
        from sdk.cli.wallet_utils import derive_hotkey_from_coldkey
        from rich.table import Table
        
        print_info(f"Registering hotkey '{hotkey}' on subnet {subnet_uid}")
        
        # Get network config
        network_config = get_network_config(network)
        rpc_url = network_config.rpc_url
        chain_id = network_config.chain_id
        
        # Initialize client
        client = LuxtensorClient(rpc_url)
        
        # Derive hotkey to get address and private key
        print_info("Loading wallet keys...")
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        from_address = hotkey_data['address']
        private_key = hotkey_data['private_key']
        
        # Check if already registered
        print_info("Checking registration status...")
        try:
            is_registered = client.is_hotkey_registered(subnet_uid, from_address)
            if is_registered:
                print_warning(f"Hotkey '{hotkey}' is already registered on subnet {subnet_uid}")
                if not yes and not confirm_action("Re-register anyway?"):
                    return
        except:
            # If method doesn't exist or fails, proceed anyway
            pass
        
        # Get current nonce
        print_info("Fetching account nonce...")
        nonce = client.get_nonce(from_address)
        
        # Estimate gas
        gas_limit = TransactionSigner.estimate_gas('register')
        gas_price = DEFAULT_GAS_PRICE
        
        # Convert initial stake to base units if provided
        stake_base = int(initial_stake * MDT_TO_BASE_UNITS) if initial_stake > 0 else 0
        
        # Build registration transaction data
        # TODO (GitHub Issue): Implement actual registration transaction encoding
        # Expected format: function_selector + subnet_uid + hotkey_address + stake + api_endpoint
        # Example structure:
        #   register_data = encode_register_call(
        #       function_selector="0xabcd1234",
        #       subnet_uid=subnet_uid,
        #       hotkey=from_address,
        #       stake=stake_base,
        #       api_endpoint=api_endpoint or ""
        #   )
        register_data = b''  # Placeholder - implement when Luxtensor registration pallet is ready
        
        # Create transaction signer
        signer = TransactionSigner(private_key)
        
        # Build and sign transaction
        print_info("Building and signing transaction...")
        signed_tx = signer.build_and_sign_transaction(
            to=from_address,  # Registration to self
            value=stake_base if stake_base > 0 else 0,
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit,
            data=register_data,
            chain_id=chain_id
        )
        
        # Display registration summary
        console.print("\n[bold yellow]Registration Summary:[/bold yellow]")
        table = Table(show_header=False, box=None)
        table.add_row("[bold]Coldkey:[/bold]", coldkey)
        table.add_row("[bold]Hotkey:[/bold]", hotkey)
        table.add_row("[bold]Address:[/bold]", from_address)
        table.add_row("[bold]Subnet UID:[/bold]", str(subnet_uid))
        table.add_row("[bold]Network:[/bold]", network)
        table.add_row("", "")
        if stake_base > 0:
            table.add_row("[bold yellow]Initial Stake:[/bold yellow]", f"{initial_stake} MDT ({stake_base} base units)")
        if api_endpoint:
            table.add_row("[bold yellow]API Endpoint:[/bold yellow]", api_endpoint)
        table.add_row("", "")
        table.add_row("[bold]Gas Limit:[/bold]", str(gas_limit))
        table.add_row("[bold]Gas Price:[/bold]", f"{gas_price} ({gas_price / 1e9} Gwei)")
        table.add_row("[bold]Est. Fee:[/bold]", f"{gas_limit * gas_price} base units")
        console.print(table)
        console.print()
        
        # Confirm before submitting
        if not yes and not confirm_action("Submit registration transaction?"):
            print_warning("Registration cancelled")
            return
        
        # Submit transaction
        print_info("Submitting transaction to network...")
        result = client.submit_transaction(signed_tx)
        
        print_success(f"Hotkey registered successfully!")
        print_info(f"Transaction hash: {result.tx_hash}")
        print_info(f"Block: {result.block_number}")
        
        if not result.success:
            print_warning(f"Transaction may have failed. Status: {result.status}")
            print_info("Check the transaction hash on the block explorer for details")
        else:
            print_success(f"Hotkey '{hotkey}' is now registered on subnet {subnet_uid}")
            if api_endpoint:
                print_info(f"API endpoint: {api_endpoint}")
            if stake_base > 0:
                print_info(f"Initial stake: {initial_stake} MDT")
        
    except FileNotFoundError as e:
        print_error(f"Wallet not found: {str(e)}")
    except KeyError as e:
        print_error(f"Hotkey not found: {str(e)}")
    except Exception as e:
        print_error(f"Failed to register hotkey: {str(e)}")
