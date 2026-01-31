# Luxtensor Troubleshooting Guide

Common issues and solutions for Luxtensor node operators.

---

## Node Issues

### Node Won't Start

**Symptoms**: Node exits immediately or fails to start.

**Solutions**:

1. Check config file syntax:

   ```bash
   luxtensor-node --config config.toml --validate
   ```

2. Verify ports are available:

   ```bash
   netstat -tulpn | grep -E "30333|8545|8546"
   ```

3. Check disk space:

   ```bash
   df -h
   ```

4. Review logs:

   ```bash
   journalctl -u luxtensor-node -n 100
   ```

---

### Node Not Syncing

**Symptoms**: Block height not increasing, stuck at a block.

**Solutions**:

1. **Check peers**:

   ```bash
   curl -X POST http://localhost:8545 \
     -d '{"jsonrpc":"2.0","method":"system_peers","params":[],"id":1}'
   ```

   If peers = 0, check firewall and port forwarding.

2. **Force reconnect**:

   ```bash
   # Add more bootstrap nodes
   luxtensor-node --bootnodes /ip4/x.x.x.x/tcp/30333/p2p/12D3...
   ```

3. **Clear and resync**:

   ```bash
   rm -rf ~/.luxtensor/db
   systemctl restart luxtensor-node
   ```

---

### High Memory Usage

**Symptoms**: Node uses excessive RAM (>80%).

**Solutions**:

1. Reduce RocksDB cache:

   ```toml
   [storage]
   cache_size = 134217728  # 128MB instead of 256MB
   ```

2. Limit peer connections:

   ```toml
   [network]
   max_peers = 25  # Default: 50
   ```

3. Enable pruning:

   ```toml
   [storage]
   pruning = true
   keep_blocks = 10000
   ```

---

## Validator Issues

### Validator Not Producing Blocks

**Symptoms**: Registered validator never produces blocks.

**Solutions**:

1. **Check activation epoch**:

   ```bash
   curl -X POST http://localhost:8545 \
     -d '{"jsonrpc":"2.0","method":"staking_getValidator","params":["0xYOUR_ADDR"],"id":1}'
   ```

   Validators activate after `activation_epoch`.

2. **Verify stake**:
   Minimum stake is 10,000 MDT.

3. **Check consensus status**:

   ```bash
   curl -X POST http://localhost:8545 \
     -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}'
   ```

---

### High Missed Blocks

**Symptoms**: Validator missing blocks, potential slashing.

**Solutions**:

1. **Check system clock**:

   ```bash
   timedatectl status
   # Enable NTP sync
   timedatectl set-ntp true
   ```

2. **Check network latency**:

   ```bash
   ping seed1.luxtensor.network
   ```

3. **Increase resources**:
   - CPU: At least 4 cores recommended
   - RAM: At least 8 GB
   - Disk: SSD required

4. **Review missed blocks**:

   ```bash
   curl -X POST http://localhost:8545 \
     -d '{"jsonrpc":"2.0","method":"staking_getValidator","params":["0xADDR"],"id":1}' \
     | jq '.result.missed_blocks'
   ```

---

## Network Issues

### Peer Count Low

**Symptoms**: Less than 5 peers, slow sync.

**Solutions**:

1. **Check firewall**:

   ```bash
   ufw allow 30333/tcp
   ```

2. **Port forwarding**: Ensure router forwards port 30333.

3. **Add bootstrap nodes**:

   ```toml
   [network]
   bootstrap_nodes = [
     "/ip4/seed1.luxtensor.network/tcp/30333/p2p/...",
     "/ip4/seed2.luxtensor.network/tcp/30333/p2p/..."
   ]
   ```

---

### Eclipse Attack Warning

**Symptoms**: Log shows "Eclipse protection triggered".

**Solutions**:

This is a security feature. If legitimate:

1. **Diversify connections**:

   ```toml
   [network]
   max_peers_per_subnet = 3
   ```

2. **Add trusted peers** from different ASNs.

---

## RPC Issues

### RPC Not Responding

**Solutions**:

1. **Check RPC enabled**:

   ```toml
   [rpc]
   enabled = true
   addr = "127.0.0.1:8545"
   ```

2. **Check rate limits**:
   Default: 100 requests/second.

3. **Enable CORS** for web clients:

   ```toml
   [rpc]
   cors = ["*"]
   ```

---

## Getting Help

1. **Logs**: `journalctl -u luxtensor-node -f`
2. **Discord**: [join.luxtensor.network/discord](https://join.luxtensor.network/discord)
3. **GitHub Issues**: [github.com/moderntensor/luxtensor/issues](https://github.com/moderntensor/luxtensor/issues)
