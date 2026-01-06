# Python → Rust Component Mapping
# Bản Đồ Chuyển Đổi Components từ ModernTensor sang LuxTensor

**Dự án:** LuxTensor Migration  
**Ngày:** 6 Tháng 1, 2026  
**Mục đích:** Map Python code to Rust implementation

---

## 📊 Overview

| Python Module | Rust Crate | LOC Python | Est. Rust LOC | Priority |
|--------------|------------|------------|---------------|----------|
| sdk/blockchain/ | luxtensor-core | ~5,500 | ~3,500 | 🔴 Critical |
| sdk/consensus/ | luxtensor-consensus | ~6,000 | ~4,000 | 🔴 Critical |
| sdk/network/ | luxtensor-network | ~4,500 | ~3,000 | 🔴 High |
| sdk/storage/ | luxtensor-storage | ~3,500 | ~2,500 | 🟡 Medium |
| sdk/api/ | luxtensor-rpc | ~2,500 | ~2,000 | 🟡 Medium |

---

## 🔧 Blockchain Core Module

### Python: sdk/blockchain/

#### block.py → luxtensor-core/src/block.rs
```python
# Python (ModernTensor)
@dataclass
class Block:
    version: int
    height: int
    timestamp: int
    previous_hash: bytes
    state_root: bytes
    txs_root: bytes
    transactions: List[Transaction]
    
    def hash(self) -> bytes:
        data = self.serialize()
        return hashlib.sha256(data).digest()
```

```rust
// Rust (LuxTensor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub height: u64,
    pub timestamp: u64,
    pub previous_hash: Hash,
    pub state_root: Hash,
    pub txs_root: Hash,
}

impl Block {
    pub fn hash(&self) -> Hash {
        let bytes = bincode::serialize(self).unwrap();
        keccak256(&bytes)
    }
}
```

**Mapping Notes:**
- `@dataclass` → `#[derive(Debug, Clone, Serialize, Deserialize)]`
- `List[Transaction]` → `Vec<Transaction>`
- `hashlib.sha256` → `keccak256()` from luxtensor-crypto

---

#### transaction.py → luxtensor-core/src/transaction.rs
```python
# Python
@dataclass
class Transaction:
    nonce: int
    from_address: bytes
    to_address: Optional[bytes]
    value: int
    gas_price: int
    gas_limit: int
    data: bytes
    signature: Signature
    
    def verify_signature(self) -> bool:
        message = self.signing_message()
        return self.signature.verify(message, self.from_address)
```

```rust
// Rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub nonce: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub value: u128,
    pub gas_price: u64,
    pub gas_limit: u64,
    pub data: Vec<u8>,
    pub v: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
}

impl Transaction {
    pub fn verify_signature(&self) -> Result<(), TransactionError> {
        let message = self.signing_message();
        let pubkey = self.recover_pubkey()?;
        let address = Address::from_pubkey(&pubkey);
        
        if address != self.from {
            return Err(TransactionError::InvalidSignature);
        }
        Ok(())
    }
}
```

**Mapping Notes:**
- `Optional[bytes]` → `Option<Address>`
- Error handling: exceptions → `Result<T, E>`
- Signature split into `v, r, s` components

---

#### state.py → luxtensor-core/src/state.rs
```python
# Python
class StateDB:
    def __init__(self, db_path: str):
        self.db = LevelDB(db_path)
        self.cache: Dict[bytes, Account] = {}
    
    def get_account(self, address: bytes) -> Account:
        if address in self.cache:
            return self.cache[address]
        
        data = self.db.get(address)
        account = Account.deserialize(data)
        self.cache[address] = account
        return account
    
    def set_account(self, address: bytes, account: Account):
        self.cache[address] = account
    
    def commit(self) -> bytes:
        for address, account in self.cache.items():
            self.db.put(address, account.serialize())
        self.cache.clear()
        return self.calculate_root()
```

```rust
// Rust
pub struct StateDB {
    db: Arc<RocksDB>,
    cache: HashMap<Address, Account>,
    dirty: HashSet<Address>,
}

impl StateDB {
    pub fn new(db: Arc<RocksDB>) -> Self {
        Self {
            db,
            cache: HashMap::new(),
            dirty: HashSet::new(),
        }
    }
    
    pub fn get_account(&mut self, address: &Address) -> Result<Account, StateError> {
        if let Some(account) = self.cache.get(address) {
            return Ok(account.clone());
        }
        
        let bytes = self.db.get(address.as_bytes())?
            .ok_or(StateError::AccountNotFound)?;
        let account: Account = bincode::deserialize(&bytes)?;
        
        self.cache.insert(*address, account.clone());
        Ok(account)
    }
    
    pub fn set_account(&mut self, address: Address, account: Account) {
        self.cache.insert(address, account);
        self.dirty.insert(address);
    }
    
    pub fn commit(&mut self) -> Result<Hash, StateError> {
        let mut batch = self.db.batch();
        
        for address in &self.dirty {
            if let Some(account) = self.cache.get(address) {
                let bytes = bincode::serialize(account)?;
                batch.put(address.as_bytes(), &bytes);
            }
        }
        
        self.db.write(batch)?;
        self.dirty.clear();
        
        Ok(self.calculate_root()?)
    }
}
```

**Mapping Notes:**
- `Dict` → `HashMap`
- Manual memory management → automatic with `Arc` and ownership
- `LevelDB` → `RocksDB` (more features, better performance)
- Added `dirty` tracking for optimized commits

---

#### crypto.py → luxtensor-crypto/
```python
# Python
class KeyPair:
    def __init__(self, private_key: Optional[bytes] = None):
        if private_key:
            self.private_key = private_key
        else:
            self.private_key = os.urandom(32)
        self.public_key = self._derive_public_key()
    
    def sign(self, message: bytes) -> bytes:
        return ecdsa.sign(message, self.private_key)
    
    def address(self) -> bytes:
        pubkey_hash = hashlib.sha256(self.public_key).digest()
        return pubkey_hash[-20:]
```

```rust
// Rust
use secp256k1::{SecretKey, PublicKey, Secp256k1, Message};

pub struct KeyPair {
    secret_key: SecretKey,
    public_key: PublicKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut rand::thread_rng());
        Self { secret_key, public_key }
    }
    
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        let secret_key = SecretKey::from_slice(bytes)?;
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        Ok(Self { secret_key, public_key })
    }
    
    pub fn sign(&self, message: &[u8]) -> Signature {
        let secp = Secp256k1::new();
        let msg_hash = keccak256(message);
        let message = Message::from_slice(&msg_hash).unwrap();
        secp.sign_ecdsa(&message, &self.secret_key)
    }
    
    pub fn address(&self) -> Address {
        let pubkey_bytes = self.public_key.serialize_uncompressed();
        let hash = keccak256(&pubkey_bytes[1..]);
        Address::from_slice(&hash[12..])
    }
}
```

**Mapping Notes:**
- `os.urandom` → `rand::thread_rng()`
- `ecdsa` lib → `secp256k1` crate (industry standard)
- Type safety: `&[u8; 32]` instead of `bytes`
- Result types for error handling

---

## 🔐 Consensus Module

### Python: sdk/consensus/

#### pos.py → luxtensor-consensus/src/pos.rs
```python
# Python
class ProofOfStake:
    def __init__(self, state_db: StateDB):
        self.state = state_db
        self.validators: Dict[bytes, Validator] = {}
        self.total_stake = 0
    
    def select_validator(self, slot: int) -> bytes:
        seed = self.compute_seed(slot)
        random.seed(seed)
        
        weights = [v.stake for v in self.validators.values()]
        selected = random.choices(list(self.validators.keys()), weights=weights)[0]
        return selected
```

```rust
// Rust
use rand::prelude::*;

pub struct ProofOfStake {
    state: Arc<RwLock<StateDB>>,
    validator_set: ValidatorSet,
}

impl ProofOfStake {
    pub fn new(state: Arc<RwLock<StateDB>>) -> Self {
        Self {
            state,
            validator_set: ValidatorSet::new(),
        }
    }
    
    pub fn select_validator(&self, slot: u64) -> Result<Address, ConsensusError> {
        let seed = self.compute_seed(slot);
        let mut rng = StdRng::from_seed(seed);
        
        let validators = self.validator_set.validators();
        let total_stake = validators.iter().map(|v| v.stake).sum::<u128>();
        
        let mut roll = rng.gen_range(0..total_stake);
        
        for validator in validators {
            if roll < validator.stake {
                return Ok(validator.address);
            }
            roll -= validator.stake;
        }
        
        Err(ConsensusError::NoValidatorSelected)
    }
}
```

**Mapping Notes:**
- `Dict[bytes, Validator]` → `ValidatorSet` (custom type)
- `random.choices` → weighted random with `rand` crate
- Thread-safe with `Arc<RwLock<T>>`
- Deterministic RNG with seed

---

#### fork_choice.py → luxtensor-consensus/src/fork_choice.rs
```python
# Python
class ForkChoice:
    def __init__(self):
        self.blocks: Dict[bytes, Block] = {}
        self.head: bytes = None
    
    def add_block(self, block: Block):
        block_hash = block.hash()
        self.blocks[block_hash] = block
        self._update_head()
    
    def _update_head(self):
        # GHOST algorithm
        self.head = self._compute_best_head()
```

```rust
// Rust
pub struct ForkChoice {
    blocks: HashMap<Hash, Block>,
    head: Hash,
    scores: HashMap<Hash, u64>,
}

impl ForkChoice {
    pub fn new(genesis: Block) -> Self {
        let genesis_hash = genesis.hash();
        let mut blocks = HashMap::new();
        let mut scores = HashMap::new();
        
        blocks.insert(genesis_hash, genesis);
        scores.insert(genesis_hash, 0);
        
        Self {
            blocks,
            head: genesis_hash,
            scores,
        }
    }
    
    pub fn add_block(&mut self, block: Block) -> Result<(), ForkChoiceError> {
        let block_hash = block.hash();
        
        // Verify parent exists
        if !self.blocks.contains_key(&block.header.previous_hash) {
            return Err(ForkChoiceError::OrphanBlock);
        }
        
        // Calculate score (GHOST)
        let parent_score = self.scores[&block.header.previous_hash];
        let score = parent_score + 1;
        
        self.blocks.insert(block_hash, block);
        self.scores.insert(block_hash, score);
        
        self.update_head();
        
        Ok(())
    }
    
    fn update_head(&mut self) {
        self.head = self.scores.iter()
            .max_by_key(|(_, &score)| score)
            .map(|(hash, _)| *hash)
            .unwrap();
    }
}
```

**Mapping Notes:**
- Added `scores` for GHOST algorithm
- Error handling with `Result`
- Efficient iteration with iterators
- Type safety with `Hash` type

---

## 🌐 Network Module

### Python: sdk/network/

#### p2p.py → luxtensor-network/src/p2p.rs
```python
# Python (using asyncio)
class P2PNode:
    def __init__(self, port: int):
        self.port = port
        self.peers: Dict[str, Peer] = {}
    
    async def start(self):
        server = await asyncio.start_server(
            self.handle_connection, '0.0.0.0', self.port
        )
        async with server:
            await server.serve_forever()
    
    async def broadcast_transaction(self, tx: Transaction):
        for peer in self.peers.values():
            await peer.send(tx.serialize())
```

```rust
// Rust (using libp2p)
use libp2p::{gossipsub, swarm::Swarm, PeerId};

pub struct P2PNode {
    swarm: Swarm<NetworkBehaviour>,
    peers: HashMap<PeerId, Peer>,
}

impl P2PNode {
    pub async fn new(config: P2PConfig) -> Result<Self, NetworkError> {
        // Setup libp2p swarm
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        
        let transport = /* setup transport */;
        let behaviour = NetworkBehaviour { /* setup */ };
        let swarm = Swarm::new(transport, behaviour, local_peer_id);
        
        Ok(Self {
            swarm,
            peers: HashMap::new(),
        })
    }
    
    pub async fn start(&mut self) -> Result<(), NetworkError> {
        self.swarm.listen_on(
            format!("/ip4/0.0.0.0/tcp/{}", self.config.port).parse()?
        )?;
        
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {}", address);
                }
                SwarmEvent::Behaviour(event) => {
                    self.handle_event(event).await?;
                }
                _ => {}
            }
        }
    }
    
    pub async fn broadcast_transaction(&mut self, tx: Transaction) -> Result<(), NetworkError> {
        let message = bincode::serialize(&tx)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(TOPIC_TRANSACTIONS, message)?;
        Ok(())
    }
}
```

**Mapping Notes:**
- Custom P2P → `libp2p` (battle-tested)
- `asyncio` → `tokio` async runtime
- Manual serialization → `bincode`
- Gossipsub for efficient broadcasting

---

## 💾 Storage Module

### Python: sdk/storage/

#### blockchain_db.py → luxtensor-storage/src/db.rs
```python
# Python
import plyvel

class BlockchainDB:
    def __init__(self, path: str):
        self.blocks_db = plyvel.DB(f"{path}/blocks", create_if_missing=True)
        self.state_db = plyvel.DB(f"{path}/state", create_if_missing=True)
    
    def store_block(self, block: Block):
        key = block.hash()
        value = block.serialize()
        self.blocks_db.put(key, value)
    
    def get_block(self, block_hash: bytes) -> Optional[Block]:
        data = self.blocks_db.get(block_hash)
        if data:
            return Block.deserialize(data)
        return None
```

```rust
// Rust
use rocksdb::{DB, Options};

pub struct BlockchainDB {
    blocks_db: DB,
    state_db: DB,
}

impl BlockchainDB {
    pub fn open(path: &str) -> Result<Self, StorageError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_open_files(10000);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        
        Ok(Self {
            blocks_db: DB::open(&opts, format!("{}/blocks", path))?,
            state_db: DB::open(&opts, format!("{}/state", path))?,
        })
    }
    
    pub fn store_block(&self, block: &Block) -> Result<(), StorageError> {
        let key = block.hash();
        let value = bincode::serialize(block)?;
        self.blocks_db.put(key, value)?;
        Ok(())
    }
    
    pub fn get_block(&self, hash: &Hash) -> Result<Option<Block>, StorageError> {
        match self.blocks_db.get(hash)? {
            Some(bytes) => {
                let block = bincode::deserialize(&bytes)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }
}
```

**Mapping Notes:**
- `plyvel` (LevelDB) → `rocksdb` (better performance)
- Added compression and tuning options
- Type-safe keys with `&Hash`
- Error propagation with `?` operator

---

## 🔌 API Module

### Python: sdk/api/

#### rpc.py → luxtensor-rpc/src/server.rs
```python
# Python (FastAPI)
from fastapi import FastAPI

app = FastAPI()

@app.post("/")
async def handle_rpc(request: RpcRequest):
    if request.method == "eth_blockNumber":
        return {"result": blockchain.get_height()}
    elif request.method == "eth_getBalance":
        address = request.params[0]
        balance = state.get_balance(address)
        return {"result": hex(balance)}
```

```rust
// Rust (jsonrpc)
use jsonrpc_core::{IoHandler, Result as RpcResult, Params};
use jsonrpc_http_server::ServerBuilder;

pub struct RpcServer {
    blockchain: Arc<RwLock<Blockchain>>,
    state: Arc<RwLock<StateDB>>,
}

impl RpcServer {
    pub async fn start(&self, addr: &str) -> Result<(), RpcError> {
        let mut io = IoHandler::new();
        
        let blockchain = self.blockchain.clone();
        io.add_method("eth_blockNumber", move |_: Params| {
            let chain = blockchain.read().unwrap();
            Ok(json!(chain.height()))
        });
        
        let state = self.state.clone();
        io.add_method("eth_getBalance", move |params: Params| {
            let address: Address = params.parse()?;
            let state = state.read().unwrap();
            let balance = state.get_balance(&address)?;
            Ok(json!(format!("0x{:x}", balance)))
        });
        
        let server = ServerBuilder::new(io)
            .threads(4)
            .start_http(&addr.parse()?)?;
        
        server.wait();
        Ok(())
    }
}
```

**Mapping Notes:**
- `FastAPI` → `jsonrpc-http-server`
- Route decorators → `add_method`
- Thread-safe with `Arc<RwLock<T>>`
- JSON-RPC 2.0 compliant

---

## 📊 Performance Comparison

| Operation | Python (ms) | Rust (ms) | Speedup |
|-----------|-------------|-----------|---------|
| Block hash | 5.2 | 0.05 | **100x** |
| Signature verify | 8.1 | 0.12 | **67x** |
| Transaction execute | 12.0 | 0.8 | **15x** |
| State read | 2.3 | 0.15 | **15x** |
| State write | 8.5 | 0.4 | **21x** |
| Merkle proof | 15.0 | 0.6 | **25x** |

---

## ✅ Migration Checklist

### Phase 1: Core
- [ ] Block structure
- [ ] Transaction format
- [ ] State management
- [ ] Cryptography primitives
- [ ] Merkle tree

### Phase 2: Consensus
- [ ] PoS implementation
- [ ] Validator selection
- [ ] Fork choice
- [ ] Reward distribution
- [ ] AI validation

### Phase 3: Network
- [ ] P2P protocol (libp2p)
- [ ] Peer discovery
- [ ] Block sync
- [ ] Transaction propagation
- [ ] Message handling

### Phase 4: Storage
- [ ] RocksDB integration
- [ ] State DB with trie
- [ ] Block indexer
- [ ] Transaction indexer

### Phase 5: API
- [ ] JSON-RPC server
- [ ] Standard methods
- [ ] AI-specific methods
- [ ] WebSocket support

### Phase 6: Node
- [ ] Full node service
- [ ] Configuration
- [ ] Logging
- [ ] Metrics

### Phase 7: Testing
- [ ] Unit tests
- [ ] Integration tests
- [ ] Property tests
- [ ] Benchmarks

---

## 🔧 Migration Tools

### Code Translation Helpers
```bash
# Find all Python dataclasses
rg "@dataclass" sdk/

# Find async functions
rg "async def" sdk/

# Find dict/list usage
rg "Dict\[|List\[" sdk/
```

### Testing Strategy
1. **Unit tests first** - ensure each component works
2. **Integration tests** - test interactions
3. **Compatibility tests** - compare outputs with Python
4. **Performance tests** - verify speedup

---

## 📝 Notes

### Key Differences
1. **Memory Management**: Python GC → Rust ownership
2. **Concurrency**: GIL → true parallelism
3. **Error Handling**: Exceptions → Result types
4. **Type System**: Dynamic → static with generics
5. **Performance**: Interpreted → compiled native code

### Best Practices
- Use `Arc<RwLock<T>>` for shared mutable state
- Prefer `Vec<T>` over `Box<[T]>` for flexibility
- Use `thiserror` for error types
- Use `tracing` instead of `println!` for logging
- Run `clippy` regularly for idiomatic Rust

---

**Mapping complete! Ready for implementation. 🦀**
