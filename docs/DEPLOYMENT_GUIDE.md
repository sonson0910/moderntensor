# ModernTensor Phase 6: Deployment Guide

This document provides comprehensive deployment instructions for ModernTensor Layer 1 blockchain nodes using Docker and Kubernetes.

## Table of Contents
- [Prerequisites](#prerequisites)
- [Local Development with Docker Compose](#local-development-with-docker-compose)
- [Production Deployment with Kubernetes](#production-deployment-with-kubernetes)
- [Monitoring Setup](#monitoring-setup)
- [Testing](#testing)

## Prerequisites

### For Docker Deployment
- Docker Engine 20.10+
- Docker Compose 1.29+
- 8GB+ RAM
- 50GB+ disk space

### For Kubernetes Deployment
- Kubernetes cluster 1.20+
- kubectl configured
- Persistent volume provisioner
- 100GB+ storage per node
- LoadBalancer or Ingress controller

## Local Development with Docker Compose

### Quick Start

1. **Build the Docker image:**
```bash
cd docker
chmod +x build.sh
./build.sh
```

2. **Start the network:**
```bash
docker-compose up -d
```

3. **Check status:**
```bash
docker-compose ps
docker-compose logs -f validator1
```

### Network Architecture

The docker-compose setup creates:
- **3 Validator Nodes** (validator1, validator2, validator3)
- **Prometheus** for metrics collection
- **Grafana** for visualization

#### Port Mapping

| Service | RPC Port | P2P Port | Metrics Port |
|---------|----------|----------|--------------|
| Validator 1 | 8545 | 30303 | 9090 |
| Validator 2 | 8546 | 30304 | 9091 |
| Validator 3 | 8547 | 30305 | 9092 |
| Prometheus | - | - | 9093 |
| Grafana | 3000 | - | - |

### Interacting with the Network

#### Using curl (JSON-RPC)
```bash
# Get current block number
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_blockNumber",
    "params": [],
    "id": 1
  }'

# Get account balance
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_getBalance",
    "params": ["0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb", "latest"],
    "id": 1
  }'
```

#### Using Web3.py
```python
from web3 import Web3

# Connect to node
w3 = Web3(Web3.HTTPProvider('http://localhost:8545'))

# Check connection
print(f"Connected: {w3.is_connected()}")
print(f"Block number: {w3.eth.block_number}")
```

### Stopping the Network
```bash
docker-compose down

# To also remove volumes (WARNING: This deletes all blockchain data)
docker-compose down -v
```

## Production Deployment with Kubernetes

### Step 1: Create Namespace
```bash
kubectl apply -f k8s/namespace.yaml
```

### Step 2: Create ConfigMap
```bash
kubectl apply -f k8s/configmap.yaml
```

### Step 3: Deploy StatefulSet
```bash
kubectl apply -f k8s/statefulset.yaml
```

This creates:
- 3 validator pods with persistent storage
- Automatic pod scheduling
- Rolling update support
- Health checks (liveness and readiness probes)

### Step 4: Create Services
```bash
kubectl apply -f k8s/service.yaml
```

This creates:
- **Headless service** for inter-pod communication
- **LoadBalancer service** for external RPC access
- **ClusterIP service** for metrics

### Verify Deployment
```bash
# Check pod status
kubectl get pods -n moderntensor

# Check services
kubectl get svc -n moderntensor

# View logs
kubectl logs -n moderntensor moderntensor-validator-0 --tail=50

# Exec into pod
kubectl exec -it -n moderntensor moderntensor-validator-0 -- /bin/bash
```

### Scaling

To change the number of validators:
```bash
kubectl scale statefulset moderntensor-validator -n moderntensor --replicas=5
```

### Rolling Updates
```bash
# Update image
kubectl set image statefulset/moderntensor-validator \
  -n moderntensor \
  validator=moderntensor:v0.2.0

# Check rollout status
kubectl rollout status statefulset/moderntensor-validator -n moderntensor
```

### Accessing RPC API

#### Via LoadBalancer
```bash
# Get external IP
EXTERNAL_IP=$(kubectl get svc moderntensor-rpc -n moderntensor -o jsonpath='{.status.loadBalancer.ingress[0].ip}')

# Make RPC call
curl -X POST http://$EXTERNAL_IP:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

#### Via Port Forward (for testing)
```bash
kubectl port-forward -n moderntensor svc/moderntensor-rpc 8545:8545

# In another terminal
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

## Monitoring Setup

### Prometheus

Prometheus is configured to scrape metrics from all validator nodes every 15 seconds.

#### Access Prometheus UI (Docker)
```bash
open http://localhost:9093
```

#### Access Prometheus UI (Kubernetes)
```bash
kubectl port-forward -n moderntensor svc/prometheus 9090:9090
open http://localhost:9090
```

#### Example Queries
```promql
# Current block height
moderntensor_block_height

# Transaction rate
rate(moderntensor_transactions_total[5m])

# Connected peers
moderntensor_peers_connected

# Block production time (p95)
histogram_quantile(0.95, rate(moderntensor_block_production_seconds_bucket[5m]))
```

### Grafana

#### Access Grafana (Docker)
```bash
open http://localhost:3000
```
Default credentials: admin/admin

#### Add Prometheus Data Source
1. Go to Configuration > Data Sources
2. Click "Add data source"
3. Select "Prometheus"
4. Set URL to `http://prometheus:9090` (Docker) or appropriate K8s service
5. Click "Save & Test"

#### Import Dashboard
Create a dashboard with panels for:
- Block height over time
- Transaction throughput
- Connected peers
- Block production latency
- Network bandwidth
- Validator status

## Testing

### Run Integration Tests
```bash
# Install dependencies
pip install pytest pytest-asyncio plyvel

# Run tests
cd /home/runner/work/moderntensor/moderntensor
python -m pytest tests/integration/test_full_flow.py -v
```

### Load Testing
```bash
# Install hey (HTTP load testing tool)
go install github.com/rakyll/hey@latest

# Test RPC endpoint
hey -n 1000 -c 10 -m POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://localhost:8545
```

## Troubleshooting

### Docker Issues

**Issue: Containers won't start**
```bash
# Check logs
docker-compose logs validator1

# Check disk space
df -h

# Restart services
docker-compose restart
```

**Issue: Cannot connect to RPC**
```bash
# Check if port is listening
netstat -tulpn | grep 8545

# Check firewall
sudo ufw status
```

### Kubernetes Issues

**Issue: Pods stuck in Pending**
```bash
# Check events
kubectl describe pod moderntensor-validator-0 -n moderntensor

# Check PVC
kubectl get pvc -n moderntensor

# Check resources
kubectl top nodes
```

**Issue: Pods restarting**
```bash
# Check logs
kubectl logs moderntensor-validator-0 -n moderntensor --previous

# Check resource limits
kubectl describe pod moderntensor-validator-0 -n moderntensor | grep -A 10 "Limits"
```

**Issue: Cannot access services**
```bash
# Check service endpoints
kubectl get endpoints -n moderntensor

# Test from within cluster
kubectl run -it --rm debug --image=busybox --restart=Never -- wget -O- http://moderntensor-rpc:8545/health
```

## Performance Tuning

### Docker Compose
Edit `docker-compose.yml`:
```yaml
environment:
  - MT_CACHE_SIZE=2048  # Increase cache
  - MT_MAX_PEERS=100    # More peers
```

### Kubernetes
Edit `k8s/statefulset.yaml`:
```yaml
resources:
  requests:
    memory: "4Gi"   # Increase memory
    cpu: "2000m"    # Increase CPU
  limits:
    memory: "8Gi"
    cpu: "4000m"
```

## Security Considerations

1. **Network Policies**: Implement K8s network policies to restrict pod-to-pod communication
2. **RBAC**: Configure proper role-based access control
3. **Secrets Management**: Use Kubernetes secrets for sensitive data
4. **TLS**: Enable TLS for RPC endpoints in production
5. **Firewall**: Configure firewall rules for P2P and RPC ports

## Backup and Recovery

### Backup Blockchain Data
```bash
# Docker
docker run --rm -v moderntensor_validator1-data:/data -v $(pwd):/backup \
  ubuntu tar czf /backup/validator1-backup.tar.gz /data

# Kubernetes
kubectl exec moderntensor-validator-0 -n moderntensor -- \
  tar czf /tmp/backup.tar.gz /data
kubectl cp moderntensor-validator-0:/tmp/backup.tar.gz ./backup.tar.gz -n moderntensor
```

### Restore from Backup
```bash
# Docker
docker run --rm -v moderntensor_validator1-data:/data -v $(pwd):/backup \
  ubuntu tar xzf /backup/validator1-backup.tar.gz -C /

# Kubernetes
kubectl cp ./backup.tar.gz moderntensor-validator-0:/tmp/backup.tar.gz -n moderntensor
kubectl exec moderntensor-validator-0 -n moderntensor -- \
  tar xzf /tmp/backup.tar.gz -C /
```

## Next Steps

1. **Configure Monitoring Alerts**: Set up Prometheus AlertManager
2. **Implement CI/CD**: Automate deployment with GitHub Actions
3. **Add Ingress**: Configure Ingress for RPC access
4. **Enable TLS**: Set up certificates for secure communication
5. **Configure Autoscaling**: Implement horizontal pod autoscaling

## Support

For issues or questions:
- GitHub Issues: https://github.com/sonson0910/moderntensor/issues
- Documentation: See LAYER1_IMPLEMENTATION_SUMMARY.md
- API Guide: See docs/API_USAGE.md
