# Component-by-Component Migration Plan

## 📋 Chi Tiết Chuyển Đổi Từng Module

### Module 1: Core - Blockchain Primitives

#### 1.1 Cryptography (`core/src/crypto.rs`)

**Source**: `sdk/blockchain/crypto.py` (400 LOC)

**Components to Migrate**:
- ✅ `KeyPair` class → `struct KeyPair`
- ✅ ECDSA signing/verification
- ✅ Hash functions (SHA256, BLAKE3)
- ✅ Merkle tree implementation
- ✅ Address derivation

**Rust Crates**:
```toml
secp256k1 = "0.28"     # ECDSA signatures
sha2 = "0.10"          # SHA-256
blake3 = "1.5"         # BLAKE3
hex = "0.4"            # Hex encoding
```

**Implementation Guide**:
```rust
use secp256k1::{Secp256k1, SecretKey, PublicKey};
use sha2::{Sha256, Digest};

pub struct KeyPair {
    secret_key: SecretKey,
    public_key: PublicKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut rand::thread_rng());
        KeyPair { secret_key, public_key }
    }
    
    pub fn sign(&self, message: &[u8]) -> Signature {
        let secp = Secp256k1::new();
        let msg = Message::from_digest_slice(message).unwrap();
        secp.sign_ecdsa(&msg, &self.secret_key)
    }
    
    pub fn address(&self) -> Address {
        let pubkey_bytes = self.public_key.serialize_uncompressed();
        let hash = Sha256::digest(&pubkey_bytes[1..]); // Skip first byte
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&hash[12..]); // Take last 20 bytes
        addr
    }
}
```

**Tests to Port**:
- Key generation
- Sign/verify roundtrip
- Address derivation
- Merkle tree proof generation/verification

**Timeline**: Week 3-4 (2 weeks)

---

#### 1.2 Transactions (`core/src/transaction.rs`)

**Source**: `sdk/blockchain/transaction.py` (350 LOC)

**Components to Migrate**:
- ✅ `Transaction` dataclass → `struct Transaction` (DONE - initial version)
- ✅ Transaction hashing (DONE)
- ✅ Intrinsic gas calculation (DONE)
- ⬜ Full signature implementation
- ⬜ RLP encoding for Ethereum compatibility
- ⬜ `TransactionReceipt` (DONE - basic structure)

**Additional Implementation Needed**:
```rust
use rlp::{Encodable, Decodable, RlpStream, Rlp};

impl Encodable for Transaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(9);
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas_limit);
        s.append(&self.to);
        s.append(&self.value);
        s.append(&self.data);
        s.append(&self.v);
        s.append(&self.r);
        s.append(&self.s);
    }
}

impl Transaction {
    pub fn signing_hash(&self) -> Hash {
        // Hash for signing (without signature)
        let mut stream = RlpStream::new();
        stream.begin_list(6);
        stream.append(&self.nonce);
        stream.append(&self.gas_price);
        stream.append(&self.gas_limit);
        stream.append(&self.to);
        stream.append(&self.value);
        stream.append(&self.data);
        let encoded = stream.out();
        let mut hasher = Sha256::new();
        hasher.update(&encoded);
        hasher.finalize().into()
    }
}
```

**Timeline**: Week 4 (1 week)

---

#### 1.3 Blocks (`core/src/block.rs`)

**Source**: `sdk/blockchain/block.py` (300 LOC)

**Status**: ✅ Basic structure DONE

**Additional Work Needed**:
- ⬜ Proper Merkle root calculation (use `patricia-trie`)
- ⬜ Block signature verification
- ⬜ Block RLP encoding
- ⬜ Uncle blocks support (optional)

**Implementation**:
```rust
use patricia_trie::{TrieDB, TrieDBMut};

impl Block {
    pub fn calculate_transactions_root(transactions: &[Transaction]) -> Hash {
        let mut trie = TrieDBMut::new();
        for (i, tx) in transactions.iter().enumerate() {
            let key = rlp::encode(&i);
            let value = rlp::encode(tx);
            trie.insert(&key, &value);
        }
        trie.root().into()
    }
}
```

**Timeline**: Week 5 (1 week)

---

#### 1.4 State Management (`core/src/state.rs`)

**Source**: `sdk/blockchain/state.py` (470 LOC)

**Status**: ⬜ TODO

**Components to Migrate**:
- Account model (balance, nonce, storage, code)
- StateDB with cache
- State transitions
- Merkle Patricia Trie
- Commit/rollback functionality

**Implementation Plan**:
```rust
use std::collections::HashMap;
use patricia_trie::{TrieDB, TrieDBMut};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub nonce: u64,
    pub balance: Balance,
    pub storage_root: Hash,
    pub code_hash: Hash,
}

pub struct StateDB {
    /// Committed state (on disk)
    committed: TrieDB,
    
    /// Cache for dirty accounts
    cache: HashMap<Address, Account>,
    
    /// Dirty accounts (modified but not committed)
    dirty: HashSet<Address>,
}

impl StateDB {
    pub fn get_account(&self, address: &Address) -> Option<Account> {
        // Check cache first
        if let Some(account) = self.cache.get(address) {
            return Some(account.clone());
        }
        // Load from trie
        self.committed.get(address)
    }
    
    pub fn set_account(&mut self, address: Address, account: Account) {
        self.cache.insert(address, account);
        self.dirty.insert(address);
    }
    
    pub fn commit(&mut self) -> Hash {
        for address in &self.dirty {
            if let Some(account) = self.cache.get(address) {
                self.committed.insert(address, account);
            }
        }
        self.dirty.clear();
        self.committed.root()
    }
    
    pub fn rollback(&mut self) {
        for address in &self.dirty {
            self.cache.remove(address);
        }
        self.dirty.clear();
    }
}
```

**Timeline**: Week 5-6 (2 weeks)

---

#### 1.5 Validation (`core/src/validation.rs`)

**Source**: `sdk/blockchain/validation.py` (400 LOC)

**Status**: ⬜ TODO

**Components**:
- Block validation rules
- Transaction validation
- Gas metering
- State execution
- Contract deployment and calls

**Implementation**:
```rust
pub struct BlockValidator {
    chain_config: ChainConfig,
}

impl BlockValidator {
    pub fn validate_block(&self, block: &Block, parent: &Block) -> CoreResult<()> {
        // Check block number
        if block.header.number != parent.header.number + 1 {
            return Err(CoreError::InvalidBlock("Invalid block number".into()));
        }
        
        // Check parent hash
        if block.header.parent_hash != parent.hash() {
            return Err(CoreError::InvalidBlock("Invalid parent hash".into()));
        }
        
        // Check timestamp
        if block.header.timestamp <= parent.header.timestamp {
            return Err(CoreError::InvalidBlock("Invalid timestamp".into()));
        }
        
        // Verify signature
        self.verify_signature(block)?;
        
        // Validate transactions
        for tx in &block.transactions {
            self.validate_transaction(tx)?;
        }
        
        Ok(())
    }
    
    pub fn validate_transaction(&self, tx: &Transaction) -> CoreResult<()> {
        // Check signature
        tx.verify_signature()?;
        
        // Check intrinsic gas
        if tx.gas_limit < tx.intrinsic_gas() {
            return Err(CoreError::InvalidTransaction("Insufficient gas".into()));
        }
        
        Ok(())
    }
    
    pub fn execute_transaction(
        &self,
        tx: &Transaction,
        state: &mut StateDB,
    ) -> CoreResult<TransactionReceipt> {
        // Load sender account
        let mut sender = state.get_account(&tx.from)
            .ok_or(CoreError::InvalidTransaction("Sender not found".into()))?;
        
        // Check nonce
        if tx.nonce != sender.nonce {
            return Err(CoreError::InvalidNonce {
                expected: sender.nonce,
                actual: tx.nonce,
            });
        }
        
        // Check balance
        let total_cost = tx.value + (tx.gas_limit as u128 * tx.gas_price as u128);
        if sender.balance < total_cost {
            return Err(CoreError::InsufficientBalance);
        }
        
        // Update sender
        sender.nonce += 1;
        sender.balance -= total_cost;
        state.set_account(tx.from, sender);
        
        // Transfer value
        if let Some(to_addr) = tx.to {
            let mut recipient = state.get_account(&to_addr)
                .unwrap_or(Account::default());
            recipient.balance += tx.value;
            state.set_account(to_addr, recipient);
        }
        
        // Create receipt
        Ok(TransactionReceipt {
            transaction_hash: tx.hash(),
            status: true,
            gas_used: tx.intrinsic_gas(),
            // ... other fields
        })
    }
}
```

**Timeline**: Week 6-7 (2 weeks)

---

### Module 2: Consensus Layer

#### 2.1 Proof of Stake (`consensus/src/pos.rs`)

**Source**: `sdk/consensus/pos.py` (390 LOC)

**Components**:
- Validator set management
- Stake tracking
- Validator selection (VRF-based)
- Epoch processing
- Reward distribution
- Slashing mechanism

**Key Structures**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub address: Address,
    pub public_key: PublicKey,
    pub stake: Balance,
    pub commission: u16, // Basis points (0-10000)
    pub is_jailed: bool,
    pub jail_until: u64,
}

pub struct ValidatorSet {
    validators: HashMap<Address, Validator>,
    total_stake: Balance,
}

impl ValidatorSet {
    pub fn select_validator(&self, seed: &[u8]) -> Result<Address> {
        // VRF-based weighted random selection
        let mut rng = ChaChaRng::from_seed(seed);
        let target = rng.gen_range(0..self.total_stake);
        
        let mut cumulative = 0;
        for (addr, validator) in &self.validators {
            if validator.is_jailed {
                continue;
            }
            cumulative += validator.stake;
            if cumulative >= target {
                return Ok(*addr);
            }
        }
        
        Err(ConsensusError::NoValidatorSelected)
    }
}
```

**Timeline**: Week 9-10 (2 weeks)

---

#### 2.2 Fork Choice (`consensus/src/fork_choice.rs`)

**Source**: `sdk/consensus/fork_choice.py` (300 LOC)

**Components**:
- GHOST algorithm
- Block tree management
- Canonical chain selection
- Finalization (Casper FFG)

**Implementation**:
```rust
pub struct BlockNode {
    pub hash: Hash,
    pub parent: Hash,
    pub number: BlockNumber,
    pub total_weight: u64,
    pub children: Vec<Hash>,
    pub is_finalized: bool,
}

pub struct ForkChoice {
    blocks: HashMap<Hash, BlockNode>,
    head: Hash,
    finalized: Hash,
}

impl ForkChoice {
    pub fn add_block(&mut self, block: &Block, weight: u64) {
        let node = BlockNode {
            hash: block.hash(),
            parent: block.header.parent_hash,
            number: block.header.number,
            total_weight: weight,
            children: Vec::new(),
            is_finalized: false,
        };
        
        // Update parent's children
        if let Some(parent) = self.blocks.get_mut(&block.header.parent_hash) {
            parent.children.push(block.hash());
        }
        
        self.blocks.insert(block.hash(), node);
        self.update_head();
    }
    
    fn update_head(&mut self) {
        // GHOST: Select heaviest subtree
        let mut current = self.finalized;
        loop {
            let node = &self.blocks[&current];
            if node.children.is_empty() {
                self.head = current;
                break;
            }
            
            // Find child with heaviest subtree
            let mut heaviest = node.children[0];
            let mut max_weight = self.subtree_weight(heaviest);
            
            for &child in &node.children[1..] {
                let weight = self.subtree_weight(child);
                if weight > max_weight {
                    max_weight = weight;
                    heaviest = child;
                }
            }
            
            current = heaviest;
        }
    }
    
    fn subtree_weight(&self, hash: Hash) -> u64 {
        // Calculate total weight of subtree
        let node = &self.blocks[&hash];
        let mut weight = node.total_weight;
        for &child in &node.children {
            weight += self.subtree_weight(child);
        }
        weight
    }
}
```

**Timeline**: Week 11-12 (2 weeks)

---

### Module 3: Network Layer

#### 3.1 P2P Networking (`network/src/p2p.rs`)

**Source**: `sdk/network/p2p.py` (550 LOC)

**Components**:
- Peer discovery (mDNS, Kademlia DHT)
- Connection management
- Protocol handlers
- Gossipsub for block/tx propagation

**Implementation with libp2p**:
```rust
use libp2p::{
    gossipsub, identify, kad, mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, PeerId, Swarm,
};

#[derive(NetworkBehaviour)]
pub struct P2PBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    kad: kad::Kademlia<kad::store::MemoryStore>,
    identify: identify::Behaviour,
}

pub struct P2PNetwork {
    swarm: Swarm<P2PBehaviour>,
    block_topic: gossipsub::IdentTopic,
    tx_topic: gossipsub::IdentTopic,
}

impl P2PNetwork {
    pub async fn new(keypair: Keypair, listen_addr: Multiaddr) -> Result<Self> {
        let peer_id = PeerId::from(keypair.public());
        
        // Setup transport
        let transport = tcp::tokio::Transport::default()
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&keypair)?)
            .multiplex(yamux::Config::default())
            .boxed();
        
        // Setup behaviour
        let behaviour = P2PBehaviour {
            gossipsub: create_gossipsub_behaviour(),
            mdns: mdns::tokio::Behaviour::new(Default::default())?,
            kad: kad::Kademlia::new(peer_id, MemoryStore::new(peer_id)),
            identify: identify::Behaviour::new(Default::default()),
        };
        
        let mut swarm = Swarm::new(transport, behaviour, peer_id, Default::default());
        swarm.listen_on(listen_addr)?;
        
        let block_topic = gossipsub::IdentTopic::new("blocks");
        let tx_topic = gossipsub::IdentTopic::new("transactions");
        
        Ok(P2PNetwork { swarm, block_topic, tx_topic })
    }
    
    pub async fn broadcast_block(&mut self, block: &Block) -> Result<()> {
        let data = bincode::serialize(block)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.block_topic.clone(), data)?;
        Ok(())
    }
}
```

**Timeline**: Week 17-18 (2 weeks)

---

#### 3.2 Chain Synchronization (`network/src/sync.rs`)

**Source**: `sdk/network/sync.py` (450 LOC)

**Components**:
- Block sync protocol
- State sync
- Fast sync / warp sync
- Catch-up mechanism

**Timeline**: Week 19 (1 week)

---

### Module 4: Storage Layer

#### 4.1 Database (`storage/src/db.rs`)

**Source**: `sdk/storage/blockchain_db.py`, `sdk/storage/state_db.py` (850 LOC total)

**Components**:
- RocksDB integration
- Block storage
- State storage
- Transaction indexing
- Receipt storage

**Implementation**:
```rust
use rocksdb::{DB, Options, WriteBatch};

pub struct BlockchainDB {
    db: Arc<DB>,
}

impl BlockchainDB {
    pub fn open(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        let db = DB::open_cf(&opts, path, &[
            "blocks",
            "transactions",
            "receipts",
            "state",
        ])?;
        
        Ok(BlockchainDB { db: Arc::new(db) })
    }
    
    pub fn put_block(&self, block: &Block) -> Result<()> {
        let cf = self.db.cf_handle("blocks").unwrap();
        let key = block.hash();
        let value = bincode::serialize(block)?;
        self.db.put_cf(cf, key, value)?;
        Ok(())
    }
    
    pub fn get_block(&self, hash: &Hash) -> Result<Option<Block>> {
        let cf = self.db.cf_handle("blocks").unwrap();
        match self.db.get_cf(cf, hash)? {
            Some(bytes) => Ok(Some(bincode::deserialize(&bytes)?)),
            None => Ok(None),
        }
    }
}
```

**Timeline**: Week 21-22 (2 weeks)

---

### Module 5: RPC & API Layer

#### 5.1 JSON-RPC (`rpc/src/jsonrpc.rs`)

**Source**: `sdk/api/jsonrpc.py` (400 LOC)

**Standard Methods**:
- `eth_blockNumber`
- `eth_getBalance`
- `eth_getTransactionByHash`
- `eth_sendRawTransaction`
- `eth_call`
- `eth_estimateGas`

**Implementation**:
```rust
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    server::{Server, ServerHandle},
};

#[rpc(server)]
pub trait RpcApi {
    #[method(name = "eth_blockNumber")]
    async fn block_number(&self) -> RpcResult<u64>;
    
    #[method(name = "eth_getBalance")]
    async fn get_balance(&self, address: String, block: Option<String>) -> RpcResult<String>;
    
    #[method(name = "eth_sendRawTransaction")]
    async fn send_raw_transaction(&self, data: String) -> RpcResult<String>;
}

pub struct RpcApiImpl {
    blockchain: Arc<Blockchain>,
}

#[async_trait]
impl RpcApiServer for RpcApiImpl {
    async fn block_number(&self) -> RpcResult<u64> {
        Ok(self.blockchain.get_latest_block_number().await)
    }
    
    async fn get_balance(&self, address: String, _block: Option<String>) -> RpcResult<String> {
        let addr = parse_address(&address)?;
        let balance = self.blockchain.get_balance(&addr).await?;
        Ok(format!("0x{:x}", balance))
    }
    
    async fn send_raw_transaction(&self, data: String) -> RpcResult<String> {
        let tx_bytes = hex::decode(data.trim_start_matches("0x"))?;
        let tx: Transaction = bincode::deserialize(&tx_bytes)?;
        let hash = self.blockchain.add_transaction(tx).await?;
        Ok(format!("0x{}", hex::encode(hash)))
    }
}
```

**Timeline**: Week 25-26 (2 weeks)

---

## ✅ Migration Checklist

### Phase 1: Core (Weeks 1-8)
- [ ] Crypto module complete
- [ ] Transaction module complete
- [ ] Block module complete
- [ ] State module complete
- [ ] Validation module complete
- [ ] All tests passing
- [ ] Documentation complete

### Phase 2: Consensus (Weeks 9-16)
- [ ] PoS implementation
- [ ] Fork choice implementation
- [ ] Validator management
- [ ] Reward distribution
- [ ] Slashing mechanism
- [ ] Integration tests

### Phase 3: Network (Weeks 17-20)
- [ ] P2P networking
- [ ] Chain sync
- [ ] Block propagation
- [ ] Transaction pool
- [ ] Multi-node tests

### Phase 4: Storage (Weeks 21-24)
- [ ] RocksDB integration
- [ ] Block storage
- [ ] State storage
- [ ] Indexing
- [ ] Data migration tools

### Phase 5: RPC (Weeks 25-28)
- [ ] JSON-RPC server
- [ ] Standard methods
- [ ] WebSocket support
- [ ] Rate limiting
- [ ] API documentation

### Phase 6: Integration (Weeks 29-32)
- [ ] Full node implementation
- [ ] Configuration management
- [ ] CLI interface
- [ ] Testnet deployment
- [ ] End-to-end tests

---

## 🎯 Success Criteria

- ✅ All Python functionality replicated
- ✅ >80% test coverage
- ✅ No clippy warnings
- ✅ 10x performance improvement
- ✅ Full documentation
- ✅ Multi-node testnet running
- ✅ RPC API compatible

---

**Total Estimated Time**: 6-8 months (32 weeks)
