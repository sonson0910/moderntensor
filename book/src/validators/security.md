# Security Best Practices

Securing a validator is critical to avoid slashing and loss of funds.

## 1. Network Security

- **Firewall**: Only open port 30333 (P2P). Close 8545 (RPC) to public internet.
- **DDoS Protection**: Use a sentry node architecture.

## 2. Key Management

- **Validator Key**: Hot key used for signing blocks. Store on the server, heavily protected (chmod 600).
- **Withdrawal Key**: Cold key. Keep OFF the server (Hardware wallet / Paper wallet).

## 3. Operations

- **Backups**: Backup your `validator.key` and `node.toml`.
- **Updates**: Join the Discord for emergency security patches.
- **Failover**: Have a backup node ready (but NOT running simultaneously to avoid double-signing).
