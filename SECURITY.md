# Security Policy

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via email to: **<security@moderntensor.io>**

You should receive a response within 48 hours. If for some reason you do not, please follow up via email to ensure we received your original message.

Please include:

- Type of issue (e.g., buffer overflow, reentrancy, timing attack, etc.)
- Full paths of source file(s)
- Step-by-step instructions to reproduce
- Impact and potential exploitation
- Proof-of-concept (if available)

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.5.x   | ✅ (current) |
| 0.4.x   | ✅ (security fixes) |
| < 0.4   | ❌ |

## Security Measures

### Smart Contracts (Solidity)

- **OpenZeppelin** battle-tested base contracts
- **ReentrancyGuard** on all state-changing functions
- **SafeERC20** for token transfers
- **Access control** with role-based permissions
- **Gas limits** on unbounded arrays:
  - `MAX_PARTICIPANTS = 1000` (GradientAggregator)
  - `MAX_ATTESTATIONS = 100` (TrustGraph)

### Blockchain Core (Rust)

- **Constant-time comparisons** for sensitive data (`subtle::ConstantTimeEq`)
- **Input validation** on all RPC endpoints
- **Safe error handling** with `unwrap_or()` patterns
- **Externalized configuration** via `constants.rs`
- **Rate limiting** at reverse proxy layer (see below)

### Rate Limiting (Production)

We recommend implementing rate limiting at the nginx reverse proxy layer:

```nginx
# /etc/nginx/conf.d/rpc_limit.conf
limit_req_zone $binary_remote_addr zone=rpc:10m rate=100r/s;
limit_conn_zone $binary_remote_addr zone=rpc_conn:10m;

server {
    location /rpc {
        limit_req zone=rpc burst=200 nodelay;
        limit_conn rpc_conn 50;
        proxy_pass http://127.0.0.1:8545;
    }
}
```

This provides:

- **100 requests/second** per IP address
- **Burst capacity** of 200 requests
- **Maximum 50 concurrent** connections per IP

## Security Update Process

1. **Disclosure** - Security team confirms the vulnerability
2. **Fix Development** - Patch developed in private repository
3. **Testing** - Comprehensive testing of the patch
4. **Release** - Patch released with security advisory
5. **Public Disclosure** - After 90 days or patch deployment

## Bug Bounty Program

We plan to launch a bug bounty program before mainnet. Details will be announced at [moderntensor.io/security](https://moderntensor.io/security).

## Contact

- **Security Email:** <security@moderntensor.io>
- **GPG Key:** Available at [moderntensor.io/.well-known/security.txt](https://moderntensor.io/.well-known/security.txt)
