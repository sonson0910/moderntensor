# ModernTensor Legal Structure

> **Document Status:** Draft — For Internal & Investor Review
> **Last Updated:** February 2026
> **Classification:** Confidential

---

## Executive Summary

This document outlines the recommended multi-entity legal structure for the ModernTensor project. The structure is designed to maximize regulatory compliance, protect intellectual property, facilitate fundraising, and support token issuance — while maintaining operational flexibility across key jurisdictions.

---

## Recommended Entity Structure

### 1. ModernTensor Foundation (Singapore)

- **Entity Type**: Company Limited by Guarantee (CLG) or Foundation Company
- **Jurisdiction**: Singapore (crypto-friendly, clear regulatory framework)
- **Purpose**:
  - Govern protocol development and ecosystem direction
  - Manage DAO treasury and grant programs
  - Employ core developers and researchers
  - Hold intellectual property (IP) and open-source licensing
  - Serve as the public-facing entity for partnerships and governance
- **Regulatory**: Falls under Singapore's Payment Services Act (PSA) — may require a Major Payment Institution (MPI) license or exemption depending on token classification
- **Board Composition**: Recommended 3–5 directors including at least one independent Singapore-resident director

### 2. ModernTensor Labs (Development Company)

- **Entity Type**: Pte. Ltd. (Private Limited Company)
- **Jurisdiction**: Singapore or Vietnam
- **Purpose**:
  - Core development team employment and contractor management
  - Receive equity investment funding (Pre-Seed, Seed, Series A)
  - Commercial partnerships and enterprise licensing
  - Product development and go-to-market execution
- **Structure**: Subsidiary of, or contracted by, the Foundation
- **Investment Vehicle**: This entity accepts SAFE/equity investments from VCs
- **Vietnam Option**: If headquartered in Vietnam, lower operational costs (~40–60% reduction) while maintaining Singapore Foundation as the holding entity

### 3. Token Issuance Vehicle

- **Entity Type**: BVI Business Company or Cayman Islands Exempted Company
- **Jurisdiction**: British Virgin Islands (BVI) or Cayman Islands
- **Purpose**:
  - Issue MDT tokens via Token Generation Event (TGE)
  - Conduct Initial DEX Offering (IDO) or launchpad sale
  - Manage token vesting smart contracts
  - Hold pre-allocated token reserves (ecosystem, team, treasury)
- **Why BVI/Cayman**:
  - Favorable token issuance regulations with legal precedent
  - No capital gains tax on token transactions
  - Internationally recognized legal jurisdiction
  - Common structure used by Ethereum Foundation, Solana Foundation, etc.
- **Compliance**: Must still adhere to KYC/AML requirements for token sale participants

### Entity Relationship Diagram

```
┌─────────────────────────────────────────┐
│       ModernTensor Foundation           │
│       (Singapore CLG)                   │
│                                         │
│  • Protocol governance                  │
│  • IP ownership                         │
│  • DAO treasury management              │
│  • Grant programs                       │
└────────────┬───────────────┬────────────┘
             │               │
    Service  │               │  Token
    Agreement│               │  Management
             │               │  Agreement
             ▼               ▼
┌────────────────┐  ┌────────────────────┐
│ ModernTensor   │  │  Token Issuance    │
│ Labs Pte. Ltd. │  │  Vehicle (BVI)     │
│ (SG/VN)        │  │                    │
│                │  │  • MDT token       │
│ • Development  │  │    issuance        │
│ • Operations   │  │  • TGE/IDO        │
│ • Fundraising  │  │  • Vesting mgmt   │
└────────────────┘  └────────────────────┘
```

---

## Legal Checklist (Pre-Fundraising)

| # | Item | Status | Priority | Est. Timeline |
|---|------|--------|----------|---------------|
| 1 | Register Foundation entity (Singapore CLG) | ☐ | **CRITICAL** | 4–6 weeks |
| 2 | Register Development company (Pte. Ltd.) | ☐ | **CRITICAL** | 2–4 weeks |
| 3 | Register Token Issuance Vehicle (BVI) | ☐ | **CRITICAL** | 2–3 weeks |
| 4 | Draft Token SAFTs (Simple Agreement for Future Tokens) | ☐ | **CRITICAL** | 2–3 weeks |
| 5 | Draft SAFEs (Simple Agreement for Future Equity) | ☐ | **CRITICAL** | 1–2 weeks |
| 6 | Legal opinion on MDT token classification | ☐ | **HIGH** | 3–4 weeks |
| 7 | KYC/AML compliance framework & provider integration | ☐ | **HIGH** | 2–4 weeks |
| 8 | Terms of Service (testnet & mainnet) | ☐ | **HIGH** | 1–2 weeks |
| 9 | Privacy Policy (GDPR/PDPA compliant) | ☐ | **HIGH** | 1–2 weeks |
| 10 | Token holder agreement / Token purchase terms | ☐ | **MEDIUM** | 2–3 weeks |
| 11 | IP assignment agreements (founders → Foundation) | ☐ | **MEDIUM** | 1–2 weeks |
| 12 | Employee/contractor agreements (ESOP, vesting) | ☐ | **MEDIUM** | 1–2 weeks |
| 13 | Data processing agreements (DPA) | ☐ | **LOW** | 1 week |
| 14 | Insurance coverage (D&O, cyber liability) | ☐ | **LOW** | 2–4 weeks |

---

## Regulatory Considerations

### Token Classification

- MDT is primarily a **utility token** used for:
  - Gas fees on the ModernTensor network
  - Staking by validators and miners
  - Payment for decentralized AI compute services
  - Governance voting within the DAO
- In most jurisdictions, utility tokens with genuine network functionality have lighter regulatory requirements than securities
- **However**, the following features may attract securities classification in certain jurisdictions:
  - Emission rewards distributed to validators/miners (passive income)
  - Staking yields (may be considered investment returns)
  - Token vesting schedules for team/investors (expectation of profit)
- **Recommendation**: Obtain a formal legal opinion from a qualified crypto law firm in Singapore (e.g., Dentons Rodyk, Rajah & Tann) before any token sale

### Howey Test Analysis (US)

| Howey Factor | MDT Assessment | Risk |
|---|---|---|
| Investment of money | Token purchase = yes | Medium |
| Common enterprise | Network participants = yes | Medium |
| Expectation of profit | Utility use cases reduce this | Low–Medium |
| Efforts of others | Decentralized governance mitigates | Low |

**Conclusion**: MDT has strong utility characteristics but should be treated conservatively. Exclude US persons from token sale to mitigate SEC risk.

### Key Jurisdictions

| Jurisdiction | Risk Level | Regulatory Body | Notes |
|---|---|---|---|
| **Singapore** | Low | MAS (Monetary Authority of Singapore) | Crypto-friendly, Payment Services Act (PSA), clear licensing framework |
| **USA** | High | SEC, CFTC, FinCEN | Active enforcement — exclude US persons from token sale |
| **EU** | Medium | ESMA (via MiCA) | Markets in Crypto-Assets (MiCA) regulation fully applicable from 2025 |
| **UK** | Medium | FCA | Financial promotions regime — restrict UK marketing |
| **Vietnam** | Medium | SBV (State Bank of Vietnam) | Evolving regulations — crypto not legally recognized as payment but not banned |
| **Japan** | Medium–High | FSA/JFSA | Strict licensing; may require registration as a crypto-asset exchange |
| **UAE** | Low | VARA (Dubai) | Progressive regulatory framework, attractive for regional operations |
| **Hong Kong** | Medium | SFC, HKMA | New licensing regime (2024); potential for regulated token sales |

---

## Investment Structure

### Pre-Seed / Seed Round

- **Instrument**: SAFE (Simple Agreement for Future Equity) into ModernTensor Labs
- **Token Side Letter**: Additional SAFT granting token allocation at TGE
- **Standard Terms**:
  - Valuation cap: To be determined
  - Discount rate: 15–25% (typical for seed stage)
  - Pro-rata rights: Yes
  - Most Favored Nation (MFN): Yes
  - Information rights: Quarterly financial updates

### Token Sale (TGE)

- **Instrument**: SAFT (Simple Agreement for Future Tokens) from Token Issuance Vehicle
- **Vesting**: 12-month cliff, 24–36 month linear vesting for investor allocations
- **Lock-up**: 6–12 months post-TGE for strategic partners
- **KYC/AML**: Required for all participants
- **Geo-restrictions**: US, China, and other restricted jurisdictions excluded

---

## Intellectual Property Strategy

### IP Ownership

| Asset | Owner | License |
|---|---|---|
| Core protocol (blockchain) | ModernTensor Foundation | Apache 2.0 / MIT |
| SDK & developer tools | ModernTensor Foundation | Apache 2.0 |
| ModernTensor brand & trademarks | ModernTensor Foundation | Proprietary |
| Commercial applications | ModernTensor Labs | Proprietary (optional) |
| Research publications | ModernTensor Foundation | CC BY 4.0 |

### IP Assignment Flow

1. All founders assign pre-existing project IP to the Foundation
2. All employees/contractors sign IP assignment agreements
3. Foundation licenses IP back to Labs under a service agreement
4. Open-source code remains under permissive licenses (Apache 2.0 / MIT)

---

## Estimated Costs

| Item | Cost (USD) | Frequency |
|---|---|---|
| Singapore CLG registration | $3,000–5,000 | One-time |
| Singapore Pte. Ltd. registration | $1,500–3,000 | One-time |
| BVI company registration | $2,000–3,000 | One-time |
| Registered agent (BVI) | $1,500–2,500 | Annual |
| Legal opinion (token classification) | $10,000–25,000 | One-time |
| SAFT/SAFE drafting | $5,000–10,000 | One-time |
| KYC/AML provider setup | $2,000–5,000 | One-time |
| Terms of Service & Privacy Policy | $3,000–5,000 | One-time |
| Annual compliance (Singapore) | $5,000–10,000 | Annual |
| Annual audit (Singapore) | $3,000–8,000 | Annual |
| Corporate secretary (Singapore) | $1,200–2,400 | Annual |
| **Total initial setup** | **$28,200–68,500** | — |
| **Annual recurring** | **$10,700–22,900** | — |

> **Note:** Costs are estimates based on 2025–2026 market rates. Actual costs may vary based on complexity and chosen service providers.

---

## Recommended Law Firms (Crypto-Specialized)

### Singapore
- **Dentons Rodyk** — Full-service, strong blockchain practice
- **Rajah & Tann** — Leading regional firm with dedicated crypto/fintech team
- **Ellipsis** — Boutique crypto-focused firm (cost-effective for startups)
- **Pinsent Masons MPillay** — International firm with Singapore crypto expertise

### BVI / Cayman Islands
- **Harneys** — Market leader for BVI/Cayman token structures
- **Carey Olsen** — Strong offshore corporate practice
- **Walkers** — Established offshore firm with crypto clients

### Vietnam
- **VILAF** — Top-tier Vietnamese law firm
- **Baker McKenzie Vietnam** — International firm with local expertise
- **Tilleke & Gibbins** — Regional firm with Vietnam presence

### Global / Multi-Jurisdictional
- **DLA Piper** — Global coverage with dedicated Web3 practice
- **CMS** — Strong European and Asian blockchain team
- **Latham & Watkins** — Top-tier for institutional-grade token offerings

---

## Tax Considerations

### Singapore Foundation
- Corporate tax rate: 17% (with startup exemptions for first 3 years)
- No capital gains tax
- Goods & Services Tax (GST): Digital token supply may be exempt
- Tax incentive schemes available for tech companies

### BVI Token Issuance Vehicle
- No corporate income tax
- No capital gains tax
- No withholding tax
- Annual government fees: ~$1,600 (for authorized capital up to $50,000)

### Team Taxation
- Founders and employees taxed in their country of residence
- Token grants may be taxable as employment income at vesting
- **Recommendation**: Each team member should seek personal tax advice

---

## Timeline

| Phase | Activities | Target |
|---|---|---|
| **Phase 1** (Month 1–2) | Register Singapore Foundation & Labs entity, engage legal counsel | Q1 2026 |
| **Phase 2** (Month 2–3) | Obtain legal opinion on token classification, draft SAFT/SAFE | Q1 2026 |
| **Phase 3** (Month 3–4) | Register BVI entity, finalize KYC/AML framework, ToS & Privacy Policy | Q2 2026 |
| **Phase 4** (Month 4–5) | IP assignment, employee agreements, pre-fundraising legal review | Q2 2026 |
| **Phase 5** (Month 5–6) | Commence fundraising with full legal infrastructure in place | Q2–Q3 2026 |

---

## Disclaimer

This document is for informational and planning purposes only. It does not constitute legal, tax, or financial advice. ModernTensor and its affiliates recommend engaging qualified legal counsel in all relevant jurisdictions before taking any action described herein. Regulatory landscapes for digital assets are evolving rapidly; all information should be verified with current legal guidance at the time of execution.

---

*Document prepared for ModernTensor Foundation — February 2026*
