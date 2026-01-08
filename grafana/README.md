# ModernTensor Grafana Dashboards

This directory contains Grafana dashboard templates for monitoring ModernTensor blockchain.

## 📊 Available Dashboards

### 1. Blockchain Metrics (`blockchain_metrics.json`)
Monitors core blockchain operations and performance:
- **Block Height**: Current blockchain height
- **Block Production Time**: Average time to produce blocks
- **Transactions Per Second (TPS)**: Real-time transaction throughput
- **Transactions Per Block**: Average transactions per block
- **Total Blocks**: Cumulative blocks produced
- **Total Accounts**: Number of accounts in state
- **State Size**: Size of blockchain state in bytes
- **Gas Usage**: Average gas used per transaction
- **Block Size**: Average block size in bytes

### 2. Network Metrics (`network_metrics.json`)
Tracks network health and P2P connectivity:
- **Connected Peers**: Number of connected peers
- **Sync Progress**: Blockchain synchronization percentage
- **Network Bandwidth**: Bytes received/sent per second
- **Peer Messages**: Messages received/sent by type
- **Network Summary**: Aggregated network statistics

### 3. Security Metrics (`security_metrics.json`)
Monitors security events and threats:
- **Authentication Failures**: Failed authentication attempts
- **Rate Limit Violations**: Rate limit hits
- **Blocked IPs**: Number of blocked IP addresses
- **DDoS Protection Events**: DDoS attack detections
- **Active Connections**: Current active connections
- **Total Auth Attempts**: Cumulative authentication attempts
- **Security Alerts**: Total security alerts with threshold colors
- **Security Events Timeline**: Time-series view of security events

**Alerts Configured:**
- High Auth Failures (threshold: 10 in 5 minutes)
- DDoS Attack Detected (threshold: 5 events in 5 minutes)

### 4. Consensus Metrics (`consensus_metrics.json`)
Tracks consensus mechanism and validator performance:
- **Active Validators**: Number of active validators
- **Total Validator Stake**: Combined stake across all validators
- **Epoch Number**: Current consensus epoch
- **Validator Rewards**: Rate of rewards distribution
- **Validator Penalties**: Rate of penalties applied
- **AI Tasks Execution**: AI task completion rate by status
- **AI Task Execution Time**: Average AI task execution duration
- **Total AI Tasks**: Cumulative AI tasks submitted

## 🚀 Installation

### 1. Import Dashboards Manually

1. Open Grafana UI
2. Navigate to **Dashboards** → **Import**
3. Click **Upload JSON file**
4. Select one of the dashboard JSON files
5. Click **Import**

### 2. Import via Provisioning

Add to your Grafana provisioning configuration:

```yaml
# grafana/provisioning/dashboards/moderntensor.yaml
apiVersion: 1

providers:
  - name: 'ModernTensor'
    orgId: 1
    folder: 'ModernTensor'
    type: file
    disableDeletion: false
    updateIntervalSeconds: 30
    allowUiUpdates: true
    options:
      path: /etc/grafana/provisioning/dashboards/moderntensor
```

Copy dashboard files to the provisioning directory:
```bash
mkdir -p /etc/grafana/provisioning/dashboards/moderntensor
cp grafana/dashboards/*.json /etc/grafana/provisioning/dashboards/moderntensor/
```

### 3. Configure Data Source

Ensure Prometheus data source is configured in Grafana:

```yaml
# grafana/provisioning/datasources/prometheus.yaml
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true
    editable: true
```

## 📈 Dashboard Features

### Time Controls
- Default time range: Last 6 hours
- Available refresh intervals: 10s, 30s, 1m, 5m
- Auto-refresh: 30 seconds (configurable)

### Alerts
Security dashboard includes pre-configured alerts:
- High authentication failures
- DDoS attack detection

Configure alert notifications in Grafana:
1. Go to **Alerting** → **Notification channels**
2. Add channels (Email, Slack, PagerDuty, etc.)
3. Alerts will automatically use configured channels

## 🔧 Customization

### Modifying Dashboards

1. Import dashboard to Grafana
2. Click **Dashboard settings** (gear icon)
3. Modify panels, queries, or layout
4. Click **Save dashboard**
5. Export JSON and replace original file

### Adding Custom Panels

Use these Prometheus metrics:

**Blockchain:**
- `moderntensor_block_height`
- `moderntensor_block_production_seconds`
- `moderntensor_transactions_total{status="..."}`
- `moderntensor_state_accounts_total`

**Network:**
- `moderntensor_peers_connected`
- `moderntensor_network_bytes_received_total`
- `moderntensor_peer_messages_received_total{message_type="..."}`

**Consensus:**
- `moderntensor_validators_active`
- `moderntensor_validator_stake_total`
- `moderntensor_ai_tasks_completed_total{status="..."}`

**Security:**
- `moderntensor_auth_failures_total`
- `moderntensor_rate_limit_violations_total`
- `moderntensor_ddos_events_total`

See `sdk/monitoring/metrics.py` for complete metrics list.

## 📖 Usage Tips

### Performance Monitoring
- Watch **Block Production Time** for consensus issues
- Monitor **TPS** for throughput bottlenecks
- Check **Gas Usage** for transaction costs

### Network Health
- **Connected Peers** should be > 5 for healthy network
- **Sync Progress** = 100% when fully synchronized
- Monitor **Bandwidth** for network congestion

### Security Monitoring
- Set up alerts for **Authentication Failures**
- Watch **Blocked IPs** for attack patterns
- Monitor **DDoS Events** for network attacks

### Consensus Health
- **Active Validators** should meet minimum threshold
- Monitor **Validator Penalties** for misbehavior
- Track **AI Task** completion rates and execution times

## 🔗 Related Documentation

- [Monitoring & Observability](../../sdk/monitoring/README.md)
- [Prometheus Metrics](../../sdk/monitoring/metrics.py)
- [Alert Rules](../../sdk/monitoring/alerts.py)
- [Security Features](../../sdk/security/README.md)

## 🐛 Troubleshooting

### No Data Displayed

1. Verify Prometheus is scraping metrics:
   ```bash
   curl http://localhost:9090/api/v1/targets
   ```

2. Check ModernTensor metrics endpoint:
   ```bash
   curl http://localhost:8080/metrics
   ```

3. Verify Grafana data source connection:
   - Go to **Configuration** → **Data Sources**
   - Click **Prometheus**
   - Click **Test** button

### Slow Dashboard Loading

1. Reduce time range (e.g., last 1 hour)
2. Increase refresh interval (e.g., 1 minute)
3. Optimize Prometheus queries (use recording rules)

### Alert Not Firing

1. Check alert conditions in panel settings
2. Verify notification channels are configured
3. Check Grafana alert evaluation (Alerting → Alert Rules)

## 📝 Version History

- **v1.0** (2026-01-08): Initial dashboard suite
  - Blockchain metrics dashboard
  - Network metrics dashboard
  - Security metrics dashboard
  - Consensus metrics dashboard

## 🤝 Contributing

To contribute new dashboards:

1. Create dashboard in Grafana
2. Export as JSON
3. Add to `grafana/dashboards/`
4. Update this README
5. Submit pull request

---

**Last Updated**: 2026-01-08  
**Maintainer**: ModernTensor Team
