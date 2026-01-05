"""
Tests for testnet module

Tests for genesis, faucet, bootstrap, monitoring, and deployment functionality.
"""

import pytest
import asyncio
import time
from pathlib import Path
import tempfile
import shutil

from sdk.testnet import (
    GenesisConfig,
    GenesisGenerator,
    Faucet,
    FaucetConfig,
    BootstrapNode,
    BootstrapConfig
)
from sdk.testnet.genesis import (
    ValidatorConfig,
    AccountConfig,
    ConsensusConfig,
    NetworkConfig
)
from sdk.testnet.monitoring import (
    TestnetMonitor,
    NodeHealth,
    NetworkMetrics,
    TestnetExplorer
)
from sdk.testnet.deployment import TestnetDeployer, DeploymentConfig


class TestGenesis:
    """Tests for genesis configuration"""
    
    def test_validator_config(self):
        """Test validator configuration"""
        validator = ValidatorConfig(
            address="0x1234567890123456789012345678901234567890",
            stake=1000000,
            public_key="0xabcd",
            name="Test Validator"
        )
        
        assert validator.address.startswith("0x")
        assert validator.stake == 1000000
        assert validator.name == "Test Validator"
        
        data = validator.to_dict()
        assert data['address'] == validator.address
        assert 'name' in data
    
    def test_account_config(self):
        """Test account configuration"""
        account = AccountConfig(
            address="0x1234567890123456789012345678901234567890",
            balance=1000000000,
            nonce=0
        )
        
        assert account.balance == 1000000000
        assert account.nonce == 0
        
        data = account.to_dict()
        assert data['balance'] == account.balance
    
    def test_genesis_config_creation(self):
        """Test genesis config creation"""
        consensus = ConsensusConfig()
        network = NetworkConfig(chain_id=9999, network_name="test")
        
        config = GenesisConfig(
            chain_id=9999,
            network_name="test",
            genesis_time="2026-01-05T00:00:00Z",
            consensus=consensus,
            network=network
        )
        
        assert config.chain_id == 9999
        assert config.network_name == "test"
        assert config.total_supply == 1_000_000_000
        
        # Test serialization
        data = config.to_dict()
        assert 'chain_id' in data
        assert 'consensus' in data
        assert 'network' in data
        
        # Test JSON conversion
        json_str = config.to_json()
        assert '"chain_id": 9999' in json_str
    
    def test_genesis_generator(self):
        """Test genesis generator"""
        generator = GenesisGenerator()
        config = generator.create_testnet_config(
            chain_id=9999,
            network_name="test-network",
            validator_count=3
        )
        
        assert config.chain_id == 9999
        assert config.network_name == "test-network"
        assert len(config.initial_validators) == 3
        assert len(config.initial_accounts) >= 1  # At least faucet
        
        # Test validation
        errors = generator.validate_config()
        assert isinstance(errors, list)
    
    def test_add_validator(self):
        """Test adding validators"""
        generator = GenesisGenerator()
        generator.create_testnet_config(validator_count=2)
        
        initial_count = len(generator.config.initial_validators)
        
        generator.add_validator(
            address="0xtest",
            stake=1000000,
            public_key="0xpubkey",
            name="New Validator"
        )
        
        assert len(generator.config.initial_validators) == initial_count + 1
    
    def test_genesis_block_generation(self):
        """Test genesis block generation"""
        generator = GenesisGenerator()
        generator.create_testnet_config()
        
        genesis_block = generator.generate_genesis_block()
        
        assert 'header' in genesis_block
        assert 'transactions' in genesis_block
        assert 'config' in genesis_block
        assert genesis_block['header']['height'] == 0
        assert len(genesis_block['transactions']) == 0
    
    def test_genesis_export(self):
        """Test exporting genesis configuration"""
        generator = GenesisGenerator()
        generator.create_testnet_config()
        
        with tempfile.TemporaryDirectory() as tmpdir:
            generator.export_config(Path(tmpdir))
            
            # Check files were created
            assert (Path(tmpdir) / 'genesis.json').exists()
            assert (Path(tmpdir) / 'genesis_block.json').exists()
            assert (Path(tmpdir) / 'validators.json').exists()


class TestFaucet:
    """Tests for faucet functionality"""
    
    def test_faucet_config(self):
        """Test faucet configuration"""
        config = FaucetConfig(
            tokens_per_request=100_000_000_000,
            cooldown_period=3600,
            max_requests_per_address=3
        )
        
        assert config.tokens_per_request == 100_000_000_000
        assert config.cooldown_period == 3600
    
    def test_faucet_initialization(self):
        """Test faucet initialization"""
        faucet = Faucet()
        
        assert faucet.config is not None
        assert faucet.total_distributed == 0
        assert faucet.request_count == 0
        assert len(faucet.blocked_addresses) == 0
    
    @pytest.mark.asyncio
    async def test_request_tokens(self):
        """Test requesting tokens"""
        faucet = Faucet()
        
        result = await faucet.request_tokens("0x1234567890123456789012345678901234567890")
        
        assert result['success'] is True
        assert 'tx_hash' in result
        assert result['amount'] == faucet.config.tokens_per_request
        assert faucet.request_count == 1
    
    @pytest.mark.asyncio
    async def test_invalid_address(self):
        """Test requesting with invalid address"""
        faucet = Faucet()
        
        result = await faucet.request_tokens("invalid_address")
        
        assert result['success'] is False
        assert 'error' in result
    
    @pytest.mark.asyncio
    async def test_rate_limiting(self):
        """Test rate limiting"""
        config = FaucetConfig(
            max_requests_per_address=2,
            cooldown_period=3600
        )
        faucet = Faucet(config)
        
        address = "0x1234567890123456789012345678901234567890"
        
        # First request - should succeed
        result1 = await faucet.request_tokens(address)
        assert result1['success'] is True
        
        # Second request - should succeed
        result2 = await faucet.request_tokens(address)
        assert result2['success'] is True
        
        # Third request - should fail (exceeded limit)
        result3 = await faucet.request_tokens(address)
        assert result3['success'] is False
        assert 'Too many requests' in result3['error']
    
    @pytest.mark.asyncio
    async def test_ip_rate_limiting(self):
        """Test IP-based rate limiting"""
        config = FaucetConfig(max_requests_per_ip=2)
        faucet = Faucet(config)
        
        ip = "192.168.1.1"
        
        # Request from different addresses, same IP
        result1 = await faucet.request_tokens("0x" + "1" * 40, ip)
        assert result1['success'] is True
        
        result2 = await faucet.request_tokens("0x" + "2" * 40, ip)
        assert result2['success'] is True
        
        # Third request from same IP should fail
        result3 = await faucet.request_tokens("0x" + "3" * 40, ip)
        assert result3['success'] is False
    
    def test_block_address(self):
        """Test blocking addresses"""
        faucet = Faucet()
        address = "0x1234567890123456789012345678901234567890"
        
        faucet.block_address(address)
        assert address in faucet.blocked_addresses
        
        can_request, reason = faucet.can_request(address)
        assert can_request is False
        assert "blocked" in reason.lower()
        
        faucet.unblock_address(address)
        assert address not in faucet.blocked_addresses
    
    def test_get_stats(self):
        """Test getting statistics"""
        faucet = Faucet()
        stats = faucet.get_stats()
        
        assert 'total_requests' in stats
        assert 'successful_requests' in stats
        assert 'rejected_requests' in stats
        assert 'total_tokens_distributed' in stats
    
    def test_get_address_info(self):
        """Test getting address information"""
        faucet = Faucet()
        address = "0x1234567890123456789012345678901234567890"
        
        info = faucet.get_address_info(address)
        
        assert info['address'] == address
        assert info['total_requests'] == 0
        assert info['can_request'] is True


class TestBootstrapNode:
    """Tests for bootstrap node"""
    
    def test_bootstrap_config(self):
        """Test bootstrap configuration"""
        config = BootstrapConfig(
            listen_port=30303,
            max_peers=100,
            chain_id=9999
        )
        
        assert config.listen_port == 30303
        assert config.max_peers == 100
    
    def test_bootstrap_initialization(self):
        """Test bootstrap node initialization"""
        node = BootstrapNode()
        
        assert node.config is not None
        assert len(node.peers) == 0
        assert node.running is False
    
    def test_register_peer(self):
        """Test registering peers"""
        node = BootstrapNode()
        
        success = node.register_peer(
            node_id="node1",
            address="192.168.1.1",
            port=30303
        )
        
        assert success is True
        assert "node1" in node.peers
        assert node.stats['total_peers_seen'] == 1
    
    def test_get_peers(self):
        """Test getting peer list"""
        node = BootstrapNode()
        
        # Register multiple peers
        for i in range(5):
            node.register_peer(
                node_id=f"node{i}",
                address=f"192.168.1.{i}",
                port=30303
            )
        
        # Get peers
        peers = node.get_peers(max_count=3)
        
        assert len(peers) <= 3
    
    def test_peer_exclusion(self):
        """Test excluding peers from discovery"""
        node = BootstrapNode()
        
        # Register peers
        node.register_peer("node1", "192.168.1.1", 30303)
        node.register_peer("node2", "192.168.1.2", 30303)
        node.register_peer("node3", "192.168.1.3", 30303)
        
        # Get peers excluding node1
        peers = node.get_peers(exclude={"node1"})
        
        peer_ids = [p.node_id for p in peers]
        assert "node1" not in peer_ids
    
    def test_remove_peer(self):
        """Test removing peers"""
        node = BootstrapNode()
        
        node.register_peer("node1", "192.168.1.1", 30303)
        assert "node1" in node.peers
        
        node.remove_peer("node1")
        assert "node1" not in node.peers
    
    def test_get_stats(self):
        """Test getting statistics"""
        node = BootstrapNode()
        
        node.register_peer("node1", "192.168.1.1", 30303)
        
        stats = node.get_stats()
        
        assert 'total_peers_seen' in stats
        assert 'active_peers' in stats
        assert 'uptime_seconds' in stats


class TestMonitoring:
    """Tests for monitoring functionality"""
    
    def test_node_health(self):
        """Test node health status"""
        health = NodeHealth(
            node_id="node1",
            status="healthy",
            last_block_height=100,
            last_block_time=time.time(),
            peer_count=10,
            sync_status="synced"
        )
        
        assert health.status == "healthy"
        assert health.last_block_height == 100
        
        data = health.to_dict()
        assert data['node_id'] == "node1"
    
    def test_network_metrics(self):
        """Test network metrics"""
        metrics = NetworkMetrics(
            total_nodes=5,
            healthy_nodes=4,
            total_validators=3,
            active_validators=3,
            current_height=1000,
            avg_block_time=12.5,
            total_transactions=5000,
            tps=100.0
        )
        
        assert metrics.total_nodes == 5
        assert metrics.tps == 100.0
        
        data = metrics.to_dict()
        assert 'total_nodes' in data
    
    def test_testnet_monitor(self):
        """Test testnet monitor"""
        monitor = TestnetMonitor()
        
        # Add node health
        health = NodeHealth(
            node_id="node1",
            status="healthy",
            last_block_height=100,
            last_block_time=time.time(),
            peer_count=5,
            sync_status="synced"
        )
        monitor.update_node_health(health)
        
        assert "node1" in monitor.nodes
        
        # Get node health
        retrieved = monitor.get_node_health("node1")
        assert retrieved is not None
        assert retrieved.node_id == "node1"
    
    def test_network_metrics_calculation(self):
        """Test calculating network metrics"""
        monitor = TestnetMonitor()
        
        # Add some nodes
        for i in range(3):
            health = NodeHealth(
                node_id=f"node{i}",
                status="healthy" if i < 2 else "degraded",
                last_block_height=100 + i,
                last_block_time=time.time(),
                peer_count=5,
                sync_status="synced"
            )
            monitor.update_node_health(health)
        
        metrics = monitor.calculate_network_metrics()
        
        assert metrics.total_nodes == 3
        assert metrics.healthy_nodes == 2
        assert metrics.current_height == 102
    
    def test_testnet_explorer(self):
        """Test testnet explorer"""
        explorer = TestnetExplorer()
        
        # Add a block
        block = {
            'height': 1,
            'hash': '0xabc',
            'timestamp': time.time()
        }
        explorer.add_block(block)
        
        assert len(explorer.blocks) == 1
        
        # Get latest blocks
        latest = explorer.get_latest_blocks(count=1)
        assert len(latest) == 1
        assert latest[0]['height'] == 1
        
        # Get block by height
        retrieved = explorer.get_block(1)
        assert retrieved is not None
        assert retrieved['height'] == 1


class TestDeployment:
    """Tests for deployment functionality"""
    
    def test_deployment_config(self):
        """Test deployment configuration"""
        config = DeploymentConfig(
            network_name="test-net",
            chain_id=9999,
            num_validators=3
        )
        
        assert config.network_name == "test-net"
        assert config.num_validators == 3
    
    def test_testnet_deployer(self):
        """Test testnet deployer"""
        with tempfile.TemporaryDirectory() as tmpdir:
            config = DeploymentConfig(
                network_name="test-net",
                chain_id=9999,
                num_validators=2,
                genesis_dir=Path(tmpdir) / "genesis",
                data_dir=Path(tmpdir) / "data"
            )
            
            deployer = TestnetDeployer(config)
            
            # Test genesis preparation
            genesis_dir = deployer.prepare_genesis()
            assert genesis_dir.exists()
            assert (genesis_dir / 'genesis.json').exists()
            
            # Test directory setup
            deployer.setup_directories()
            assert config.data_dir.exists()
            assert (config.data_dir / 'validator-1').exists()


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
