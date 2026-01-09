# Lộ Trình Xây Dựng mtcli (ModernTensor CLI)

**Ngày:** 9 Tháng 1, 2026  
**Trạng Thái:** Phase 1 Hoàn Thành (30%)  
**Mục Tiêu:** Xây dựng CLI hoàn chỉnh cho Luxtensor blockchain

---

## 📊 Tổng Quan

mtcli (ModernTensor CLI) là công cụ dòng lệnh để tương tác với blockchain Luxtensor, được xây dựng dựa trên kinh nghiệm từ btcli của Bittensor nhưng được tối ưu hóa cho kiến trúc ModernTensor.

### Kiến Trúc So Sánh

```
BITTENSOR                           MODERNTENSOR
┌──────────────────┐               ┌──────────────────┐
│  btcli (Typer)   │               │  mtcli (Click)   │
│  - wallet        │               │  - wallet ✅     │
│  - stake         │               │  - stake 🚧      │
│  - subnets       │               │  - subnet 🚧     │
│  - root          │               │  - validator 🚧  │
│  - sudo          │               │  - query 🚧      │
│  - weights       │               │  - tx 🚧         │
└──────────────────┘               └──────────────────┘
        ↓                                   ↓
┌──────────────────┐               ┌──────────────────┐
│    Subtensor     │               │    Luxtensor     │
│   (Substrate)    │               │  (Custom L1)     │
└──────────────────┘               └──────────────────┘
```

---

## ✅ Đã Hoàn Thành (Phase 1 - 30%)

### 1. Core CLI Framework

✅ **Hoàn thành 100%**

- Framework Click với command groups
- Rich console output (bảng, màu sắc, panel)
- Configuration management (YAML)
- Error handling và logging
- Version management

**Files:**
- `sdk/cli/main.py` - Entry point chính
- `sdk/cli/utils.py` - Utilities và helpers
- `sdk/cli/config.py` - Configuration management

### 2. Key Management Module

✅ **Hoàn thành 100%**

- BIP39 mnemonic generation (12/24 từ)
- BIP44 HD key derivation
- Password-based encryption (PBKDF2 + Fernet)
- Ethereum-compatible addresses
- Keypair generation

**Files:**
- `sdk/keymanager/key_generator.py` - Key generation
- `sdk/keymanager/encryption.py` - Encryption/decryption

### 3. Wallet Commands (Partial)

✅ **Hoàn thành 40%**

**Commands hoạt động:**
```bash
# Tạo coldkey mới
mtcli wallet create-coldkey --name my_coldkey

# Khôi phục từ mnemonic
mtcli wallet restore-coldkey --name restored_key

# Tạo hotkey
mtcli wallet generate-hotkey --coldkey my_coldkey --hotkey-name miner_hk1

# Liệt kê wallets
mtcli wallet list
```

**Commands cần implement:**
- [ ] `import-hotkey` - Import hotkey từ file
- [ ] `regen-hotkey` - Tái tạo hotkey từ index
- [ ] `list-hotkeys` - Liệt kê tất cả hotkeys
- [ ] `show-hotkey` - Hiển thị thông tin hotkey
- [ ] `show-address` - Hiển thị địa chỉ
- [ ] `query-address` - Query balance từ network
- [ ] `register-hotkey` - Đăng ký trên network

### 4. Utility Commands (Partial)

✅ **Hoàn thành 50%**

**Commands hoạt động:**
```bash
# Convert đơn vị
mtcli utils convert --from-mdt 1.5

# Generate keypair test
mtcli utils generate-keypair

# Version info
mtcli utils version
```

**Commands cần implement:**
- [ ] `latency` - Test network latency
- [ ] `connection` - Test node connections

---

## 🚧 Đang Thực Hiện (Phase 2 - Target: 2 tuần)

### 1. Hoàn Thiện Wallet Commands (Week 1)

**Priority: 🔴 HIGH**

#### A. List & Show Commands
```bash
mtcli wallet list-hotkeys --coldkey my_coldkey
mtcli wallet show-hotkey --coldkey my_coldkey --hotkey miner_hk1
mtcli wallet show-address --coldkey my_coldkey --hotkey miner_hk1 --network testnet
```

**Implementation:**
- Load hotkeys từ `hotkeys.json`
- Display formatted tables
- Show derivation paths
- Display addresses và public keys

#### B. Query Commands (Integration với LuxtensorClient)
```bash
mtcli wallet query-address --coldkey my_coldkey --network testnet
```

**Implementation:**
- Integrate với `sdk/luxtensor_client.py`
- Query balance, nonce, stake từ blockchain
- Display formatted output
- Cache results

#### C. Register Commands (Transaction Submission)
```bash
mtcli wallet register-hotkey \
  --coldkey my_coldkey \
  --hotkey miner_hk1 \
  --subnet-uid 1 \
  --initial-stake 10000000 \
  --api-endpoint "http://123.45.67.89:8080" \
  --network testnet
```

**Implementation:**
- Build transaction để register
- Sign transaction với private key
- Submit lên blockchain
- Monitor transaction status

**Dependencies:**
- LuxtensorClient methods
- Transaction builder
- Signing utilities

### 2. Query Commands (Week 2)

**Priority: 🔴 HIGH**

#### A. Address Queries
```bash
# Query bất kỳ address nào
mtcli query address addr_test1... --network testnet

# Query balance
mtcli query balance --coldkey my_coldkey --hotkey miner_hk1 --network testnet

# Query UTxOs (nếu có)
mtcli query utxos --coldkey my_coldkey --hotkey miner_hk1 --network testnet
```

**Implementation:**
- Use LuxtensorClient.get_account_info()
- Use LuxtensorClient.get_balance()
- Format output với Rich tables
- Add caching

#### B. Subnet Queries
```bash
# Query subnet info
mtcli query subnet --subnet-uid 1 --network testnet

# List all subnets
mtcli query list-subnets --network testnet
```

**Implementation:**
- Use LuxtensorClient subnet methods
- Display subnet metadata
- Show participant counts
- Show emission schedules

#### C. Validator/Miner Queries
```bash
# Query validator
mtcli query validator <address> --network testnet

# Query miner
mtcli query miner <address> --network testnet
```

**Implementation:**
- Query validator info từ blockchain
- Show stake, rewards, performance
- Show active status
- Format với tables

---

## 📅 Phase 3: Transaction Commands (Week 3-4)

**Priority: 🟡 MEDIUM**

### 1. Send Transactions
```bash
# Send tokens
mtcli tx send \
  --coldkey my_coldkey \
  --hotkey miner_hk1 \
  --to recipient_address \
  --amount 5000000 \
  --network testnet
```

**Implementation:**
- Transaction builder
- Gas estimation
- Transaction signing
- Broadcast và monitor
- Receipt verification

### 2. Transaction History
```bash
# View history
mtcli tx history --coldkey my_coldkey --hotkey miner_hk1 --limit 10

# Check status
mtcli tx status <tx_hash> --network testnet
```

**Implementation:**
- Query transactions từ indexer
- Parse transaction data
- Display formatted history
- Show pending/confirmed status

---

## 📅 Phase 4: Staking Commands (Week 5-6)

**Priority: 🔴 HIGH**

### 1. Stake Management
```bash
# Add stake
mtcli stake add \
  --coldkey my_coldkey \
  --hotkey validator_hk \
  --amount 1000000 \
  --network testnet

# Remove stake
mtcli stake remove \
  --coldkey my_coldkey \
  --hotkey validator_hk \
  --amount 500000 \
  --network testnet

# Claim rewards
mtcli stake claim \
  --coldkey my_coldkey \
  --hotkey validator_hk \
  --network testnet
```

**Implementation:**
- Integration với Luxtensor staking pallet
- Transaction building cho stake operations
- Reward calculation
- Unbonding period handling

### 2. Stake Information
```bash
# Show staking info
mtcli stake info --coldkey my_coldkey --hotkey validator_hk

# List all stakes
mtcli stake list --network testnet
```

**Implementation:**
- Query staking state
- Show validator list
- Display APY/rewards
- Show stake distribution

---

## 📅 Phase 5: Subnet Commands (Week 7-8)

**Priority: 🟡 MEDIUM**

### 1. Subnet Management
```bash
# Create subnet
mtcli subnet create --coldkey my_coldkey --name "My Subnet"

# Register on subnet
mtcli subnet register \
  --coldkey my_coldkey \
  --hotkey miner_hk1 \
  --subnet-uid 1

# Show info
mtcli subnet info --subnet-uid 1

# List participants
mtcli subnet participants --subnet-uid 1
```

**Implementation:**
- Subnet creation transactions
- Registration logic
- Parameter updates
- Participant queries

---

## 📅 Phase 6: Validator Commands (Week 9-10)

**Priority: 🔴 HIGH**

### 1. Validator Operations
```bash
# Start validator
mtcli validator start \
  --coldkey my_coldkey \
  --hotkey validator_hk \
  --network testnet

# Stop validator
mtcli validator stop

# Check status
mtcli validator status
```

**Implementation:**
- Validator node management
- Process monitoring
- Health checks
- Performance metrics

### 2. Weight Management
```bash
# Set weights
mtcli validator set-weights \
  --coldkey my_coldkey \
  --hotkey validator_hk \
  --subnet-uid 1 \
  --weights weights.json
```

**Implementation:**
- Weight calculation
- Weight submission transactions
- Validation logic
- Consensus participation

---

## 📅 Phase 7: Testing & Polish (Week 11-12)

**Priority: 🔴 HIGH**

### 1. Testing

- [ ] Unit tests cho tất cả commands
- [ ] Integration tests với testnet
- [ ] E2E test scenarios
- [ ] Performance testing
- [ ] Security testing

### 2. Documentation

- [ ] User guide hoàn chỉnh
- [ ] API documentation
- [ ] Examples và tutorials
- [ ] Vietnamese documentation
- [ ] Video tutorials

### 3. Polish

- [ ] Error messages improvements
- [ ] Better progress indicators
- [ ] Confirmation prompts
- [ ] Logging system
- [ ] Debug mode

---

## 🎯 Mục Tiêu Cụ Thể

### Q1 2026 (Tháng 1-3)

**Tháng 1 (Hiện Tại):**
- ✅ Phase 1: Core framework (Complete)
- 🚧 Phase 2: Wallet & Query commands

**Tháng 2:**
- Phase 3: Transaction commands
- Phase 4: Staking commands
- Integration testing

**Tháng 3:**
- Phase 5: Subnet commands
- Phase 6: Validator commands
- Phase 7: Testing & Polish
- Documentation
- Release v1.0.0

---

## 📊 So Sánh với btcli

| Feature | btcli | mtcli | Status |
|---------|-------|-------|--------|
| **Wallet Management** | ✅ | 🟡 40% | Phase 1-2 |
| **Staking** | ✅ | ⚪ 0% | Phase 4 |
| **Queries** | ✅ | ⚪ 0% | Phase 2 |
| **Transactions** | ✅ | ⚪ 0% | Phase 3 |
| **Subnet Management** | ✅ | ⚪ 0% | Phase 5 |
| **Validator Ops** | ✅ | ⚪ 0% | Phase 6 |
| **Root/Sudo** | ✅ | ⚪ N/A | Not needed |
| **Weights** | ✅ | ⚪ 0% | Phase 6 |
| **Configuration** | ✅ | ✅ 100% | Complete |
| **Output Format** | ✅ | ✅ 100% | Complete |

---

## 🔧 Công Nghệ Sử Dụng

### Framework & Libraries

1. **Click** - CLI framework (thay vì Typer của btcli)
   - Linh hoạt hơn
   - Control tốt hơn
   - Ecosystem lớn

2. **Rich** - Terminal output
   - Beautiful tables
   - Progress bars
   - Syntax highlighting
   - Colors và styling

3. **BIP Utils** - Key derivation
   - BIP39 mnemonic
   - BIP44 HD derivation
   - Compatible với standards

4. **eth-account** - Ethereum compatibility
   - Key generation
   - Address derivation
   - Transaction signing

5. **Cryptography** - Security
   - PBKDF2 key derivation
   - Fernet encryption
   - Secure password handling

### Integration với SDK

```python
# LuxtensorClient
from sdk.luxtensor_client import LuxtensorClient

client = LuxtensorClient(network='testnet')
balance = client.get_balance(address)

# AsyncLuxtensorClient
from sdk.async_luxtensor_client import AsyncLuxtensorClient

async_client = AsyncLuxtensorClient(network='testnet')
info = await async_client.get_account_info(address)

# Key Management
from sdk.keymanager import KeyGenerator

kg = KeyGenerator()
mnemonic = kg.generate_mnemonic()
hotkey = kg.derive_hotkey(mnemonic, index=0)
```

---

## 📝 Notes

### Khác Biệt với btcli

1. **Framework:** Click thay vì Typer
   - Click: Mature, flexible, widely used
   - Typer: Newer, type-based, FastAPI style

2. **Key Derivation:** Ethereum-compatible
   - Path: m/44'/60'/0'/0/index
   - Compatible với MetaMask, web3

3. **Storage:** Simpler structure
   - `.moderntensor/wallets/`
   - `.moderntensor/config.yaml`

4. **Commands:** Organized differently
   - `wallet`, `stake`, `query`, `tx`
   - No `root` or `sudo` (different governance)

### Ưu Điểm của mtcli

1. ✅ **Simpler:** Dễ hiểu, dễ maintain
2. ✅ **Modern:** Latest dependencies
3. ✅ **Secure:** Strong encryption
4. ✅ **Compatible:** Ethereum ecosystem
5. ✅ **Documented:** Vietnamese + English

### Thử Nghiệm

```bash
# Install
pip install -e .

# Test version
mtcli --version

# Test wallet
mtcli wallet create-coldkey --name test_key

# Help
mtcli --help
mtcli wallet --help
```

---

## 🎯 Kết Luận

mtcli đang trên đà phát triển tốt với Phase 1 đã hoàn thành. Core framework và key management đã sẵn sàng. Tiếp theo sẽ focus vào:

1. **Week 1-2:** Hoàn thiện wallet và query commands
2. **Week 3-4:** Transaction commands
3. **Week 5-6:** Staking commands
4. **Week 7-8:** Subnet commands
5. **Week 9-10:** Validator commands
6. **Week 11-12:** Testing và documentation

**Target:** Release v1.0.0 vào cuối tháng 3/2026

---

**Tài Liệu Tham Khảo:**
- btcli: https://github.com/opentensor/btcli
- Click: https://click.palletsprojects.com/
- Rich: https://rich.readthedocs.io/
- BIP39/44: https://github.com/bitcoin/bips
