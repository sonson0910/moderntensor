# ModernTensor FAQ

## General Questions

### What is ModernTensor?
ModernTensor is a custom Layer 1 blockchain designed specifically for decentralized AI workloads. It creates a permissionless marketplace where AI model providers can earn rewards for quality contributions, and users can access decentralized AI compute.

### How is ModernTensor different from Bittensor?
| Aspect | ModernTensor | Bittensor |
|--------|--------------|-----------|
| Blockchain | Custom L1 (Rust) | Substrate (Polkadot) |
| Performance | 1,000-5,000 TPS | ~100 TPS |
| Token Emission | Adaptive | Fixed 7,200/day |
| Entry Barrier | 0 tokens | ~1,000+ TAO |
| Burns | 4 mechanisms | None |
| Smart Contracts | Full EVM | Limited |

### When is mainnet launching?
Mainnet is scheduled for Q1 2026, approximately 2 months from now. The testnet is currently live.

### What is $MDT?
MDT (ModernTensor Token) is the native cryptocurrency of the ModernTensor network. It's used for staking, governance, paying for AI services, and earning rewards.

---

## Technology Questions

### Why Rust instead of Python/JavaScript?
Rust provides:
- **Memory safety** without garbage collection
- **10x performance** improvement over interpreted languages
- **Fearless concurrency** for parallel processing
- **Growing ecosystem** for blockchain and cryptography

### What is LuxTensor?
LuxTensor is the Rust implementation of the ModernTensor blockchain. It consists of 11 modular crates handling core functionality, consensus, networking, storage, and smart contracts.

### Is ModernTensor EVM compatible?
Yes. ModernTensor supports full EVM compatibility, allowing Ethereum developers to deploy existing smart contracts with minimal modifications.

### What consensus mechanism does ModernTensor use?
ModernTensor uses Proof of Stake (PoS) combined with an AI validation layer. Validators stake MDT tokens and participate in both block production and AI model quality evaluation.

---

## Token Questions

### What is the total supply of $MDT?
The maximum supply is 21,000,000 MDT. Unlike many projects, no tokens are pre-minted — all tokens are created through network emission based on utility.

### How does adaptive emission work?
```
MintAmount = BaseReward × UtilityScore × QualityMultiplier × HalvingFactor
```
- High network activity = more emission
- Low network activity = less emission
- Halves approximately every 4 years

### What are the burn mechanisms?
1. **Transaction Fees** — 50% of base fee burned
2. **Subnet Registration** — 100 MDT burned per subnet
3. **Unmet Quota** — 100 MDT burned when quality thresholds missed
4. **Slashing** — 50% of slashed stake burned

### How much can I earn from staking?
| Tier | MDT Required | APY Range |
|------|--------------|-----------|
| Full Node | 10,000 | 6-12% |
| Validator | 100,000 | 10-18% |
| Super Validator | 500,000 | 15-25% |

---

## Participation Questions

### How do I become a validator?
1. Stake minimum 100,000 MDT
2. Run validator node software
3. Participate in consensus
4. Earn staking rewards

### How do I become a miner (AI provider)?
1. Register an AI model on a subnet
2. Meet quality thresholds set by validators
3. Earn rewards based on performance scores

### Can I participate without tokens?
Yes! Unlike Bittensor, ModernTensor has zero entry barriers:
- Run a light node with 0 MDT
- Develop DApps on the platform
- Contribute to open-source development

### Where can I get testnet tokens?
Testnet faucet is available. Check the documentation for the faucet URL.

---

## Development Questions

### Is there an SDK?
Yes. The Python SDK provides 144+ RPC methods for interacting with the blockchain. Install via pip:
```bash
pip install moderntensor
```

### Where is the documentation?
- **GitHub:** github.com/sonson0910/moderntensor
- **Docs folder:** /docs in the repository
- **Whitepaper:** WHITEPAPER.md

### How can I contribute?
1. Fork the repository
2. Check open issues
3. Submit pull requests
4. Join community discussions

---

## Business Questions

### Is there an investment opportunity?
ModernTensor is raising a $2-3M seed round. Contact the team for investment inquiries.

### How can I partner with ModernTensor?
Partnership inquiries welcome. We're looking for:
- AI model providers
- Compute infrastructure partners
- Exchange partners
- Developer tool integrations

### What is the token distribution?
| Allocation | Percentage |
|------------|------------|
| Emission Rewards | 45% |
| Ecosystem Grants | 12% |
| Team (4yr vest) | 10% |
| Treasury | 10% |
| Private Sale (2yr vest) | 8% |
| Foundation | 5% |
| Liquidity | 5% |
| IDO | 5% |

---

## Support

### Where can I get help?
- **GitHub Issues:** For technical problems
- **Discord:** For community support (coming soon)
- **Documentation:** Comprehensive guides available

### How do I report bugs?
Submit issues on GitHub with:
1. Description of the problem
2. Steps to reproduce
3. Expected vs actual behavior
4. System information
