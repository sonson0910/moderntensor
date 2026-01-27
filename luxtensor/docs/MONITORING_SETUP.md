# Luxtensor Monitoring Setup Guide

## 📋 Overview

This guide covers setting up monitoring for Luxtensor nodes using:

- **Prometheus** - Metrics collection
- **Grafana** - Visualization dashboards
- **AlertManager** - Alerting

---

## 1. Prometheus Configuration

### 1.1 Install Prometheus

```bash
# Ubuntu/Debian
sudo apt update && sudo apt install prometheus

# Or download binary
wget https://github.com/prometheus/prometheus/releases/download/v2.47.0/prometheus-2.47.0.linux-amd64.tar.gz
tar xvfz prometheus-*.tar.gz
cd prometheus-*
```

### 1.2 Configure Prometheus

Create `/etc/prometheus/prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - localhost:9093

rule_files:
  - "luxtensor_alerts.yml"

scrape_configs:
  # Luxtensor Node Metrics
  - job_name: 'luxtensor-node'
    static_configs:
      - targets:
        - 'node1.example.com:8545'
        - 'node2.example.com:8545'
        - 'node3.example.com:8545'
    metrics_path: '/metrics'
    scrape_interval: 10s

  # Luxtensor Indexer Metrics
  - job_name: 'luxtensor-indexer'
    static_configs:
      - targets: ['indexer.example.com:4000']
    metrics_path: '/metrics'
```

### 1.3 Create Alert Rules

Create `/etc/prometheus/luxtensor_alerts.yml`:

```yaml
groups:
  - name: luxtensor-critical
    rules:
      # Node not producing blocks
      - alert: NodeNotProducingBlocks
        expr: increase(luxtensor_blocks_produced_total[5m]) == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Node {{ $labels.instance }} not producing blocks"
          description: "No blocks produced in 5 minutes"

      # Low peer count
      - alert: LowPeerCount
        expr: luxtensor_peer_count < 3
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Low peer count on {{ $labels.instance }}"
          description: "Peer count is {{ $value }}"

      # Node not synced
      - alert: NodeNotSynced
        expr: luxtensor_is_syncing == 1
        for: 30m
        labels:
          severity: warning
        annotations:
          summary: "Node {{ $labels.instance }} still syncing"

      # High memory usage
      - alert: HighMemoryUsage
        expr: process_resident_memory_bytes / 1024 / 1024 / 1024 > 8
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage on {{ $labels.instance }}"
          description: "Memory usage is {{ $value }}GB"

      # Mempool full
      - alert: MempoolFull
        expr: luxtensor_mempool_size > 9000
        for: 1m
        labels:
          severity: warning
        annotations:
          summary: "Mempool nearly full on {{ $labels.instance }}"

  - name: luxtensor-consensus
    rules:
      # Missed blocks
      - alert: ValidatorMissedBlocks
        expr: increase(luxtensor_missed_blocks_total[10m]) > 3
        for: 1m
        labels:
          severity: warning
        annotations:
          summary: "Validator {{ $labels.instance }} missed blocks"

      # Slashing event
      - alert: SlashingEvent
        expr: increase(luxtensor_slashing_events_total[5m]) > 0
        for: 0m
        labels:
          severity: critical
        annotations:
          summary: "Slashing event detected"
```

---

## 2. Grafana Setup

### 2.1 Install Grafana

```bash
# Ubuntu/Debian
sudo apt install grafana

# Start service
sudo systemctl start grafana-server
sudo systemctl enable grafana-server
```

### 2.2 Add Prometheus Data Source

1. Open Grafana: <http://localhost:3000>
2. Login (default: admin/admin)
3. Configuration → Data Sources → Add data source
4. Select Prometheus
5. URL: <http://localhost:9090>
6. Save & Test

### 2.3 Import Luxtensor Dashboard

Create dashboard JSON and import:

```json
{
  "title": "Luxtensor Node Dashboard",
  "panels": [
    {
      "title": "Block Height",
      "type": "stat",
      "targets": [
        {
          "expr": "luxtensor_block_height",
          "legendFormat": "{{instance}}"
        }
      ]
    },
    {
      "title": "Peer Count",
      "type": "gauge",
      "targets": [
        {
          "expr": "luxtensor_peer_count"
        }
      ],
      "fieldConfig": {
        "defaults": {
          "thresholds": {
            "steps": [
              {"color": "red", "value": 0},
              {"color": "yellow", "value": 3},
              {"color": "green", "value": 10}
            ]
          }
        }
      }
    },
    {
      "title": "Transactions Per Block",
      "type": "timeseries",
      "targets": [
        {
          "expr": "rate(luxtensor_transactions_total[5m])"
        }
      ]
    },
    {
      "title": "Block Time",
      "type": "timeseries",
      "targets": [
        {
          "expr": "luxtensor_avg_block_time_ms / 1000"
        }
      ]
    },
    {
      "title": "Mempool Size",
      "type": "timeseries",
      "targets": [
        {
          "expr": "luxtensor_mempool_size"
        }
      ]
    },
    {
      "title": "Memory Usage",
      "type": "timeseries",
      "targets": [
        {
          "expr": "process_resident_memory_bytes / 1024 / 1024"
        }
      ]
    }
  ]
}
```

---

## 3. Node Metrics Endpoint

Luxtensor node exposes metrics at `/metrics`:

```bash
curl http://localhost:8545/metrics
```

### Available Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `luxtensor_block_height` | Gauge | Current block height |
| `luxtensor_blocks_produced_total` | Counter | Blocks produced |
| `luxtensor_transactions_total` | Counter | Total transactions |
| `luxtensor_peer_count` | Gauge | Connected peers |
| `luxtensor_mempool_size` | Gauge | Pending transactions |
| `luxtensor_avg_block_time_ms` | Gauge | Average block time |
| `luxtensor_uptime_seconds` | Gauge | Node uptime |
| `luxtensor_is_syncing` | Gauge | 1 if syncing, 0 if synced |

---

## 4. Quick Start Script

```bash
#!/bin/bash
# setup_monitoring.sh

# Install Prometheus
docker run -d --name prometheus \
  -p 9090:9090 \
  -v /etc/prometheus:/etc/prometheus \
  prom/prometheus

# Install Grafana
docker run -d --name grafana \
  -p 3000:3000 \
  grafana/grafana

# Install AlertManager
docker run -d --name alertmanager \
  -p 9093:9093 \
  prom/alertmanager

echo "Prometheus: http://localhost:9090"
echo "Grafana: http://localhost:3000 (admin/admin)"
echo "AlertManager: http://localhost:9093"
```

---

## 5. Recommended Alert Channels

### Slack Integration

```yaml
# alertmanager.yml
receivers:
  - name: 'slack-notifications'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/XXX/YYY/ZZZ'
        channel: '#luxtensor-alerts'
        send_resolved: true
```

### PagerDuty Integration

```yaml
receivers:
  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: 'YOUR_SERVICE_KEY'
```

---

## 📊 Dashboard Screenshots

### Main Dashboard

- Block height across all nodes
- Transaction throughput
- Peer connectivity map

### Validator Dashboard

- Blocks produced per validator
- Missed blocks
- Slashing events

### Network Dashboard

- Geographic peer distribution
- Message propagation times
- Sync status
