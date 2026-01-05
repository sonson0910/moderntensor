# Sơ Đồ Kiến Trúc Chi Tiết - ModernTensor Layer 1 Blockchain

## 1. Kiến Trúc Tổng Quan

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ModernTensor Layer 1 Blockchain                          │
│                         Full Stack Architecture                              │
└─────────────────────────────────────────────────────────────────────────────┘

                              ┌──────────────┐
                              │   User App   │
                              │  (Wallet,    │
                              │   Explorer)  │
                              └──────┬───────┘
                                     │
                        ┌────────────┴────────────┐
                        │                         │
                        ▼                         ▼
                ┌───────────────┐        ┌───────────────┐
                │   JSON-RPC    │        │   GraphQL     │
                │   (Phase 5)   │        │   (Phase 5)   │
                └───────┬───────┘        └───────┬───────┘
                        │                         │
                        └────────────┬────────────┘
                                     │
                          ┌──────────▼──────────┐
                          │      L1Node         │
                          │   (Phase 8 Core)    │
                          │   Orchestrator      │
                          └──────────┬──────────┘
                                     │
        ┌────────────────────────────┼────────────────────────────┐
        │                            │                            │
        ▼                            ▼                            ▼
┌───────────────┐           ┌───────────────┐           ┌───────────────┐
│   Blockchain  │           │   Consensus   │           │    Network    │
│   Layer       │           │   Layer       │           │    Layer      │
│   (Phase 1)   │◄─────────►│   (Phase 2)   │◄─────────►│   (Phase 3)   │
└───────┬───────┘           └───────┬───────┘           └───────┬───────┘
        │                           │                           │
        ▼                           ▼                           ▼
┌───────────────┐           ┌───────────────┐           ┌───────────────┐
│   Storage     │           │  Tokenomics   │           │ Optimization  │
│   Layer       │           │   System      │           │   & Security  │
│   (Phase 4)   │           │               │           │   (Phase 7)   │
└───────────────┘           └───────────────┘           └───────────────┘
```

---

## 2. Data Flow - Transaction Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Transaction Lifecycle                                │
└─────────────────────────────────────────────────────────────────────────────┘

    User                  API Layer           Blockchain Layer        Network
     │                       │                       │                   │
     │  1. Create TX         │                       │                   │
     ├──────────────────────>│                       │                   │
     │                       │                       │                   │
     │                       │  2. Validate & Sign   │                   │
     │                       ├──────────────────────>│                   │
     │                       │                       │                   │
     │                       │  3. Add to Mempool    │                   │
     │                       │      (node.mempool)   │                   │
     │                       │<──────────────────────┤                   │
     │                       │                       │                   │
     │                       │                       │  4. Broadcast TX  │
     │                       │                       ├──────────────────>│
     │                       │                       │                   │
     │                       │                       │  5. Propagate     │
     │                       │                       │<──────────────────┤
     │                       │                       │                   │
     │  6. TX Hash           │                       │                   │
     │<──────────────────────┤                       │                   │
     │                       │                       │                   │
     
                        Consensus Phase
     
                       Consensus Layer           Blockchain Layer
                              │                         │
                              │  7. Select Validator    │
                              │    (PoS Selection)      │
                              ├────────────────────────>│
                              │                         │
                              │  8. Create Block        │
                              │    (from mempool TXs)   │
                              │<────────────────────────┤
                              │                         │
                              │  9. Validate Block      │
                              ├────────────────────────>│
                              │                         │
                              │ 10. Execute TXs         │
                              │    Update State         │
                              │<────────────────────────┤
                              │                         │
                              │ 11. Store Block         │
                              │    (to BlockchainDB)    │
                              │────────────────────────>│
                              │                         │
```

---

## 3. Module Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Module Dependency Structure                            │
└─────────────────────────────────────────────────────────────────────────────┘

Layer 8 (Orchestration):
    ┌─────────────────────────────────────────────────┐
    │              L1Node (testnet/node.py)           │
    │  • Orchestrates all layers                      │
    │  • Manages node lifecycle                       │
    │  • Coordinates block production                 │
    └────────────┬────────────────────────────────────┘
                 │
    ┌────────────┴────────────┐
    │                         │
    ▼                         ▼
┌─────────────┐         ┌─────────────┐
│   Genesis   │         │   Faucet    │
│   Generator │         │   System    │
└──────┬──────┘         └──────┬──────┘
       │                       │
       └───────────┬───────────┘
                   │
                   ▼
Layer 1 (Core):
    ┌─────────────────────────────────────────────────┐
    │           Blockchain Components                 │
    ├─────────────────────────────────────────────────┤
    │  Block ◄──┐                                     │
    │           │                                     │
    │  Transaction ◄──┐                               │
    │           │     │                               │
    │  StateDB  │     │                               │
    │           │     │                               │
    │  Crypto ──┘     │                               │
    │  (KeyPair,      │                               │
    │   MerkleTree)   │                               │
    │           │     │                               │
    │  Validation ────┘                               │
    └────────────┬────────────────────────────────────┘
                 │
    ┌────────────┴────────────┐
    │                         │
    ▼                         ▼
Layer 2 (Consensus):        Layer 4 (Storage):
┌─────────────┐             ┌─────────────┐
│     PoS     │             │ BlockchainDB│
│ ValidatorSet│             │   Indexer   │
│ ForkChoice  │             └─────────────┘
│ AI Validate │
└──────┬──────┘
       │
       ▼
Layer 3 (Network):
┌─────────────┐
│   P2PNode   │
│ SyncManager │
│  Messages   │
└─────────────┘

Supporting Layers:
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│    API      │  │ Tokenomics  │  │Optimization │  │  Security   │
│ (RPC/GQL)   │  │ (Rewards)   │  │ (Perf)      │  │  (Audit)    │
└─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘
```

---

## 4. Component Interaction - Block Production

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Block Production Flow                                  │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────┐
│  Epoch Timer    │
│  (Consensus)    │
└────────┬────────┘
         │
         │ Epoch boundary reached
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│              ProofOfStake.select_validator()                │
│  • Read ValidatorSet                                        │
│  • Calculate stake weights                                  │
│  • VRF-like deterministic selection                         │
│  • Return selected validator                                │
└────────────────────────┬────────────────────────────────────┘
                         │
                         │ Selected: validator_address
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              L1Node.produce_block()                         │
│  1. Get transactions from mempool                           │
│  2. Create block header (height, parent_hash, timestamp)    │
│  3. Execute transactions → update StateDB                   │
│  4. Calculate Merkle roots (tx_root, state_root)            │
│  5. Sign block with validator key                           │
└────────────────────────┬────────────────────────────────────┘
                         │
                         │ New Block created
                         │
         ┌───────────────┼───────────────┐
         │               │               │
         ▼               ▼               ▼
┌────────────────┐ ┌─────────────┐ ┌─────────────┐
│ BlockValidator │ │ BlockchainDB│ │  P2PNode    │
│ .validate()    │ │ .store()    │ │ .broadcast()│
└────────────────┘ └─────────────┘ └─────────────┘
         │               │               │
         │ Valid?        │ Stored        │ Propagated
         │               │               │
         └───────────────┴───────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              ForkChoice.add_block()                         │
│  • Add to block tree                                        │
│  • Update canonical chain (GHOST)                           │
│  • Check finalization                                       │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. State Management Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      State Management System                                │
└─────────────────────────────────────────────────────────────────────────────┘

                        ┌──────────────────┐
                        │    StateDB       │
                        │  (state.py)      │
                        └────────┬─────────┘
                                 │
                ┌────────────────┼────────────────┐
                │                │                │
                ▼                ▼                ▼
        ┌───────────┐    ┌───────────┐   ┌───────────┐
        │   Cache   │    │   Dirty   │   │  Storage  │
        │  (Memory) │    │   Map     │   │ (LevelDB) │
        └───────────┘    └───────────┘   └───────────┘
                │                │                │
                └────────────────┼────────────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │   Account       │
                        │   Structure     │
                        ├─────────────────┤
                        │ • nonce         │
                        │ • balance       │
                        │ • storage_root  │
                        │ • code_hash     │
                        └─────────────────┘

Read Path:
   get_account(address) → Check Cache → Check Dirty → Read Storage → Return

Write Path:
   set_account(address, account) → Update Cache → Mark Dirty → (lazy write)

Commit Path:
   commit() → Write all Dirty to Storage → Clear Dirty → Calculate State Root
```

---

## 6. Network Layer P2P Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      P2P Network Architecture                               │
└─────────────────────────────────────────────────────────────────────────────┘

                        ┌──────────────────┐
                        │  Bootstrap Node  │
                        │  (Well-known)    │
                        └────────┬─────────┘
                                 │
                ┌────────────────┼────────────────┐
                │                │                │
                ▼                ▼                ▼
        ┌───────────┐    ┌───────────┐   ┌───────────┐
        │   Node A  │    │   Node B  │   │   Node C  │
        │ (Validator)│◄──►│ (Validator)│◄─►│  (Full)   │
        └───────────┘    └───────────┘   └───────────┘
                │                │                │
                └────────────────┼────────────────┘
                                 │
                        ┌────────▼────────┐
                        │   P2PNode       │
                        │   (p2p.py)      │
                        ├─────────────────┤
                        │ • Discovery     │
                        │ • Connection    │
                        │ • Messaging     │
                        │ • Sync          │
                        └─────────────────┘
                                 │
                ┌────────────────┼────────────────┐
                │                │                │
                ▼                ▼                ▼
        ┌───────────┐    ┌───────────┐   ┌───────────┐
        │  Block    │    │    TX     │   │   State   │
        │Propagation│    │ Broadcast │   │   Sync    │
        └───────────┘    └───────────┘   └───────────┘

Message Types:
   • PING/PONG        - Heartbeat
   • GET_BLOCKS       - Block request
   • BLOCKS           - Block response
   • NEW_BLOCK        - Block announcement
   • NEW_TX           - Transaction announcement
   • GET_STATE        - State request
```

---

## 7. Consensus Flow - Proof of Stake

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Proof of Stake Consensus Flow                            │
└─────────────────────────────────────────────────────────────────────────────┘

Epoch N:
   ┌─────────────────────────────────────────────────────────┐
   │                  ValidatorSet                           │
   │  ┌────────────────────────────────────────────┐         │
   │  │ Validator A: stake=1000, active=True       │         │
   │  │ Validator B: stake=2000, active=True       │         │
   │  │ Validator C: stake=500,  active=True       │         │
   │  │ Validator D: stake=1500, active=False      │         │
   │  └────────────────────────────────────────────┘         │
   │  Total Active Stake: 3500                               │
   └─────────────────────────────────────────────────────────┘
                           │
                           │ Select validator for slot
                           ▼
   ┌─────────────────────────────────────────────────────────┐
   │           Stake-Weighted Selection                      │
   │  random_value = VRF(epoch, slot, seed)                  │
   │  cumulative_stake:                                      │
   │    [0-1000]   → Validator A (28.6%)                     │
   │    [1000-3000] → Validator B (57.1%)                    │
   │    [3000-3500] → Validator C (14.3%)                    │
   │  selected = find_validator(random_value % 3500)         │
   └─────────────────────────────────────────────────────────┘
                           │
                           │ Validator B selected
                           ▼
   ┌─────────────────────────────────────────────────────────┐
   │              Block Production (Validator B)             │
   │  1. Collect transactions from mempool                   │
   │  2. Create block                                        │
   │  3. Sign with validator key                             │
   │  4. Broadcast to network                                │
   └─────────────────────────────────────────────────────────┘
                           │
                           │ Block N+1
                           ▼
   ┌─────────────────────────────────────────────────────────┐
   │              Block Validation (All Nodes)               │
   │  • Verify signature from Validator B                    │
   │  • Verify Validator B is selected for this slot         │
   │  • Verify Validator B has sufficient stake              │
   │  • Verify block structure and transactions              │
   └─────────────────────────────────────────────────────────┘
                           │
                           │ Valid → Accept
                           ▼
   ┌─────────────────────────────────────────────────────────┐
   │                Reward Distribution                      │
   │  • Calculate block reward                               │
   │  • Add to Validator B's balance                         │
   │  • Update validator stats                               │
   └─────────────────────────────────────────────────────────┘

Epoch Boundary:
   ┌─────────────────────────────────────────────────────────┐
   │              Epoch Processing                           │
   │  • Distribute epoch rewards                             │
   │  • Process slashing (if any)                            │
   │  • Update validator set                                 │
   │  • Finalize blocks (Casper FFG)                         │
   └─────────────────────────────────────────────────────────┘
```

---

## 8. API Layer Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         API Layer Structure                                 │
└─────────────────────────────────────────────────────────────────────────────┘

                        ┌──────────────────┐
                        │   Client Apps    │
                        └────────┬─────────┘
                                 │
                ┌────────────────┼────────────────┐
                │                │                │
                ▼                ▼                ▼
        ┌───────────┐    ┌───────────┐   ┌───────────┐
        │ JSON-RPC  │    │  GraphQL  │   │  REST     │
        │   (rpc.py)│    │(graphql.py│   │ (future)  │
        └─────┬─────┘    └─────┬─────┘   └─────┬─────┘
              │                │               │
              └────────────────┼───────────────┘
                               │
                    ┌──────────▼──────────┐
                    │    API Router       │
                    │   (FastAPI)         │
                    └──────────┬──────────┘
                               │
                ┌──────────────┼──────────────┐
                │              │              │
                ▼              ▼              ▼
        ┌───────────┐  ┌───────────┐  ┌───────────┐
        │ Blockchain│  │  Storage  │  │   State   │
        │   Layer   │  │   Layer   │  │   Layer   │
        └───────────┘  └───────────┘  └───────────┘

JSON-RPC Methods:
   • eth_blockNumber      - Get current block height
   • eth_getBlockByNumber - Get block by height
   • eth_getBlockByHash   - Get block by hash
   • eth_getBalance       - Get account balance
   • eth_sendTransaction  - Submit transaction
   • eth_call             - Execute read-only call

GraphQL Queries:
   • block(height: Int)           - Query block
   • transaction(hash: String)    - Query transaction
   • account(address: String)     - Query account
   • blocks(from: Int, to: Int)   - Query range of blocks
```

---

## 9. Storage Layer Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Storage Layer Design                                 │
└─────────────────────────────────────────────────────────────────────────────┘

                    ┌──────────────────────┐
                    │   BlockchainDB       │
                    │  (blockchain_db.py)  │
                    └──────────┬───────────┘
                               │
                ┌──────────────┼──────────────┐
                │              │              │
                ▼              ▼              ▼
        ┌───────────┐  ┌───────────┐  ┌───────────┐
        │   Block   │  │    TX     │  │   State   │
        │  Storage  │  │  Storage  │  │  Storage  │
        └─────┬─────┘  └─────┬─────┘  └─────┬─────┘
              │              │              │
              └──────────────┼──────────────┘
                             │
                    ┌────────▼────────┐
                    │    LevelDB      │
                    │  (Key-Value)    │
                    └─────────────────┘

Key Prefixes:
   • block:height:{n}       → Block data
   • block:hash:{h}         → Block height lookup
   • tx:hash:{h}            → Transaction data
   • state:account:{addr}   → Account state
   • state:root             → Current state root
   • metadata:height        → Current blockchain height

Indexer (indexer.py):
   ┌─────────────────────────────────────────────────────────┐
   │  • Index blocks by height and hash                      │
   │  • Index transactions by hash                           │
   │  • Index accounts by address                            │
   │  • Maintain metadata (height, best block)               │
   │  • Support fast lookups                                 │
   └─────────────────────────────────────────────────────────┘
```

---

## 10. Complete System Integration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│              ModernTensor L1 - Complete System View                         │
└─────────────────────────────────────────────────────────────────────────────┘

External                API Layer              Core Layers              Storage
  │                        │                        │                     │
  │                        │                        │                     │
┌─┴──────────┐      ┌──────┴──────┐        ┌───────┴────────┐    ┌──────┴─────┐
│  Wallet    │──────►│  JSON-RPC   │───────►│   Blockchain   │───►│  LevelDB   │
│  DApp      │      │  GraphQL    │        │   • Block      │    │  Storage   │
└────────────┘      └─────────────┘        │   • TX         │    └────────────┘
                                            │   • State      │
┌────────────┐      ┌─────────────┐        └───────┬────────┘
│  Explorer  │──────►│  REST API   │                │
│  Monitor   │      │  (Future)   │                │
└────────────┘      └─────────────┘                │
                                                    │
┌────────────┐      ┌─────────────┐        ┌───────▼────────┐    ┌────────────┐
│  Validator │──────►│   P2P       │───────►│   Consensus    │───►│ Validator  │
│  Node      │      │  Network    │        │   • PoS        │    │   Set      │
└────────────┘      └─────────────┘        │   • ForkChoice │    └────────────┘
                                            │   • AI Validate│
                            ┌───────────────┴────────────────┘
                            │
                    ┌───────▼────────┐
                    │   Tokenomics   │
                    │   • Rewards    │
                    │   • Emissions  │
                    │   • Burning    │
                    └────────────────┘

All Orchestrated by L1Node (testnet/node.py)
```

---

**Tài liệu này mô tả kiến trúc chi tiết của blockchain ModernTensor Layer 1**  
**Ngày cập nhật:** 5 Tháng 1, 2026
