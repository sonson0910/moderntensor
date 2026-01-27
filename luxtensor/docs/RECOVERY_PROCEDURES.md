# Luxtensor Node Recovery Procedures

## 📋 Table of Contents

1. [Node Crash Recovery](#node-crash-recovery)
2. [Database Corruption](#database-corruption)
3. [Network Partition Recovery](#network-partition-recovery)
4. [State Sync from Scratch](#state-sync-from-scratch)
5. [Validator Key Recovery](#validator-key-recovery)
6. [Emergency Procedures](#emergency-procedures)

---

## 1. Node Crash Recovery

### Symptoms

- Node process terminated unexpectedly
- Service not responding

### Steps

```bash
# 1. Check logs for crash reason
tail -100 /var/log/luxtensor/node.log

# 2. Verify database integrity
./luxtensor-node --verify-db --data-dir ./data

# 3. Restart node
systemctl restart luxtensor-node
# OR
./luxtensor-node --config config.toml

# 4. Verify sync status
curl -s http://localhost:8545 -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"system_health","id":1}'
```

### If Node Won't Start

```bash
# Try loading mempool backup
./luxtensor-node --load-mempool ./data/mempool_backup.bin

# If DB corrupted, restore from backup
./luxtensor-node --restore-backup ./backups/latest
```

---

## 2. Database Corruption

### Symptoms

- "Database corruption detected" in logs
- State root mismatch
- Unable to read blocks

### Steps

```bash
# 1. Stop node immediately
systemctl stop luxtensor-node

# 2. List available backups
ls -la ./data/backups/

# 3. Restore from latest backup
./luxtensor-node --restore-backup ./data/backups/backup_latest_*

# 4. Verify restoration
./luxtensor-node --verify-db

# 5. Restart and sync
systemctl start luxtensor-node
```

### No Backup Available

```bash
# Full resync from genesis
rm -rf ./data/db
./luxtensor-node --config config.toml --sync-mode full
```

---

## 3. Network Partition Recovery

### Symptoms

- Node isolated from network
- No new blocks for extended period
- Peer count = 0

### Steps

```bash
# 1. Check connectivity
curl -s http://localhost:8545 -X POST \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","id":1}'

# 2. Manually add bootstrap peers
curl -s http://localhost:8545 -X POST \
  -d '{"jsonrpc":"2.0","method":"admin_addPeer","params":["/ip4/SEED_IP/tcp/30303/p2p/PEER_ID"],"id":1}'

# 3. Check for fork
curl -s http://localhost:8545 -X POST \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","id":1}'

# Compare with public explorer or other nodes
```

### If on Wrong Fork

```bash
# Stop node
systemctl stop luxtensor-node

# Rollback to common ancestor
./luxtensor-node --rollback-to-block <HEIGHT>

# Restart and sync to correct chain
systemctl start luxtensor-node
```

---

## 4. State Sync from Scratch

### When to Use

- New node setup
- Corrupted state beyond recovery
- Moving to new hardware

### Steps

```bash
# 1. Clean data directory
rm -rf ./data/db ./data/state

# 2. Keep identity (for known peer ID)
# ./data/node.key - DO NOT DELETE

# 3. Start with fast sync
./luxtensor-node \
  --config config.toml \
  --sync-mode fast \
  --checkpoint-url https://checkpoints.luxtensor.network/latest

# 4. Monitor sync progress
watch -n 5 'curl -s localhost:8545 -X POST \
  -d "{\"jsonrpc\":\"2.0\",\"method\":\"eth_syncing\",\"id\":1}"'
```

---

## 5. Validator Key Recovery

### Lost Validator Key
>
> ⚠️ CRITICAL: Lost validator key = Lost ability to produce blocks

### Prevention

```bash
# Backup validator key to secure location
cp ./data/validator.key /secure/backup/location/
# Also keep encrypted copy offline
```

### Recovery from Backup

```bash
# 1. Stop validator
systemctl stop luxtensor-node

# 2. Restore key
cp /secure/backup/validator.key ./data/

# 3. Verify key
./luxtensor-node --verify-validator-key ./data/validator.key

# 4. Restart
systemctl start luxtensor-node
```

### If Key is Truly Lost

```
1. Your validator stake is still on-chain
2. Wait for unbonding period (typically 7-14 days)
3. Create new validator with new key
4. Re-stake tokens
```

---

## 6. Emergency Procedures

### 6.1 Network Attack Detected

```bash
# 1. Enable strict rate limiting
# Edit config.toml:
# [rpc]
# rate_limit = 10  # Very restrictive

# 2. Block suspicious IPs
iptables -A INPUT -s ATTACKER_IP -j DROP

# 3. Notify other validators
# Use out-of-band communication
```

### 6.2 Critical Bug - Stop All Validators

```bash
# Emergency stop
systemctl stop luxtensor-node

# Wait for official patch
# DO NOT restart until fixed binary is released
```

### 6.3 Create Emergency Backup

```bash
# While node is running
curl -s http://localhost:8545 -X POST \
  -d '{"jsonrpc":"2.0","method":"admin_createBackup","params":["emergency"],"id":1}'

# Or stop and backup manually
systemctl stop luxtensor-node
cp -r ./data ./emergency_backup_$(date +%Y%m%d_%H%M%S)
systemctl start luxtensor-node
```

---

## 📞 Support Contacts

| Issue | Contact |
|-------|---------|
| Network-wide issues | Discord #validators |
| Individual node | Support ticket |
| Security vulnerabilities | <security@moderntensor.io> |

---

## 📝 Recovery Checklist

- [ ] Node responding to RPC
- [ ] Peer count > 5
- [ ] Syncing or synced
- [ ] Producing blocks (if validator)
- [ ] Metrics endpoint working
- [ ] Logs showing normal operation
