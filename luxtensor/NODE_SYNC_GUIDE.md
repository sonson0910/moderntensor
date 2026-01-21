# 🌐 Luxtensor Node Sync & Multi-Machine Deployment

## Tổng Quan

Luxtensor hỗ trợ sync blockchain giữa nhiều máy tính qua:

1. **mDNS Discovery** - Tự động tìm nodes trong cùng mạng LAN
2. **Bootstrap Nodes** - Kết nối tới nodes cố định qua Internet
3. **Persistent Peer ID** - Mỗi node có ID cố định để cấu hình bootstrap

---

## 🔑 Persistent Peer ID (MỚI!)

### Vấn đề trước đây

- Mỗi lần restart node, Peer ID random mới được tạo
- Không thể cấu hình bootstrap_nodes cố định

### Giải pháp

- Node key được lưu vào file `node.key` trong data directory
- Peer ID giữ nguyên sau khi restart
- Có thể dùng Peer ID này trong bootstrap_nodes của nodes khác

### Cách hoạt động

```
Lần 1: Node start
├── Không tìm thấy ./data/node.key
├── Tạo keypair mới
├── Lưu vào ./data/node.key
└── In ra Peer ID: 12D3KooWHxU...

Lần 2+: Node restart
├── Tìm thấy ./data/node.key
├── Load keypair từ file
└── Peer ID giống lần 1: 12D3KooWHxU...
```

---

## 🚀 Quick Start: Chạy 2+ Machines

### Step 1: Khởi động Seed Node (Máy 1)

```bash
# Build
cd luxtensor
cargo build --release

# Tạo thư mục và config
mkdir -p seed_node
cp config.node1.toml seed_node/config.toml

# Start node
cd seed_node
../target/release/luxtensor-node --config config.toml
```

Output:

```
╔═══════════════════════════════════════════════════════════════╗
║                    🔗 Node Connection Info                     ║
╠═══════════════════════════════════════════════════════════════╣
║ Peer ID: 12D3KooWHxUxbJpYmF...
║ Full ID: 12D3KooWHxUxbJpYmFtKD5R6m...
╠═══════════════════════════════════════════════════════════════╣
║ To connect other nodes, add this to their config:             ║
╠═══════════════════════════════════════════════════════════════╣
║ bootstrap_nodes = [                                           ║
║   "/ip4/YOUR_IP/tcp/30303/p2p/12D3KooWHxUxbJpYmFtKD5R6m..."  ║
║ ]                                                             ║
╚═══════════════════════════════════════════════════════════════╝
```

**Ghi lại Peer ID này!**

### Step 2: Cấu hình và chạy Node khác (Máy 2)

```bash
# Trên máy 2
cd luxtensor
cargo build --release

mkdir -p node2
cp config.node2.toml node2/config.toml
```

**Sửa `node2/config.toml`:**

```toml
[network]
listen_port = 30303

# THAY BẰNG IP VÀ PEER_ID CỦA SEED NODE (MÁY 1)
bootstrap_nodes = [
    "/ip4/192.168.1.100/tcp/30303/p2p/12D3KooWHxUxbJpYmFtKD5R6m..."
]

# Tắt mDNS nếu qua Internet (khác mạng LAN)
enable_mdns = false
```

```bash
# Start node
cd node2
../target/release/luxtensor-node --config config.toml
```

### Step 3: Verify Connection

```bash
# Kiểm tra peers đã kết nối
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}'

# Kết quả: {"result":"0x1"} = 1 peer đã kết nối

# Kiểm tra block sync
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

---

## 📋 Config Options

### [network] Section

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `listen_addr` | string | "0.0.0.0" | Địa chỉ lắng nghe |
| `listen_port` | u16 | 30303 | Port P2P |
| `bootstrap_nodes` | array | [] | Danh sách seed nodes |
| `max_peers` | usize | 50 | Số peers tối đa |
| `enable_mdns` | bool | true | Bật mDNS discovery |
| `node_key_path` | string | null | Path tới node.key file |

### Bootstrap Node Format

```
/ip4/IP_ADDRESS/tcp/PORT/p2p/PEER_ID
```

Ví dụ:

```
/ip4/203.0.113.10/tcp/30303/p2p/12D3KooWHxUxbJpYmFtKD5R6mW1vxC...
```

---

## 🔧 Scenarios

### Scenario 1: Cùng mạng LAN

```toml
# Node 1, 2, 3 - cùng LAN
[network]
enable_mdns = true
bootstrap_nodes = []
```

→ Tự động tìm nhau qua mDNS

### Scenario 2: Qua Internet

```toml
# Seed Node (public IP: 203.0.113.10)
[network]
listen_port = 30303
enable_mdns = false
bootstrap_nodes = []

# Other Nodes
[network]
listen_port = 30303
enable_mdns = false
bootstrap_nodes = [
    "/ip4/203.0.113.10/tcp/30303/p2p/12D3KooW..."
]
```

### Scenario 3: Hybrid (LAN + Internet)

```toml
# Enable cả mDNS và bootstrap
[network]
enable_mdns = true
bootstrap_nodes = [
    "/ip4/203.0.113.10/tcp/30303/p2p/12D3KooW..."
]
```

---

## 🔥 Firewall

Mở port TCP:

```bash
# Ubuntu/Debian
sudo ufw allow 30303/tcp
sudo ufw allow 8545/tcp  # RPC (optional, restrict IP)

# Windows
netsh advfirewall firewall add rule name="Luxtensor P2P" dir=in action=allow protocol=tcp localport=30303
```

---

## ❓ Troubleshooting

### Nodes không kết nối được

1. **Kiểm tra firewall**: Port 30303 phải mở
2. **Kiểm tra IP**: Dùng public IP nếu qua Internet
3. **Kiểm tra Peer ID format**: Phải là 12D3KooW...
4. **Kiểm tra logs**:

   ```bash
   # Set debug mode
   RUST_LOG=debug ./luxtensor-node --config config.toml
   ```

### Block không sync

1. **Kiểm tra chain_id**: Phải giống nhau trên tất cả nodes
2. **Kiểm tra genesis**: Phải dùng cùng genesis config
3. **Kiểm tra validators list**: Phải giống nhau

### Peer ID thay đổi mỗi lần restart

- Kiểm tra file `./data/node.key` có tồn tại không
- Kiểm tra permissions: Node phải có quyền đọc/ghi vào data_dir

---

## 📊 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Luxtensor Network                       │
│                                                             │
│  ┌─────────────┐       ┌─────────────┐       ┌───────────┐ │
│  │   Seed Node │◄─────►│   Node 2    │◄─────►│   Node 3  │ │
│  │  (PUBLIC IP)│       │  (Any IP)   │       │  (Any IP) │ │
│  └─────────────┘       └─────────────┘       └───────────┘ │
│        │                     │                     │       │
│        │       bootstrap_nodes connects here       │       │
│        ▼                     ▼                     ▼       │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                   Gossipsub Topics                  │   │
│  │   • luxtensor/blocks/1.0.0                         │   │
│  │   • luxtensor/transactions/1.0.0                   │   │
│  │   • luxtensor/sync/1.0.0                           │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## ✅ Checklist Triển khai

- [ ] Build binary trên mỗi máy: `cargo build --release`
- [ ] Tạo data directory cho node
- [ ] Copy và sửa config file
- [ ] Start seed node đầu tiên
- [ ] Ghi lại Peer ID của seed node
- [ ] Cấu hình bootstrap_nodes cho các node khác
- [ ] Mở firewall port 30303
- [ ] Verify connection qua RPC

---

*Cập nhật: 2026-01-21*
