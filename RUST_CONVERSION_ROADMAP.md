# LuxTensor: Lộ Trình Chuyển Đổi ModernTensor Layer 1 sang Rust

**Ngày tạo:** 6 Tháng 1, 2026  
**Trạng thái:** Kế hoạch chuyển đổi  
**Thời gian ước tính:** 6-9 tháng  
**Nguồn lực:** Team 3-4 Rust engineers  
**Tên dự án:** LuxTensor (viết bằng Rust)

---

## 📋 Tổng Quan Dự Án

### Mục Tiêu
Chuyển đổi ModernTensor Layer 1 blockchain từ Python sang Rust để đạt được:
- **Performance:** Tăng tốc độ xử lý 10-100x
- **Safety:** Type safety và memory safety từ Rust
- **Concurrency:** Tận dụng Rust's fearless concurrency
- **Production:** Sẵn sàng cho mainnet với performance cao
- **Efficiency:** Giảm resource usage (CPU, memory)

### Tại Sao Chuyển Sang Rust?

**Ưu điểm:**
- ✅ **Performance:** Gần như C/C++ performance
- ✅ **Safety:** No null pointers, no data races
- ✅ **Concurrency:** Fearless concurrent programming
- ✅ **Ecosystem:** Excellent blockchain libraries (libp2p, tokio, serde)
- ✅ **Community:** Strong blockchain community (Polkadot, Solana, Near)
- ✅ **Memory:** Zero-cost abstractions, no GC pauses

**So sánh Python vs Rust:**
| Aspect | Python (ModernTensor) | Rust (LuxTensor) |
|--------|----------------------|------------------|
| Performance | Baseline | 10-100x faster |
| Memory Safety | Runtime errors | Compile-time guarantees |
| Concurrency | GIL limitations | True parallelism |
| Type Safety | Dynamic | Static with inference |
| Blockchain Examples | Minimal | Polkadot, Solana, Near |

### Hiện Trạng ModernTensor (Python)

**Đã có sẵn (~9,715 LOC Python):**
- ✅ Core Blockchain (block, transaction, state, validation)
- ✅ Consensus Layer (PoS, fork choice, rewards)
- ✅ Network Layer (P2P, sync, messages)
- ✅ Storage Layer (blockchain DB, state DB, indexer)
- ✅ API Layer (JSON-RPC, GraphQL)
- ✅ Testing suite (71 tests passing)
- ✅ Testnet infrastructure

**Modules cần convert:**
```
sdk/blockchain/     (~1,865 LOC) - Core blockchain primitives
sdk/consensus/      (~1,100 LOC) - PoS consensus mechanism
sdk/network/        (~1,550 LOC) - P2P networking
sdk/storage/        (~850 LOC)   - Database and indexing
sdk/api/            (~1,200 LOC) - RPC and GraphQL APIs
sdk/crypto/         (embedded)   - Cryptographic operations
```

**Total:** ~9,715 LOC Python → ~15,000-20,000 LOC Rust (estimate)

---

## 🗺️ Lộ Trình Chi Tiết

### Phase 0: Chuẩn Bị & Thiết Kế (Tuần 1-3)

**Mục tiêu:** Setup project và quyết định kiến trúc

#### 0.1 Setup Rust Project Structure

**Cargo workspace structure:**
```
luxtensor/
├── Cargo.toml              # Workspace root
├── README.md
├── LICENSE
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── crates/
│   ├── luxtensor-core/     # Core blockchain primitives
│   ├── luxtensor-consensus/ # PoS consensus
│   ├── luxtensor-network/   # P2P networking
│   ├── luxtensor-storage/   # Database layer
│   ├── luxtensor-api/       # RPC/API server
│   ├── luxtensor-crypto/    # Cryptography
│   ├── luxtensor-types/     # Shared types
│   ├── luxtensor-node/      # Node binary
│   └── luxtensor-cli/       # CLI tools
├── tests/
│   ├── integration/
│   └── benchmarks/
└── docs/
    ├── architecture.md
    └── conversion-notes.md
```

**Workspace Cargo.toml:**
```toml
[workspace]
members = [
    "crates/luxtensor-core",
    "crates/luxtensor-consensus",
    "crates/luxtensor-network",
    "crates/luxtensor-storage",
    "crates/luxtensor-api",
    "crates/luxtensor-crypto",
    "crates/luxtensor-types",
    "crates/luxtensor-node",
    "crates/luxtensor-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["LuxTensor Team"]

[workspace.dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"

# Networking
libp2p = "0.53"

# Database
rocksdb = "0.21"
sled = "0.34"

# Cryptography
ed25519-dalek = "2.0"
sha3 = "0.10"
blake3 = "1.5"

# Utilities
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
```

**Thời gian:** 1 tuần  
**Output:** Project scaffolding, CI/CD setup

---

#### 0.2 Quyết Định Technical Stack

**Core Dependencies:**

1. **Async Runtime:** `tokio` ⭐
   - Industry standard cho async Rust
   - Excellent performance và ecosystem
   - Dùng cho: Network, consensus, RPC

2. **Serialization:** `serde` + `bincode`
   - `serde`: Universal serialization framework
   - `bincode`: Binary format cho blockchain data
   - `serde_json`: JSON-RPC API

3. **Networking:** `libp2p` ⭐
   - Battle-tested (Polkadot, IPFS)
   - Built-in peer discovery, NAT traversal
   - Support cho custom protocols

4. **Storage:** `RocksDB` hoặc `sled`
   - `RocksDB`: Proven cho blockchain (Ethereum, Bitcoin)
   - `sled`: Pure Rust, simpler API
   - **Khuyến nghị:** RocksDB cho production

5. **Cryptography:**
   - `ed25519-dalek`: Fast EdDSA signatures
   - `sha3` / `blake3`: Hashing
   - `curve25519-dalek`: Elliptic curves

6. **Consensus:** Custom implementation
   - Học từ: `tendermint-rs`, `lighthouse`
   - Adapt PoS từ Python version

**Thời gian:** 1 tuần  
**Output:** Technical design document

---

#### 0.3 Mapping Python → Rust

**Translation Strategy:**

```python
# Python (ModernTensor)
@dataclass
class Block:
    height: int
    timestamp: int
    previous_hash: bytes
    transactions: List[Transaction]
```

```rust
// Rust (LuxTensor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub height: u64,
    pub timestamp: u64,
    pub previous_hash: Hash,
    pub transactions: Vec<Transaction>,
}
```

**Key Differences:**
- Python `List` → Rust `Vec`
- Python `bytes` → Rust `Vec<u8>` or custom `Hash` type
- Python `@dataclass` → Rust `#[derive(Debug, Clone, Serialize)]`
- Python exceptions → Rust `Result<T, Error>`

**Thời gian:** 1 tuần  
**Output:** Conversion guide document

---

### Phase 1: Core Blockchain (Tháng 1-2)

**Priority:** 🔴 CRITICAL  
**Timeline:** 8 tuần  
**Team:** 2 Rust engineers

#### 1.1 Crate: `luxtensor-types` (Tuần 1-2)

**Shared types và primitives:**

```rust
// crates/luxtensor-types/src/lib.rs

/// 32-byte hash
pub type Hash = [u8; 32];

/// 20-byte address
pub type Address = [u8; 20];

/// Block height
pub type BlockHeight = u64;

/// Custom error type
#[derive(Debug, thiserror::Error)]
pub enum LuxTensorError {
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Insufficient balance")]
    InsufficientBalance,
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    // ... more errors
}

pub type Result<T> = std::result::Result<T, LuxTensorError>;
```

**Thời gian:** 2 tuần  
**Output:** ~500 LOC

---

#### 1.2 Crate: `luxtensor-crypto` (Tuần 2-3)

**Cryptographic operations:**

```rust
// crates/luxtensor-crypto/src/keypair.rs

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer, Verifier};
use sha3::{Digest, Keccak256};

pub struct KeyPair {
    keypair: Keypair,
}

impl KeyPair {
    /// Generate new random keypair
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        Self { keypair }
    }
    
    /// Sign message
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.keypair.sign(message)
    }
    
    /// Verify signature
    pub fn verify(
        message: &[u8],
        signature: &Signature,
        public_key: &PublicKey
    ) -> bool {
        public_key.verify(message, signature).is_ok()
    }
    
    /// Derive address from public key
    pub fn address(&self) -> Address {
        let public_key_bytes = self.keypair.public.as_bytes();
        let hash = Keccak256::digest(public_key_bytes);
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[12..32]);
        address
    }
}

// Merkle tree implementation
pub struct MerkleTree {
    leaves: Vec<Hash>,
    root: Hash,
}

impl MerkleTree {
    pub fn new(leaves: Vec<Hash>) -> Self {
        let root = Self::compute_root(&leaves);
        Self { leaves, root }
    }
    
    pub fn root(&self) -> Hash {
        self.root
    }
    
    fn compute_root(leaves: &[Hash]) -> Hash {
        // Recursive merkle root computation
        // ...
    }
}
```

**Thời gian:** 2 tuần  
**Output:** ~1,000 LOC

---

#### 1.3 Crate: `luxtensor-core` (Tuần 3-6)

**Core blockchain primitives:**

```rust
// crates/luxtensor-core/src/block.rs

use luxtensor_types::{Hash, BlockHeight, Result};
use luxtensor_crypto::MerkleTree;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub height: BlockHeight,
    pub timestamp: u64,
    pub previous_hash: Hash,
    pub state_root: Hash,
    pub txs_root: Hash,
    pub receipts_root: Hash,
    pub validator: Address,
    pub signature: Signature,
    pub gas_used: u64,
    pub gas_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Calculate block hash
    pub fn hash(&self) -> Hash {
        let encoded = bincode::serialize(&self.header).unwrap();
        let mut hasher = Keccak256::new();
        hasher.update(&encoded);
        hasher.finalize().into()
    }
    
    /// Validate block structure
    pub fn validate(&self) -> Result<()> {
        // Validate header
        // Validate transaction count
        // Validate merkle roots
        // ...
        Ok(())
    }
}
```

```rust
// crates/luxtensor-core/src/transaction.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub nonce: u64,
    pub from: Address,
    pub to: Option<Address>, // None for contract creation
    pub value: u128,
    pub gas_price: u64,
    pub gas_limit: u64,
    pub data: Vec<u8>,
    pub v: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
}

impl Transaction {
    /// Calculate transaction hash
    pub fn hash(&self) -> Hash {
        let encoded = bincode::serialize(self).unwrap();
        Keccak256::digest(&encoded).into()
    }
    
    /// Verify signature
    pub fn verify_signature(&self) -> Result<()> {
        // Reconstruct message
        // Recover public key from (v, r, s)
        // Verify signature
        // ...
        Ok(())
    }
    
    /// Extract sender address
    pub fn sender(&self) -> Result<Address> {
        // Recover from signature
        // ...
    }
}
```

```rust
// crates/luxtensor-core/src/state.rs

use rocksdb::{DB, Options};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub nonce: u64,
    pub balance: u128,
    pub storage_root: Hash,
    pub code_hash: Hash,
}

pub struct StateDB {
    db: Arc<DB>,
    cache: Arc<RwLock<HashMap<Address, Account>>>,
}

impl StateDB {
    pub fn new(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        
        Ok(Self {
            db: Arc::new(db),
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Get account
    pub fn get_account(&self, address: &Address) -> Result<Account> {
        // Check cache first
        if let Some(account) = self.cache.read().unwrap().get(address) {
            return Ok(account.clone());
        }
        
        // Load from DB
        let key = format!("account:{}", hex::encode(address));
        let value = self.db.get(key.as_bytes())?
            .ok_or(LuxTensorError::AccountNotFound)?;
        
        let account: Account = bincode::deserialize(&value)?;
        self.cache.write().unwrap().insert(*address, account.clone());
        
        Ok(account)
    }
    
    /// Update account
    pub fn set_account(&self, address: &Address, account: Account) -> Result<()> {
        self.cache.write().unwrap().insert(*address, account.clone());
        Ok(())
    }
    
    /// Commit changes to disk
    pub fn commit(&self) -> Result<Hash> {
        let cache = self.cache.read().unwrap();
        
        for (address, account) in cache.iter() {
            let key = format!("account:{}", hex::encode(address));
            let value = bincode::serialize(account)?;
            self.db.put(key.as_bytes(), value)?;
        }
        
        // Calculate state root
        let state_root = self.calculate_state_root()?;
        
        Ok(state_root)
    }
}
```

```rust
// crates/luxtensor-core/src/validation.rs

pub struct BlockValidator {
    state: Arc<StateDB>,
    config: ChainConfig,
}

impl BlockValidator {
    pub fn new(state: Arc<StateDB>, config: ChainConfig) -> Self {
        Self { state, config }
    }
    
    /// Validate complete block
    pub fn validate_block(&self, block: &Block) -> Result<()> {
        // 1. Validate block structure
        block.validate()?;
        
        // 2. Verify previous hash
        // 3. Check timestamp
        // 4. Verify validator signature
        // 5. Validate all transactions
        for tx in &block.transactions {
            self.validate_transaction(tx)?;
        }
        
        // 6. Check state root
        // 7. Verify gas usage
        
        Ok(())
    }
    
    /// Validate single transaction
    pub fn validate_transaction(&self, tx: &Transaction) -> Result<()> {
        // 1. Verify signature
        tx.verify_signature()?;
        
        // 2. Check nonce
        let sender = tx.sender()?;
        let account = self.state.get_account(&sender)?;
        if tx.nonce != account.nonce {
            return Err(LuxTensorError::InvalidNonce);
        }
        
        // 3. Check balance
        let total_cost = tx.value + (tx.gas_price * tx.gas_limit);
        if account.balance < total_cost {
            return Err(LuxTensorError::InsufficientBalance);
        }
        
        Ok(())
    }
    
    /// Execute transaction
    pub async fn execute_transaction(
        &self,
        tx: &Transaction
    ) -> Result<TransactionReceipt> {
        // 1. Deduct gas
        // 2. Transfer value
        // 3. Execute contract code if any
        // 4. Update state
        // 5. Generate receipt
        
        todo!()
    }
}
```

**Thời gian:** 4 tuần  
**Output:** ~3,000 LOC

---

#### 1.4 Testing Suite (Tuần 6-8)

**Unit tests và integration tests:**

```rust
// crates/luxtensor-core/tests/block_tests.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_block_hash() {
        let block = create_test_block();
        let hash = block.hash();
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_block_validation() {
        let block = create_test_block();
        assert!(block.validate().is_ok());
    }
    
    #[tokio::test]
    async fn test_transaction_execution() {
        let state = create_test_state();
        let validator = BlockValidator::new(state, ChainConfig::default());
        let tx = create_test_transaction();
        
        let receipt = validator.execute_transaction(&tx).await.unwrap();
        assert!(receipt.success);
    }
}
```

**Benchmarks:**

```rust
// crates/luxtensor-core/benches/bench.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_block_hash(c: &mut Criterion) {
    let block = create_test_block();
    c.bench_function("block_hash", |b| {
        b.iter(|| black_box(block.hash()))
    });
}

criterion_group!(benches, benchmark_block_hash);
criterion_main!(benches);
```

**Thời gian:** 2 tuần  
**Output:** ~1,000 LOC tests, benchmarks

---

### Phase 2: Consensus Layer (Tháng 2-3)

**Priority:** 🔴 CRITICAL  
**Timeline:** 6 tuần  
**Team:** 2 Rust engineers

#### 2.1 Crate: `luxtensor-consensus` (Tuần 1-4)

**PoS consensus implementation:**

```rust
// crates/luxtensor-consensus/src/pos.rs

use luxtensor_core::{Block, StateDB};
use luxtensor_types::{Address, Result};

pub struct ProofOfStake {
    state: Arc<StateDB>,
    config: ConsensusConfig,
    validator_set: Arc<RwLock<ValidatorSet>>,
}

impl ProofOfStake {
    pub fn new(
        state: Arc<StateDB>,
        config: ConsensusConfig
    ) -> Self {
        Self {
            state,
            config,
            validator_set: Arc::new(RwLock::new(ValidatorSet::new())),
        }
    }
    
    /// Select validator for slot using VRF
    pub fn select_validator(&self, slot: u64) -> Result<Address> {
        let validators = self.validator_set.read().unwrap();
        
        // Use VRF (Verifiable Random Function)
        // Weight by stake amount
        let total_stake = validators.total_stake();
        let random_point = self.vrf_output(slot) % total_stake;
        
        let mut cumulative = 0u128;
        for validator in validators.iter() {
            cumulative += validator.stake;
            if cumulative >= random_point as u128 {
                return Ok(validator.address);
            }
        }
        
        unreachable!("Should always select a validator")
    }
    
    /// Validate block producer
    pub fn validate_block_producer(
        &self,
        block: &Block,
        slot: u64
    ) -> Result<()> {
        let expected = self.select_validator(slot)?;
        if block.header.validator != expected {
            return Err(LuxTensorError::InvalidBlockProducer);
        }
        Ok(())
    }
    
    /// Process epoch transition
    pub async fn process_epoch(&self) -> Result<()> {
        // 1. Calculate rewards
        // 2. Process slashing
        // 3. Update validator set
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Validator {
    pub address: Address,
    pub stake: u128,
    pub commission: u8,
    pub active: bool,
}

pub struct ValidatorSet {
    validators: HashMap<Address, Validator>,
}

impl ValidatorSet {
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
        }
    }
    
    pub fn add_validator(&mut self, validator: Validator) {
        self.validators.insert(validator.address, validator);
    }
    
    pub fn total_stake(&self) -> u128 {
        self.validators.values()
            .map(|v| v.stake)
            .sum()
    }
}
```

**Thời gian:** 3 tuần  
**Output:** ~2,000 LOC

---

#### 2.2 Fork Choice (Tuần 4-5)

```rust
// crates/luxtensor-consensus/src/fork_choice.rs

use std::collections::HashMap;

pub struct ForkChoice {
    blocks: HashMap<Hash, Block>,
    head: Option<Hash>,
}

impl ForkChoice {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            head: None,
        }
    }
    
    /// Add block to fork choice
    pub fn add_block(&mut self, block: Block) {
        let hash = block.hash();
        self.blocks.insert(hash, block);
        self.update_head();
    }
    
    /// Update canonical head using GHOST
    fn update_head(&mut self) {
        // Greedy Heaviest Observed SubTree (GHOST)
        // Find block with most descendants
        
        if let Some(best) = self.find_best_block() {
            self.head = Some(best);
        }
    }
    
    /// Get canonical chain
    pub fn get_canonical_chain(&self) -> Vec<Block> {
        let mut chain = Vec::new();
        let mut current = self.head;
        
        while let Some(hash) = current {
            if let Some(block) = self.blocks.get(&hash) {
                chain.push(block.clone());
                current = Some(block.header.previous_hash);
            } else {
                break;
            }
        }
        
        chain.reverse();
        chain
    }
}
```

**Thời gian:** 1 tuần  
**Output:** ~500 LOC

---

#### 2.3 AI Validation Integration (Tuần 5-6)

```rust
// crates/luxtensor-consensus/src/ai_validation.rs

pub struct AIValidator {
    // zkML integration
}

impl AIValidator {
    pub async fn validate_ai_task(
        &self,
        task: &AITask,
        result: &AIResult
    ) -> Result<bool> {
        // 1. Verify zkML proof
        // 2. Check result correctness
        // 3. Score quality
        
        Ok(true)
    }
    
    pub fn calculate_ai_reward(
        &self,
        validation_score: f64,
        stake: u128
    ) -> u128 {
        // Calculate reward based on validation quality
        (stake as f64 * validation_score) as u128
    }
}
```

**Thời gian:** 1 tuần  
**Output:** ~800 LOC

---

### Phase 3: Network Layer (Tháng 3-4)

**Priority:** 🔴 HIGH  
**Timeline:** 6 tuần  
**Team:** 2 Rust engineers

#### 3.1 Crate: `luxtensor-network` (Tuần 1-4)

**P2P networking với libp2p:**

```rust
// crates/luxtensor-network/src/p2p.rs

use libp2p::{
    core::upgrade,
    floodsub::{Floodsub, FloodsubEvent, Topic},
    identity,
    mdns::{Mdns, MdnsEvent},
    mplex,
    noise,
    swarm::{SwarmBuilder, SwarmEvent},
    tcp::TokioTcpConfig,
    NetworkBehaviour, PeerId, Swarm, Transport,
};

#[derive(NetworkBehaviour)]
pub struct P2PBehaviour {
    pub floodsub: Floodsub,
    pub mdns: Mdns,
}

pub struct P2PNode {
    swarm: Swarm<P2PBehaviour>,
    peers: HashMap<PeerId, PeerInfo>,
}

impl P2PNode {
    pub async fn new(listen_port: u16) -> Result<Self> {
        // Generate keypair
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        
        // Create transport
        let transport = TokioTcpConfig::new()
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::NoiseConfig::xx(local_key).into_authenticated())
            .multiplex(mplex::MplexConfig::new())
            .boxed();
        
        // Create behaviour
        let behaviour = P2PBehaviour {
            floodsub: Floodsub::new(local_peer_id),
            mdns: Mdns::new(Default::default()).await?,
        };
        
        // Create swarm
        let mut swarm = SwarmBuilder::new(transport, behaviour, local_peer_id)
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build();
        
        // Listen on port
        swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", listen_port).parse()?)?;
        
        Ok(Self {
            swarm,
            peers: HashMap::new(),
        })
    }
    
    /// Subscribe to topic
    pub fn subscribe(&mut self, topic: &str) {
        let topic = Topic::new(topic);
        self.swarm.behaviour_mut().floodsub.subscribe(topic);
    }
    
    /// Broadcast message
    pub fn broadcast(&mut self, topic: &str, data: Vec<u8>) {
        let topic = Topic::new(topic);
        self.swarm.behaviour_mut().floodsub.publish(topic, data);
    }
    
    /// Event loop
    pub async fn run(&mut self) -> Result<()> {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {:?}", address);
                }
                SwarmEvent::Behaviour(event) => {
                    self.handle_behaviour_event(event).await?;
                }
                _ => {}
            }
        }
    }
    
    async fn handle_behaviour_event(&mut self, event: P2PBehaviourEvent) -> Result<()> {
        match event {
            P2PBehaviourEvent::Floodsub(FloodsubEvent::Message(msg)) => {
                // Handle message
                self.handle_message(msg).await?;
            }
            P2PBehaviourEvent::Mdns(MdnsEvent::Discovered(list)) => {
                // New peers discovered
                for (peer_id, _addr) in list {
                    self.swarm.behaviour_mut().floodsub.add_node_to_partial_view(peer_id);
                }
            }
            _ => {}
        }
        Ok(())
    }
}
```

**Thời gian:** 3 tuần  
**Output:** ~2,000 LOC

---

#### 3.2 Sync Protocol (Tuần 4-5)

```rust
// crates/luxtensor-network/src/sync.rs

pub struct SyncManager {
    network: Arc<P2PNode>,
    blockchain: Arc<Blockchain>,
}

impl SyncManager {
    pub async fn sync(&self) -> Result<()> {
        // 1. Find best peer
        let best_peer = self.find_best_peer().await?;
        
        // 2. Request headers
        let headers = self.request_headers(&best_peer).await?;
        
        // 3. Validate headers
        // 4. Download blocks
        // 5. Validate and apply
        
        Ok(())
    }
    
    pub async fn handle_new_block(&self, block: Block) -> Result<()> {
        // Validate and process new block
        self.blockchain.add_block(block).await?;
        Ok(())
    }
}
```

**Thời gian:** 1 tuần  
**Output:** ~800 LOC

---

#### 3.3 Message Protocol (Tuần 5-6)

```rust
// crates/luxtensor-network/src/messages.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // Handshake
    Hello(HelloMessage),
    Ping,
    Pong,
    
    // Blockchain sync
    GetBlocks { start: BlockHeight, end: BlockHeight },
    Blocks(Vec<Block>),
    GetHeaders { start: BlockHeight, end: BlockHeight },
    Headers(Vec<BlockHeader>),
    
    // Propagation
    NewTransaction(Transaction),
    NewBlock(Block),
    
    // State sync
    GetState { root: Hash },
    State(Vec<u8>),
}

pub fn encode_message(msg: &Message) -> Result<Vec<u8>> {
    bincode::serialize(msg)
        .map_err(|e| LuxTensorError::EncodingError(e.to_string()))
}

pub fn decode_message(data: &[u8]) -> Result<Message> {
    bincode::deserialize(data)
        .map_err(|e| LuxTensorError::DecodingError(e.to_string()))
}
```

**Thời gian:** 1 tuần  
**Output:** ~500 LOC

---

### Phase 4: Storage Layer (Tháng 4-5)

**Priority:** 🟡 MEDIUM  
**Timeline:** 4 tuần  
**Team:** 1-2 Rust engineers

#### 4.1 Crate: `luxtensor-storage` (Tuần 1-4)

```rust
// crates/luxtensor-storage/src/blockchain_db.rs

use rocksdb::{DB, ColumnFamilyDescriptor, Options};

pub struct BlockchainDB {
    db: Arc<DB>,
}

impl BlockchainDB {
    pub fn new(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        let cfs = vec![
            ColumnFamilyDescriptor::new("blocks", Options::default()),
            ColumnFamilyDescriptor::new("transactions", Options::default()),
            ColumnFamilyDescriptor::new("receipts", Options::default()),
            ColumnFamilyDescriptor::new("index", Options::default()),
        ];
        
        let db = DB::open_cf_descriptors(&opts, path, cfs)?;
        
        Ok(Self { db: Arc::new(db) })
    }
    
    pub fn store_block(&self, block: &Block) -> Result<()> {
        let cf = self.db.cf_handle("blocks").unwrap();
        let key = block.hash();
        let value = bincode::serialize(block)?;
        self.db.put_cf(cf, key, value)?;
        
        // Update index
        self.index_block(block)?;
        
        Ok(())
    }
    
    pub fn get_block(&self, hash: &Hash) -> Result<Option<Block>> {
        let cf = self.db.cf_handle("blocks").unwrap();
        let value = self.db.get_cf(cf, hash)?;
        
        match value {
            Some(bytes) => Ok(Some(bincode::deserialize(&bytes)?)),
            None => Ok(None),
        }
    }
    
    pub fn get_block_by_height(&self, height: BlockHeight) -> Result<Option<Block>> {
        // Query index first
        let cf_index = self.db.cf_handle("index").unwrap();
        let key = format!("height:{}", height);
        
        if let Some(hash_bytes) = self.db.get_cf(cf_index, key.as_bytes())? {
            let hash: Hash = hash_bytes.try_into()
                .map_err(|_| LuxTensorError::InvalidHash)?;
            return self.get_block(&hash);
        }
        
        Ok(None)
    }
}
```

**Thời gian:** 2 tuần  
**Output:** ~1,200 LOC

---

### Phase 5: API Layer (Tháng 5)

**Priority:** 🟡 MEDIUM  
**Timeline:** 4 tuần  
**Team:** 1 Rust engineer

#### 5.1 Crate: `luxtensor-api` (Tuần 1-3)

**JSON-RPC API với axum:**

```rust
// crates/luxtensor-api/src/rpc.rs

use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};

pub struct RpcServer {
    blockchain: Arc<Blockchain>,
}

impl RpcServer {
    pub async fn start(addr: &str, blockchain: Arc<Blockchain>) -> Result<()> {
        let state = Arc::new(Self { blockchain });
        
        let app = Router::new()
            .route("/", post(handle_rpc))
            .with_state(state);
        
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

async fn handle_rpc(
    State(server): State<Arc<RpcServer>>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let method = payload["method"].as_str().unwrap_or("");
    let params = &payload["params"];
    
    let result = match method {
        "eth_blockNumber" => server.eth_block_number().await,
        "eth_getBlockByNumber" => server.eth_get_block_by_number(params).await,
        "eth_getBalance" => server.eth_get_balance(params).await,
        "eth_sendRawTransaction" => server.eth_send_raw_transaction(params).await,
        // AI-specific methods
        "mt_submitAITask" => server.mt_submit_ai_task(params).await,
        "mt_getAIResult" => server.mt_get_ai_result(params).await,
        _ => Err(LuxTensorError::UnknownMethod),
    };
    
    match result {
        Ok(value) => Json(json!({
            "jsonrpc": "2.0",
            "result": value,
            "id": payload["id"]
        })),
        Err(err) => Json(json!({
            "jsonrpc": "2.0",
            "error": { "message": err.to_string() },
            "id": payload["id"]
        })),
    }
}
```

**Thời gian:** 2 tuần  
**Output:** ~1,500 LOC

---

#### 5.2 GraphQL API (Tuần 3-4)

```rust
// crates/luxtensor-api/src/graphql.rs

use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn block(&self, ctx: &Context<'_>, hash: Option<String>, height: Option<u64>) -> Result<Block> {
        let blockchain = ctx.data::<Arc<Blockchain>>()?;
        
        if let Some(h) = hash {
            let hash = hex::decode(h)?;
            blockchain.get_block_by_hash(&hash.try_into().unwrap()).await
        } else if let Some(height) = height {
            blockchain.get_block_by_height(height).await
        } else {
            Err(LuxTensorError::InvalidParams.into())
        }
    }
    
    async fn transaction(&self, ctx: &Context<'_>, hash: String) -> Result<Transaction> {
        let blockchain = ctx.data::<Arc<Blockchain>>()?;
        let hash = hex::decode(hash)?;
        blockchain.get_transaction(&hash.try_into().unwrap()).await
    }
}

pub type LuxTensorSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;
```

**Thời gian:** 1 tuần  
**Output:** ~800 LOC

---

### Phase 6: Node Binary & CLI (Tháng 5-6)

**Priority:** 🟡 MEDIUM  
**Timeline:** 4 tuần  
**Team:** 1 Rust engineer

#### 6.1 Crate: `luxtensor-node` (Tuần 1-2)

**Main node binary:**

```rust
// crates/luxtensor-node/src/main.rs

use clap::Parser;

#[derive(Parser)]
#[clap(name = "luxtensor")]
#[clap(about = "LuxTensor blockchain node", version)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start node
    Start {
        #[clap(long, default_value = "30303")]
        port: u16,
        
        #[clap(long, default_value = "8545")]
        rpc_port: u16,
        
        #[clap(long)]
        bootstrap: Vec<String>,
    },
    
    /// Initialize genesis
    Init {
        #[clap(long)]
        genesis: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Start { port, rpc_port, bootstrap } => {
            start_node(port, rpc_port, bootstrap).await?;
        }
        Commands::Init { genesis } => {
            init_genesis(&genesis).await?;
        }
    }
    
    Ok(())
}

async fn start_node(port: u16, rpc_port: u16, bootstrap: Vec<String>) -> Result<()> {
    // Initialize components
    let storage = BlockchainDB::new("./data")?;
    let state = StateDB::new("./data/state")?;
    let blockchain = Arc::new(Blockchain::new(storage, state));
    
    // Start network
    let mut network = P2PNode::new(port).await?;
    network.subscribe("blocks");
    network.subscribe("transactions");
    
    // Connect to bootstrap nodes
    for addr in bootstrap {
        // Connect
    }
    
    // Start RPC server
    tokio::spawn(async move {
        RpcServer::start(&format!("0.0.0.0:{}", rpc_port), blockchain).await
    });
    
    // Run network event loop
    network.run().await?;
    
    Ok(())
}
```

**Thời gian:** 2 tuần  
**Output:** ~1,000 LOC

---

#### 6.2 Crate: `luxtensor-cli` (Tuần 3-4)

**CLI tools:**

```rust
// crates/luxtensor-cli/src/main.rs

#[derive(Parser)]
#[clap(name = "luxtensor-cli")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Wallet operations
    Wallet {
        #[clap(subcommand)]
        cmd: WalletCommands,
    },
    
    /// Query blockchain
    Query {
        #[clap(subcommand)]
        cmd: QueryCommands,
    },
}

#[derive(Subcommand)]
enum WalletCommands {
    Create { name: String },
    Import { private_key: String },
    Balance { address: String },
    Send {
        from: String,
        to: String,
        amount: u128,
    },
}
```

**Thời gian:** 2 tuần  
**Output:** ~800 LOC

---

### Phase 7: Testing & Optimization (Tháng 6-7)

**Priority:** 🔴 HIGH  
**Timeline:** 6 tuần  
**Team:** 2 Rust engineers

#### 7.1 Comprehensive Testing (Tuần 1-3)

**Test categories:**
- Unit tests (per module)
- Integration tests (cross-module)
- End-to-end tests (full node)
- Property-based tests (với proptest)
- Benchmarks (với criterion)

```rust
// tests/integration/full_flow.rs

#[tokio::test]
async fn test_end_to_end_transaction() {
    // 1. Start test network
    let nodes = start_test_network(3).await;
    
    // 2. Generate accounts
    let alice = KeyPair::generate();
    let bob = KeyPair::generate();
    
    // 3. Fund Alice
    fund_account(&nodes[0], alice.address(), 1000).await;
    
    // 4. Alice sends to Bob
    let tx = create_transaction(alice, bob.address(), 100);
    nodes[0].submit_transaction(tx).await.unwrap();
    
    // 5. Wait for block
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // 6. Verify balances
    let alice_balance = nodes[0].get_balance(alice.address()).await.unwrap();
    let bob_balance = nodes[0].get_balance(bob.address()).await.unwrap();
    
    assert_eq!(alice_balance, 900); // 1000 - 100 - gas
    assert_eq!(bob_balance, 100);
}
```

**Thời gian:** 3 tuần  
**Output:** ~2,000 LOC tests

---

#### 7.2 Performance Optimization (Tuần 4-6)

**Optimization areas:**
1. **Signature verification batching**
2. **Parallel transaction execution**
3. **Database query optimization**
4. **Network message compression**
5. **Memory pooling**

```rust
// Performance benchmarks
fn benchmark_transaction_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_processing");
    
    for size in [100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter(|| process_transactions(create_test_transactions(size)))
            },
        );
    }
}
```

**Target metrics:**
- Block processing: < 1s per block
- Transaction throughput: > 1000 TPS
- Sync speed: > 500 blocks/s
- Memory usage: < 2GB for full node

**Thời gian:** 3 tuần  
**Output:** Optimized codebase

---

### Phase 8: Documentation & Examples (Tháng 7)

**Priority:** 🟡 MEDIUM  
**Timeline:** 3 tuần  
**Team:** 1 engineer + 1 technical writer

#### 8.1 Documentation (Tuần 1-2)

**Documents to create:**
1. **Architecture Guide** - System design, components
2. **API Reference** - RPC and GraphQL APIs
3. **Node Operation** - Running nodes, validators
4. **Development Guide** - Contributing, building
5. **Migration Guide** - From Python to Rust

**Thời gian:** 2 tuần

---

#### 8.2 Examples (Tuần 2-3)

**Example code:**
```rust
// examples/simple_transfer.rs
// examples/deploy_contract.rs
// examples/run_validator.rs
// examples/custom_network.rs
```

**Thời gian:** 1 tuần  
**Output:** ~500 LOC examples

---

### Phase 9: Testnet & Mainnet (Tháng 8-9)

**Priority:** 🎯 GOAL  
**Timeline:** 6 tuần  
**Team:** Full team

#### 9.1 Testnet Launch (Tuần 1-3)

**Activities:**
1. Deploy testnet nodes
2. Community testing
3. Bug fixes
4. Performance tuning

---

#### 9.2 Mainnet Launch (Tuần 4-6)

**Activities:**
1. Security audit (external)
2. Genesis ceremony
3. Validator onboarding
4. Mainnet deployment

---

## 📊 Timeline Summary

| Phase | Tháng | Team | Output (LOC) | Status |
|-------|-------|------|--------------|--------|
| 0. Setup | Tháng 1 (Tuần 1-3) | 1 | ~500 | ⏸️ |
| 1. Core | Tháng 1-2 (8 tuần) | 2 | ~5,000 | ⏸️ |
| 2. Consensus | Tháng 2-3 (6 tuần) | 2 | ~3,500 | ⏸️ |
| 3. Network | Tháng 3-4 (6 tuần) | 2 | ~3,500 | ⏸️ |
| 4. Storage | Tháng 4-5 (4 tuần) | 1-2 | ~1,500 | ⏸️ |
| 5. API | Tháng 5 (4 tuần) | 1 | ~2,500 | ⏸️ |
| 6. Node/CLI | Tháng 5-6 (4 tuần) | 1 | ~2,000 | ⏸️ |
| 7. Testing | Tháng 6-7 (6 tuần) | 2 | ~2,000 | ⏸️ |
| 8. Docs | Tháng 7 (3 tuần) | 1-2 | Docs | ⏸️ |
| 9. Launch | Tháng 8-9 (6 tuần) | Full team | - | ⏸️ |
| **Total** | **~9 tháng** | **3-4 engineers** | **~20,500 LOC** | **0%** |

---

## 💰 Budget Estimate

| Category | Cost (USD) |
|----------|------------|
| Rust Engineers (4 × 9 months × $150k/year) | $450,000 |
| Infrastructure (testnet/mainnet) | $30,000 |
| Security Audit | $80,000 |
| Bug Bounty | $50,000 |
| Contingency (20%) | $122,000 |
| **Total** | **$732,000** |

---

## 🎯 Key Decisions

### Technology Choices
- ✅ **Language:** Rust (performance + safety)
- ✅ **Async:** Tokio (battle-tested)
- ✅ **Network:** libp2p (standard cho blockchain)
- ✅ **Storage:** RocksDB (proven)
- ✅ **Crypto:** ed25519-dalek (fast)

### Migration Strategy
- ✅ **Parallel development:** Keep Python version running
- ✅ **Module-by-module:** Convert one crate at a time
- ✅ **Testing:** Extensive testing at each phase
- ✅ **Gradual rollout:** Testnet first, then mainnet

---

## 📈 Success Metrics

### Performance Targets
- Transaction throughput: **> 1,000 TPS** (10x Python)
- Block time: **< 1 second**
- Sync speed: **> 500 blocks/second**
- Memory usage: **< 2GB** for full node

### Quality Targets
- Test coverage: **> 80%**
- Zero critical vulnerabilities
- **< 100ms** p99 RPC latency

---

## ⚠️ Risks & Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Rust expertise shortage | Medium | High | Training, hire experienced |
| Performance goals not met | Low | High | Continuous benchmarking |
| Security vulnerabilities | Medium | Critical | External audit, bug bounty |
| Timeline delays | Medium | Medium | Buffer time, agile approach |
| Migration complexity | Low | Medium | Incremental approach |

---

## 🚀 Action Items - Next Steps

### Week 1 (IMMEDIATE)
1. ✅ Approve roadmap
2. ✅ Setup luxtensor repo
3. ✅ Hire/assign Rust engineers
4. ✅ Setup CI/CD pipeline
5. ✅ Create project board

### Week 2-3
1. ✅ Complete Phase 0 (Setup)
2. ✅ Technical design review
3. ✅ Start Phase 1 (Core)

### Month 2+
1. ✅ Follow phase schedule
2. ✅ Weekly progress reviews
3. ✅ Continuous testing
4. ✅ Documentation updates

---

## 📚 Learning Resources

### Rust Blockchain Projects
- **Substrate** (Polkadot framework)
- **Solana** (High-performance blockchain)
- **Near Protocol** (Sharded blockchain)
- **Lighthouse** (Ethereum 2.0 client)

### Key Libraries
- **tokio** - Async runtime
- **libp2p** - P2P networking
- **rocksdb** - Database
- **ed25519-dalek** - Signatures
- **serde** - Serialization

### Books & Docs
- "The Rust Programming Language"
- "Rust for Rustaceans"
- "Zero to Production in Rust"
- Rust blockchain tutorials

---

## 🎊 Conclusion

**LuxTensor** represents the future of ModernTensor - a high-performance, production-ready blockchain implementation in Rust.

### Key Points:
- ✅ **Feasible:** 9 months with 3-4 engineers
- ✅ **Valuable:** 10-100x performance improvement
- ✅ **Strategic:** Positions for mainnet success
- ✅ **Proven:** Following successful blockchain patterns

### Next Steps:
1. Approve this roadmap
2. Allocate resources
3. Start Phase 0 setup
4. Begin module conversion

**Timeline:** 9 months to production-ready Rust blockchain  
**Investment:** ~$732k  
**Return:** High-performance, secure, scalable blockchain

---

**Let's build LuxTensor! 🚀**

**Document này là:** Comprehensive conversion roadmap  
**Liên hệ:** Tạo GitHub issue để track progress  
**Repository:** github.com/sonson0910/luxtensor (to be created)
