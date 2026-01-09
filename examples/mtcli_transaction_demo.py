#!/usr/bin/env python
"""
Transaction Commands Demo

Demonstrates the usage of mtcli transaction commands using Luxtensor's native crypto.
Uses pure Python cryptography (ecdsa, keccak256) matching Luxtensor's Rust implementation.

NOTE: This is a demonstration script. In real usage, you would use the
mtcli commands directly from the command line.
"""

from sdk.transactions import LuxtensorTransaction, sign_transaction, encode_transaction_for_rpc, estimate_gas_for_transfer
from sdk.keymanager import KeyGenerator
from sdk.luxtensor_client import LuxtensorClient
from rich.console import Console
from rich.panel import Panel
from rich.table import Table

console = Console()


def demo_transaction_signing():
    """Demonstrate transaction signing capabilities"""
    console.print("\n[bold cyan]═══ Luxtensor Transaction Signing Demo ═══[/bold cyan]\n")
    
    # Create a test account
    kg = KeyGenerator()
    mnemonic = kg.generate_mnemonic()
    hotkey = kg.derive_hotkey(mnemonic, index=0)
    
    console.print(f"[yellow]Generated test account:[/yellow]")
    console.print(f"  Address: {hotkey['address']}")
    console.print(f"  (This is a demo account, not a real wallet)\n")
    
    # Build a Luxtensor transaction
    console.print("[yellow]Building Luxtensor transaction...[/yellow]")
    tx = LuxtensorTransaction(
        nonce=0,
        from_address=hotkey['address'],
        to_address='0x742D35CC6634C0532925a3b844Bc9E7595f0beB2',
        value=1_000_000_000,  # 1 MDT
        gas_price=50,
        gas_limit=21000,
        data=b''
    )
    
    # Display transaction details
    table = Table(title="Luxtensor Transaction Details", show_header=True)
    table.add_column("Field", style="cyan")
    table.add_column("Value", style="green")
    
    table.add_row("From", hotkey['address'])
    table.add_row("To", tx.to_address)
    table.add_row("Value", f"{tx.value / 1_000_000_000} MDT")
    table.add_row("Gas Price", str(tx.gas_price))
    table.add_row("Gas Limit", str(tx.gas_limit))
    table.add_row("Nonce", str(tx.nonce))
    
    console.print(table)
    
    # Sign transaction
    console.print("\n[yellow]Signing Luxtensor transaction...[/yellow]")
    signed_tx = sign_transaction(tx, hotkey['private_key'])
    
    console.print(f"[green]✓ Transaction signed successfully![/green]")
    console.print(f"  V: {signed_tx.v}")
    console.print(f"  R: {signed_tx.r.hex()[:20]}...")
    console.print(f"  S: {signed_tx.s.hex()[:20]}...")
    
    # Encode for RPC
    encoded = encode_transaction_for_rpc(signed_tx)
    console.print(f"\n[green]✓ Encoded for RPC submission![/green]")
    console.print(f"  Encoded TX (first 50 chars): {encoded[:50]}...")
    console.print(f"  Total length: {len(encoded)} characters\n")


def demo_gas_estimation():
    """Demonstrate gas estimation for different operations"""
    console.print("\n[bold cyan]═══ Gas Estimation Demo ═══[/bold cyan]\n")
    
    from sdk.transactions import calculate_transaction_fee, estimate_gas_for_transfer, estimate_gas_for_contract_call
    
    operations = [
        ('transfer', 21000, 'Simple MDT transfer'),
        ('token_transfer', 65000, 'ERC-20 style transfer'),
        ('stake', 100000, 'Staking operation'),
        ('unstake', 80000, 'Unstaking operation'),
        ('register', 150000, 'Hotkey registration'),
        ('set_weights', 200000, 'Validator weight setting'),
        ('complex', 300000, 'Complex contract call')
    ]
    
    table = Table(title="Gas Estimates", show_header=True)
    table.add_column("Operation", style="cyan")
    table.add_column("Gas Limit", justify="right", style="yellow")
    table.add_column("Cost @ 50 units", justify="right", style="green")
    table.add_column("Description", style="dim")
    
    for op, gas, desc in operations:
        cost = calculate_transaction_fee(gas, 50)
        cost_mdt = cost / 1_000_000_000
        
        table.add_row(
            op,
            f"{gas:,}",
            f"{cost_mdt:.9f} MDT",
            desc
        )
    
    console.print(table)
    console.print()


def demo_cli_commands():
    """Show example CLI commands"""
    console.print("\n[bold cyan]═══ CLI Command Examples ═══[/bold cyan]\n")
    
    commands = [
        ("Send Tokens", """mtcli tx send \\
  --coldkey my_coldkey \\
  --hotkey miner_hk1 \\
  --to 0x742D35CC6634C0532925a3b844Bc9E7595f0beB2 \\
  --amount 10.5 \\
  --network testnet"""),
        
        ("Query Status", """mtcli tx status \\
  0x1234567890abcdef... \\
  --network testnet"""),
        
        ("View History", """mtcli tx history \\
  --coldkey my_coldkey \\
  --hotkey miner_hk1 \\
  --network testnet \\
  --limit 10"""),
        
        ("Custom Gas", """mtcli tx send \\
  --coldkey my_coldkey \\
  --hotkey miner_hk1 \\
  --to 0x742D35CC6634C0532925a3b844Bc9E7595f0beB2 \\
  --amount 100 \\
  --gas-price 100 \\
  --gas-limit 30000 \\
  --network mainnet""")
    ]
    
    for title, cmd in commands:
        panel = Panel(
            cmd,
            title=f"[bold]{title}[/bold]",
            border_style="blue"
        )
        console.print(panel)
        console.print()


def demo_transaction_flow():
    """Show the complete transaction flow"""
    console.print("\n[bold cyan]═══ Complete Transaction Flow ═══[/bold cyan]\n")
    
    steps = [
        ("1. Load Wallet", "Load coldkey and hotkey from ~/.moderntensor/wallets/"),
        ("2. Decrypt Keys", "Prompt for password and decrypt coldkey"),
        ("3. Derive Hotkey", "Use BIP44 to derive hotkey private key"),
        ("4. Query Nonce", "Get current nonce from blockchain via RPC"),
        ("5. Check Balance", "Verify sufficient balance for transaction"),
        ("6. Build TX", "Construct transaction with user parameters"),
        ("7. Sign TX", "Sign transaction with private key (eth-account)"),
        ("8. Submit", "Broadcast signed transaction to network"),
        ("9. Confirm", "Display transaction hash and status"),
        ("10. Monitor", "Track transaction confirmation (optional)")
    ]
    
    table = Table(title="Transaction Flow Steps", show_header=True)
    table.add_column("Step", style="cyan", width=20)
    table.add_column("Description", style="white")
    
    for step, desc in steps:
        table.add_row(step, desc)
    
    console.print(table)
    console.print()


def demo_security_features():
    """Highlight security features"""
    console.print("\n[bold cyan]═══ Security Features ═══[/bold cyan]\n")
    
    features = [
        ("✓", "Password-protected coldkeys", "PBKDF2 + Fernet encryption"),
        ("✓", "BIP44 HD derivation", "Standard key derivation path"),
        ("✓", "EIP-55 checksum addresses", "Prevents address typos"),
        ("✓", "Private keys never logged", "Security by design"),
        ("✓", "Interactive confirmations", "User must approve transactions"),
        ("✓", "Balance verification", "Prevents insufficient fund errors"),
        ("✓", "Gas cost transparency", "User sees total cost before sending")
    ]
    
    table = Table(title="Security & Safety", show_header=True, border_style="green")
    table.add_column("Status", style="green", width=6)
    table.add_column("Feature", style="cyan", width=30)
    table.add_column("Details", style="dim")
    
    for status, feature, details in features:
        table.add_row(status, feature, details)
    
    console.print(table)
    console.print()


def main():
    """Run all demos"""
    console.print(Panel(
        "[bold]ModernTensor CLI - Transaction Commands Demo[/bold]\n\n"
        "This demo showcases the transaction functionality implemented in Phase 3.",
        title="mtcli Transaction Demo",
        border_style="bold magenta"
    ))
    
    # Run demos
    demo_transaction_signing()
    demo_gas_estimation()
    demo_cli_commands()
    demo_transaction_flow()
    demo_security_features()
    
    # Summary
    console.print(Panel(
        "[bold green]Phase 3 Complete! ✅[/bold green]\n\n"
        "All transaction commands are now functional:\n"
        "• tx send - Send MDT tokens\n"
        "• tx status - Query transaction status\n"
        "• tx history - View transaction history\n\n"
        "[dim]For actual usage, use the mtcli commands from your terminal.[/dim]",
        title="Summary",
        border_style="bold green"
    ))


if __name__ == '__main__':
    main()
