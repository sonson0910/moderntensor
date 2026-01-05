"""
Testnet Deployment Tools

This module provides tools for deploying and managing testnet instances.
"""

import os
import json
import subprocess
from pathlib import Path
from typing import Dict, List, Optional
from dataclasses import dataclass


@dataclass
class DeploymentConfig:
    """Configuration for testnet deployment"""
    network_name: str = "moderntensor-testnet"
    chain_id: int = 9999
    num_validators: int = 5
    genesis_dir: Path = Path("./testnet-genesis")
    data_dir: Path = Path("./testnet-data")
    use_docker: bool = True
    docker_image: str = "moderntensor:testnet"


class TestnetDeployer:
    """
    Deploy and manage testnet instances
    """
    
    def __init__(self, config: Optional[DeploymentConfig] = None):
        self.config = config or DeploymentConfig()
    
    def prepare_genesis(self) -> Path:
        """
        Prepare genesis configuration
        
        Returns:
            Path to genesis configuration directory
        """
        from .genesis import GenesisGenerator
        
        print("🔧 Preparing genesis configuration...")
        
        # Create genesis
        generator = GenesisGenerator()
        config = generator.create_testnet_config(
            chain_id=self.config.chain_id,
            network_name=self.config.network_name,
            validator_count=self.config.num_validators
        )
        
        # Validate
        errors = generator.validate_config()
        if errors:
            print("❌ Genesis validation failed:")
            for error in errors:
                print(f"  - {error}")
            raise ValueError("Invalid genesis configuration")
        
        # Export
        self.config.genesis_dir.mkdir(parents=True, exist_ok=True)
        generator.export_config(self.config.genesis_dir)
        
        print(f"✅ Genesis configuration ready at {self.config.genesis_dir}")
        return self.config.genesis_dir
    
    def setup_directories(self):
        """Set up necessary directories for testnet"""
        print("📁 Setting up directories...")
        
        self.config.data_dir.mkdir(parents=True, exist_ok=True)
        self.config.genesis_dir.mkdir(parents=True, exist_ok=True)
        
        # Create validator directories
        for i in range(self.config.num_validators):
            validator_dir = self.config.data_dir / f"validator-{i+1}"
            validator_dir.mkdir(parents=True, exist_ok=True)
            
            # Create subdirectories
            (validator_dir / "blockchain").mkdir(exist_ok=True)
            (validator_dir / "state").mkdir(exist_ok=True)
            (validator_dir / "keystore").mkdir(exist_ok=True)
        
        print(f"✅ Directories created at {self.config.data_dir}")
    
    def generate_docker_compose(self) -> Path:
        """
        Generate docker-compose.yml for testnet
        
        Returns:
            Path to docker-compose.yml
        """
        print("🐳 Generating docker-compose configuration...")
        
        services = {}
        
        # Bootstrap node
        services['bootstrap'] = {
            'image': self.config.docker_image,
            'container_name': f'{self.config.network_name}-bootstrap',
            'command': ['bootstrap', '--config', '/genesis/genesis.json'],
            'ports': ['30303:30303'],
            'volumes': [
                f'{self.config.genesis_dir}:/genesis:ro',
                f'{self.config.data_dir}/bootstrap:/data'
            ],
            'networks': [self.config.network_name]
        }
        
        # Validators
        for i in range(self.config.num_validators):
            validator_name = f'validator-{i+1}'
            services[validator_name] = {
                'image': self.config.docker_image,
                'container_name': f'{self.config.network_name}-{validator_name}',
                'command': [
                    'node',
                    '--config', '/genesis/genesis.json',
                    '--validator',
                    '--bootstrap', 'bootstrap:30303'
                ],
                'ports': [f'{8545 + i}:8545'],  # RPC ports
                'volumes': [
                    f'{self.config.genesis_dir}:/genesis:ro',
                    f'{self.config.data_dir}/{validator_name}:/data'
                ],
                'networks': [self.config.network_name],
                'depends_on': ['bootstrap']
            }
        
        # Faucet
        services['faucet'] = {
            'image': self.config.docker_image,
            'container_name': f'{self.config.network_name}-faucet',
            'command': ['faucet', '--rpc', 'validator-1:8545'],
            'ports': ['8080:8080'],
            'networks': [self.config.network_name],
            'depends_on': ['validator-1']
        }
        
        # Explorer
        services['explorer'] = {
            'image': self.config.docker_image,
            'container_name': f'{self.config.network_name}-explorer',
            'command': ['explorer', '--rpc', 'validator-1:8545'],
            'ports': ['3000:3000'],
            'networks': [self.config.network_name],
            'depends_on': ['validator-1']
        }
        
        compose_config = {
            'version': '3.8',
            'services': services,
            'networks': {
                self.config.network_name: {
                    'driver': 'bridge'
                }
            }
        }
        
        compose_path = Path('docker-compose.testnet.yml')
        with open(compose_path, 'w') as f:
            # Use a simple YAML-like format
            import yaml
            try:
                yaml.dump(compose_config, f, default_flow_style=False)
            except ImportError:
                # If PyYAML not available, write JSON (close enough for docker-compose)
                json.dump(compose_config, f, indent=2)
        
        print(f"✅ Docker Compose configuration created: {compose_path}")
        return compose_path
    
    def deploy(self):
        """Deploy the testnet"""
        print("🚀 Deploying testnet...")
        
        # Prepare genesis
        self.prepare_genesis()
        
        # Setup directories
        self.setup_directories()
        
        if self.config.use_docker:
            # Generate docker-compose
            compose_path = self.generate_docker_compose()
            
            print("\nTo start the testnet, run:")
            print(f"  docker-compose -f {compose_path} up -d")
            print("\nTo stop the testnet, run:")
            print(f"  docker-compose -f {compose_path} down")
        else:
            print("\n⚠️  Non-Docker deployment not yet implemented")
            print("Please use Docker deployment for now")
        
        print("\n✅ Testnet deployment prepared!")
        self._print_access_info()
    
    def _print_access_info(self):
        """Print access information"""
        print("\n" + "="*60)
        print("TESTNET ACCESS INFORMATION")
        print("="*60)
        print(f"Network: {self.config.network_name}")
        print(f"Chain ID: {self.config.chain_id}")
        print(f"\nValidators:")
        for i in range(self.config.num_validators):
            print(f"  Validator {i+1} RPC: http://localhost:{8545 + i}")
        print(f"\nServices:")
        print(f"  Faucet: http://localhost:8080")
        print(f"  Explorer: http://localhost:3000")
        print(f"  Bootstrap: tcp://localhost:30303")
        print("="*60)
    
    def create_deployment_docs(self) -> Path:
        """
        Create deployment documentation
        
        Returns:
            Path to documentation file
        """
        docs = """# ModernTensor Testnet Deployment Guide

## Overview

This testnet deployment includes:
- {num_validators} validator nodes
- 1 bootstrap node for peer discovery
- 1 faucet for test tokens
- 1 blockchain explorer

## Prerequisites

- Docker and Docker Compose installed
- At least 4GB RAM available
- 20GB free disk space

## Quick Start

1. Start the testnet:
   ```bash
   docker-compose -f docker-compose.testnet.yml up -d
   ```

2. Check status:
   ```bash
   docker-compose -f docker-compose.testnet.yml ps
   ```

3. View logs:
   ```bash
   docker-compose -f docker-compose.testnet.yml logs -f validator-1
   ```

4. Stop the testnet:
   ```bash
   docker-compose -f docker-compose.testnet.yml down
   ```

## Network Information

- **Network Name:** {network_name}
- **Chain ID:** {chain_id}
- **Consensus:** Proof of Stake with AI Validation

## Endpoints

### Validators
{validator_endpoints}

### Services
- **Faucet:** http://localhost:8080
- **Explorer:** http://localhost:3000
- **Bootstrap Node:** tcp://localhost:30303

## Getting Test Tokens

Visit the faucet at http://localhost:8080 and enter your address to receive test tokens.

## Development

### Connect MetaMask

1. Add Custom Network
2. Network Name: {network_name}
3. RPC URL: http://localhost:8545
4. Chain ID: {chain_id}
5. Currency Symbol: MTN

### Using Web3

```javascript
const Web3 = require('web3');
const web3 = new Web3('http://localhost:8545');

// Get block number
const blockNumber = await web3.eth.getBlockNumber();
console.log('Current block:', blockNumber);
```

### Using ethers.js

```javascript
const {{ ethers }} = require('ethers');
const provider = new ethers.JsonRpcProvider('http://localhost:8545');

// Get block number
const blockNumber = await provider.getBlockNumber();
console.log('Current block:', blockNumber);
```

## Troubleshooting

### Containers not starting
Check Docker logs:
```bash
docker-compose -f docker-compose.testnet.yml logs
```

### Reset the testnet
Stop and remove all data:
```bash
docker-compose -f docker-compose.testnet.yml down -v
rm -rf {data_dir}
```

Then deploy again.

### Check validator health
```bash
curl http://localhost:8545 -X POST -H "Content-Type: application/json" \\
  -d '{{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}}'
```

## Support

For issues and questions:
- GitHub Issues: https://github.com/sonson0910/moderntensor
- Documentation: https://docs.moderntensor.io
"""
        
        validator_endpoints = "\n".join([
            f"- **Validator {i+1}:** http://localhost:{8545 + i}"
            for i in range(self.config.num_validators)
        ])
        
        docs = docs.format(
            num_validators=self.config.num_validators,
            network_name=self.config.network_name,
            chain_id=self.config.chain_id,
            validator_endpoints=validator_endpoints,
            data_dir=self.config.data_dir
        )
        
        docs_path = Path('TESTNET_DEPLOYMENT.md')
        with open(docs_path, 'w') as f:
            f.write(docs)
        
        print(f"📄 Deployment documentation created: {docs_path}")
        return docs_path


def deploy_testnet(
    network_name: str = "moderntensor-testnet",
    chain_id: int = 9999,
    num_validators: int = 5,
    use_docker: bool = True
):
    """
    Convenience function to deploy a testnet
    
    Args:
        network_name: Name of the network
        chain_id: Chain ID
        num_validators: Number of validators
        use_docker: Whether to use Docker
    """
    config = DeploymentConfig(
        network_name=network_name,
        chain_id=chain_id,
        num_validators=num_validators,
        use_docker=use_docker
    )
    
    deployer = TestnetDeployer(config)
    deployer.deploy()
    deployer.create_deployment_docs()
