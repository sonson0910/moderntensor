"""
Genesis Block Configuration and Generation

This module handles the creation and management of genesis blocks for testnet deployment.
"""

import json
import time
from dataclasses import dataclass, field, asdict
from typing import List, Dict, Optional, Any
from datetime import datetime, timezone
from pathlib import Path


@dataclass
class ValidatorConfig:
    """Configuration for a genesis validator"""
    address: str
    stake: int
    public_key: str
    name: Optional[str] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        data = {
            'address': self.address,
            'stake': self.stake,
            'public_key': self.public_key
        }
        if self.name:
            data['name'] = self.name
        return data


@dataclass
class AccountConfig:
    """Configuration for a genesis account"""
    address: str
    balance: int
    nonce: int = 0
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return {
            'address': self.address,
            'balance': self.balance,
            'nonce': self.nonce
        }


@dataclass
class ConsensusConfig:
    """Consensus mechanism configuration"""
    type: str = 'pos'
    epoch_length: int = 100  # blocks per epoch
    slot_duration: int = 12  # seconds per slot
    validator_count: int = 21  # target validator count
    min_stake: int = 1000000  # minimum stake to become validator
    slash_percentage: int = 10  # percentage of stake slashed for violations
    reward_percentage: int = 5  # annual reward percentage
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return asdict(self)


@dataclass
class NetworkConfig:
    """Network configuration"""
    chain_id: int
    network_name: str
    p2p_port: int = 30303
    rpc_port: int = 8545
    ws_port: int = 8546
    max_peers: int = 50
    bootstrap_nodes: List[str] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return asdict(self)


@dataclass
class GenesisConfig:
    """
    Complete genesis configuration for testnet
    
    This includes all necessary parameters to initialize a new blockchain network.
    """
    # Network
    chain_id: int
    network_name: str
    genesis_time: str
    
    # Consensus
    consensus: ConsensusConfig
    
    # Network settings
    network: NetworkConfig
    
    # Initial state
    initial_validators: List[ValidatorConfig] = field(default_factory=list)
    initial_accounts: List[AccountConfig] = field(default_factory=list)
    
    # Token economics
    total_supply: int = 1_000_000_000  # 1 billion tokens
    decimals: int = 18
    
    # Block parameters
    block_gas_limit: int = 30_000_000
    min_gas_price: int = 1_000_000_000  # 1 gwei
    
    # Extra metadata
    extra_data: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert genesis config to dictionary"""
        return {
            'chain_id': self.chain_id,
            'network_name': self.network_name,
            'genesis_time': self.genesis_time,
            'consensus': self.consensus.to_dict(),
            'network': self.network.to_dict(),
            'initial_validators': [v.to_dict() for v in self.initial_validators],
            'initial_accounts': [a.to_dict() for a in self.initial_accounts],
            'total_supply': self.total_supply,
            'decimals': self.decimals,
            'block_gas_limit': self.block_gas_limit,
            'min_gas_price': self.min_gas_price,
            'extra_data': self.extra_data
        }
    
    def to_json(self, indent: int = 2) -> str:
        """Convert to JSON string"""
        return json.dumps(self.to_dict(), indent=indent)
    
    def save(self, filepath: Path):
        """Save genesis config to file"""
        with open(filepath, 'w') as f:
            f.write(self.to_json())
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'GenesisConfig':
        """Create genesis config from dictionary"""
        consensus = ConsensusConfig(**data['consensus'])
        network = NetworkConfig(**data['network'])
        
        validators = [ValidatorConfig(**v) for v in data.get('initial_validators', [])]
        accounts = [AccountConfig(**a) for a in data.get('initial_accounts', [])]
        
        return cls(
            chain_id=data['chain_id'],
            network_name=data['network_name'],
            genesis_time=data['genesis_time'],
            consensus=consensus,
            network=network,
            initial_validators=validators,
            initial_accounts=accounts,
            total_supply=data.get('total_supply', 1_000_000_000),
            decimals=data.get('decimals', 18),
            block_gas_limit=data.get('block_gas_limit', 30_000_000),
            min_gas_price=data.get('min_gas_price', 1_000_000_000),
            extra_data=data.get('extra_data', {})
        )
    
    @classmethod
    def load(cls, filepath: Path) -> 'GenesisConfig':
        """Load genesis config from file"""
        with open(filepath, 'r') as f:
            data = json.load(f)
        return cls.from_dict(data)


class GenesisGenerator:
    """
    Generate genesis blocks and configurations for testnet deployment
    """
    
    def __init__(self):
        self.config: Optional[GenesisConfig] = None
    
    def create_testnet_config(
        self,
        chain_id: int = 9999,
        network_name: str = "moderntensor-testnet",
        validator_count: int = 5,
        validator_stake: int = 10_000_000,
        faucet_balance: int = 1_000_000_000_000
    ) -> GenesisConfig:
        """
        Create a default testnet configuration
        
        Args:
            chain_id: Unique chain identifier
            network_name: Name of the network
            validator_count: Number of genesis validators
            validator_stake: Stake amount per validator
            faucet_balance: Initial balance for faucet account
        
        Returns:
            GenesisConfig: Complete genesis configuration
        """
        genesis_time = datetime.now(timezone.utc).isoformat()
        
        # Create consensus config
        consensus = ConsensusConfig(
            type='pos',
            epoch_length=100,
            slot_duration=12,
            validator_count=validator_count,
            min_stake=1_000_000
        )
        
        # Create network config
        network = NetworkConfig(
            chain_id=chain_id,
            network_name=network_name,
            p2p_port=30303,
            rpc_port=8545,
            ws_port=8546,
            max_peers=50
        )
        
        # Generate genesis validators
        validators = []
        for i in range(validator_count):
            validators.append(ValidatorConfig(
                address=f"0x{'0' * 38}{i+1:02d}",  # Dummy addresses for testnet
                stake=validator_stake,
                public_key=f"0x{'0' * 126}{i+1:02d}",
                name=f"Validator-{i+1}"
            ))
        
        # Create faucet account
        accounts = [
            AccountConfig(
                address="0xfacf00000000000000000000000000000000acef",
                balance=faucet_balance,
                nonce=0
            )
        ]
        
        self.config = GenesisConfig(
            chain_id=chain_id,
            network_name=network_name,
            genesis_time=genesis_time,
            consensus=consensus,
            network=network,
            initial_validators=validators,
            initial_accounts=accounts,
            extra_data={
                'description': 'ModernTensor Testnet Genesis Block',
                'version': '1.0.0',
                'created_at': int(time.time())
            }
        )
        
        return self.config
    
    def add_validator(
        self,
        address: str,
        stake: int,
        public_key: str,
        name: Optional[str] = None
    ):
        """Add a validator to genesis config"""
        if not self.config:
            raise ValueError("Genesis config not initialized. Call create_testnet_config first.")
        
        validator = ValidatorConfig(
            address=address,
            stake=stake,
            public_key=public_key,
            name=name
        )
        self.config.initial_validators.append(validator)
    
    def add_account(self, address: str, balance: int, nonce: int = 0):
        """Add an account to genesis config"""
        if not self.config:
            raise ValueError("Genesis config not initialized. Call create_testnet_config first.")
        
        account = AccountConfig(
            address=address,
            balance=balance,
            nonce=nonce
        )
        self.config.initial_accounts.append(account)
    
    def generate_genesis_block(self) -> Dict[str, Any]:
        """
        Generate the actual genesis block from configuration
        
        Returns:
            Dict containing genesis block data
        """
        if not self.config:
            raise ValueError("Genesis config not initialized. Call create_testnet_config first.")
        
        # Calculate initial state root
        # In a real implementation, this would compute the Merkle root
        state_root = "0x" + "0" * 64
        
        genesis_block = {
            'header': {
                'version': 1,
                'height': 0,
                'timestamp': int(datetime.fromisoformat(self.config.genesis_time).timestamp()),
                'previous_hash': "0x" + "0" * 64,
                'state_root': state_root,
                'txs_root': "0x" + "0" * 64,
                'receipts_root': "0x" + "0" * 64,
                'validator': "0x" + "0" * 64,  # Genesis has no validator
                'signature': "0x" + "0" * 128,
                'gas_used': 0,
                'gas_limit': self.config.block_gas_limit,
                'extra_data': json.dumps(self.config.extra_data)
            },
            'transactions': [],
            'config': self.config.to_dict()
        }
        
        return genesis_block
    
    def export_config(self, output_dir: Path):
        """
        Export genesis configuration to files
        
        Args:
            output_dir: Directory to save configuration files
        """
        if not self.config:
            raise ValueError("Genesis config not initialized. Call create_testnet_config first.")
        
        output_dir = Path(output_dir)
        output_dir.mkdir(parents=True, exist_ok=True)
        
        # Save genesis config
        config_path = output_dir / 'genesis.json'
        self.config.save(config_path)
        
        # Save genesis block
        block_path = output_dir / 'genesis_block.json'
        genesis_block = self.generate_genesis_block()
        with open(block_path, 'w') as f:
            json.dump(genesis_block, f, indent=2)
        
        # Save validator info
        validators_path = output_dir / 'validators.json'
        validators_data = {
            'validators': [v.to_dict() for v in self.config.initial_validators]
        }
        with open(validators_path, 'w') as f:
            json.dump(validators_data, f, indent=2)
        
        print(f"✅ Genesis configuration exported to {output_dir}")
        print(f"  - {config_path}")
        print(f"  - {block_path}")
        print(f"  - {validators_path}")
    
    def validate_config(self) -> List[str]:
        """
        Validate genesis configuration
        
        Returns:
            List of validation errors (empty if valid)
        """
        if not self.config:
            return ["Genesis config not initialized"]
        
        errors = []
        
        # Check validators
        if len(self.config.initial_validators) < 1:
            errors.append("At least one validator required")
        
        # Check total stake
        total_stake = sum(v.stake for v in self.config.initial_validators)
        if total_stake > self.config.total_supply:
            errors.append(f"Total validator stake ({total_stake}) exceeds total supply ({self.config.total_supply})")
        
        # Check accounts
        total_balance = sum(a.balance for a in self.config.initial_accounts)
        if total_balance > self.config.total_supply:
            errors.append(f"Total account balance ({total_balance}) exceeds total supply ({self.config.total_supply})")
        
        # Check addresses are unique
        all_addresses = [v.address for v in self.config.initial_validators]
        all_addresses.extend([a.address for a in self.config.initial_accounts])
        if len(all_addresses) != len(set(all_addresses)):
            errors.append("Duplicate addresses found")
        
        return errors


def create_default_testnet_genesis(output_dir: Path) -> GenesisConfig:
    """
    Convenience function to create a default testnet genesis configuration
    
    Args:
        output_dir: Directory to export configuration files
    
    Returns:
        GenesisConfig: The created configuration
    """
    generator = GenesisGenerator()
    config = generator.create_testnet_config()
    
    # Validate
    errors = generator.validate_config()
    if errors:
        print("⚠️ Validation warnings:")
        for error in errors:
            print(f"  - {error}")
    else:
        print("✅ Genesis configuration is valid")
    
    # Export
    generator.export_config(output_dir)
    
    return config
