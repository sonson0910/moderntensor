"""
Subnet commands for ModernTensor CLI

Commands for managing subnets.
"""

import click
from typing import Optional

from sdk.cli.utils import print_warning


@click.group(name='subnet', short_help='Manage subnets')
def subnet():
    """
    Subnet commands
    
    Manage and query subnets on the network.
    """
    pass


@subnet.command('create')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--name', required=True, help='Subnet name')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
def create_subnet(coldkey: str, name: str, base_dir: Optional[str], network: str):
    """Create a new subnet"""
    print_warning("Command not yet implemented")
    # TODO: Implement create subnet


@subnet.command('register')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
def register_subnet(coldkey: str, hotkey: str, subnet_uid: int, 
                   base_dir: Optional[str], network: str):
    """Register on a subnet"""
    print_warning("Command not yet implemented")
    # TODO: Implement register on subnet


@subnet.command('info')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--network', default='testnet', help='Network name')
def subnet_info(subnet_uid: int, network: str):
    """Show subnet information"""
    print_warning("Command not yet implemented")
    # TODO: Implement subnet info


@subnet.command('participants')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--network', default='testnet', help='Network name')
def subnet_participants(subnet_uid: int, network: str):
    """List subnet participants"""
    print_warning("Command not yet implemented")
    # TODO: Implement list participants
