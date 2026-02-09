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

ModernTensor operates a bug bounty program to incentivize responsible disclosure of security vulnerabilities. We value the contributions of security researchers and appreciate their efforts to keep our network safe.

### Scope

The following assets are **in scope**:

| Asset | Type | Severity |
|-------|------|----------|
| ModernTensor blockchain node (Rust) | Blockchain/DLT | Critical |
| Smart contracts (Solidity) | Smart Contract | Critical |
| SDK (`moderntensor` Python package) | Application | High |
| RPC/API endpoints | API | High |
| CLI tools (`mtcli`) | Application | Medium |
| Website (moderntensor.io) | Web Application | Medium |
| Documentation (docs.moderntensor.io) | Web Application | Low |

The following are **out of scope**:

- Third-party services and integrations not operated by ModernTensor
- Social engineering attacks (e.g., phishing)
- Denial-of-service (DoS/DDoS) attacks against production infrastructure
- Issues in dependencies unless they are directly exploitable in ModernTensor
- Findings from automated scanners without demonstrated impact
- Issues already known or previously reported

### Rewards

| Severity | Description | Reward (USD) |
|----------|-------------|--------------|
| **Critical** | Remote code execution, consensus manipulation, unauthorized token minting, private key extraction | $5,000–$25,000 |
| **High** | Privilege escalation, significant fund loss, data breach, authentication bypass | $2,000–$5,000 |
| **Medium** | Information disclosure, denial-of-service on individual nodes, logic errors with limited impact | $500–$2,000 |
| **Low** | Minor information leakage, best-practice violations, non-exploitable issues | $100–$500 |
| **Informational** | Suggestions, minor improvements, documentation issues | Recognition + swag |

> **Note:** Reward amounts are determined at ModernTensor's sole discretion based on severity, impact, and quality of the report. Exceptional reports may receive bonuses above the stated ranges.

### Responsible Disclosure Timeline

| Step | Timeline |
|------|----------|
| Acknowledgment of report | Within **48 hours** |
| Initial assessment and triage | Within **5 business days** |
| Status update to reporter | Within **10 business days** |
| Fix development and testing | Within **30 days** (critical), **90 days** (others) |
| Public disclosure | After fix is deployed, or **90 days** from report — whichever comes first |
| Reward payment | Within **30 days** after fix is verified |

### Rules of Engagement

- **Do not** access, modify, or delete data belonging to other users
- **Do not** perform attacks that degrade the availability of production services
- **Do not** publicly disclose the vulnerability before the agreed disclosure date
- **Do** provide sufficient detail for us to reproduce the issue
- **Do** make a good-faith effort to avoid privacy violations and data destruction
- **Do** limit testing to your own accounts and test environments where possible
- Researchers acting in good faith under this policy will not face legal action

### How to Report

1. Email your findings to **<security@moderntensor.io>**
2. Encrypt your report using our PGP key (see below)
3. Include: vulnerability type, affected component, reproduction steps, impact assessment, and any proof-of-concept code
4. You will receive an acknowledgment within 48 hours

## Contact

- **Security Email:** <security@moderntensor.io>
- **General Inquiries:** <hello@moderntensor.io>
- **GPG Key:** Available at [moderntensor.io/.well-known/security.txt](https://moderntensor.io/.well-known/security.txt)

### PGP Public Key

```
-----BEGIN PGP PUBLIC KEY BLOCK-----

[PLACEHOLDER — Replace with actual PGP public key before publication]

Fingerprint: XXXX XXXX XXXX XXXX XXXX  XXXX XXXX XXXX XXXX XXXX
Key ID: 0xXXXXXXXX
Email: security@moderntensor.io

-----END PGP PUBLIC KEY BLOCK-----
```

> To generate a real key pair, run:
> ```bash
> gpg --full-generate-key  # Select RSA, 4096 bits
> gpg --armor --export security@moderntensor.io > pgp-public-key.asc
> ```
