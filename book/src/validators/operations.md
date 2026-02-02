# Operations & Monitoring

## Monitoring

We recommend using Prometheus & Grafana.

### Enable Metrics

In `node.toml`:

```toml
[metrics]
enabled = true
addr = "0.0.0.0:9090"
```

### Key Metrics

- `luxtensor_block_height`: Should increase steadily.
- `luxtensor_peer_count`: Should be > 5.
- `luxtensor_missed_blocks`: Should be 0.

## Troubleshooting

### Node Not Syncing

1. Check internet connection.
2. Check if peers are connected (`system_peers`).
3. Check firewall rules (Port 30333).

### Validator Not Producing Blocks

1. Is your stake > 10,000 MDT?
2. Did you wait 1 epoch?
3. Is your validator key correct?

## Safe Exit

To withdraw your stake and stop validating:

```python
# Unregister (Available after exit_delay)
client.staking.exit_validator()
```

*Default exit delay: 2 epochs.*
