# ModernTensor Operations Manual

**Version:** 1.0.0  
**Last Updated:** 2026-01-08  
**Status:** Production Ready

This manual provides complete operational guidelines for deploying, monitoring, and maintaining ModernTensor blockchain nodes.

## Quick Links

- [Deployment Guide](#1-deployment-guide)
- [Monitoring Setup](#2-monitoring-setup)
- [Security Configuration](#3-security-configuration)
- [Troubleshooting](#4-troubleshooting)
- [Best Practices](#5-best-practices)

## 1. Deployment Guide

### Docker Deployment

```bash
# Start production environment
cd docker
docker-compose -f docker-compose.production.yml up -d

# Check status
docker-compose ps
docker logs -f moderntensor-validator
```

### Kubernetes Deployment

```bash
# Create namespace and deploy
kubectl apply -f k8s/base/namespace.yaml
kubectl apply -f k8s/base/validator-deployment.yaml

# Check status
kubectl get pods -n moderntensor
kubectl logs -f deployment/moderntensor-validator -n moderntensor
```

## 2. Monitoring Setup

### Access Grafana

```bash
# Docker: http://localhost:3000
# Kubernetes: kubectl port-forward svc/grafana 3000:3000 -n monitoring
```

### Import Dashboards

1. Login to Grafana (admin/admin)
2. Go to Dashboards → Import
3. Upload files from `grafana/dashboards/`
4. Select Prometheus data source

## 3. Security Configuration

### RBAC Setup

```python
from sdk.security import get_access_control, Role

ac = get_access_control()
ac.create_user("admin-001", roles=[Role.ADMIN])
```

### JWT Configuration

```python
from sdk.axon.security import JWTAuthenticator

jwt = JWTAuthenticator(secret_key="YOUR_SECRET")
token = jwt.generate_token(uid="user-001")
```

## 4. Troubleshooting

### Node Won't Start

```bash
# Check logs
docker logs moderntensor-validator
kubectl logs deployment/moderntensor-validator -n moderntensor

# Common fixes:
# 1. Check configuration in .env
# 2. Verify port availability
# 3. Check file permissions
```

### High Memory Usage

```bash
# Check usage
docker stats
kubectl top pods -n moderntensor

# Increase limits in deployment config
```

## 5. Best Practices

1. **Security:** Change default passwords, enable TLS
2. **Monitoring:** Check dashboards daily, set up alerts
3. **Backups:** Automated daily backups, test recovery
4. **Updates:** Weekly updates, rolling deployments
5. **Documentation:** Document all changes

---

See full operations manual for detailed procedures.

**Support:** support@moderntensor.io
