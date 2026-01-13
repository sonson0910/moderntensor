# Hướng Dẫn Chạy Luxtensor Node

## Mục Lục

1. [Yêu Cầu Hệ Thống](#yêu-cầu-hệ-thống)
2. [Cài Đặt](#cài-đặt)
3. [Cấu Hình](#cấu-hình)
4. [Chạy Node](#chạy-node)
5. [Kiểm Tra Trạng Thái](#kiểm-tra-trạng-thái)
6. [Troubleshooting](#troubleshooting)

---

## Yêu Cầu Hệ Thống

### Phần Cứng Tối Thiểu

| Thành phần | Tối thiểu | Khuyến nghị |
|------------|-----------|-------------|
| CPU | 4 cores | 8+ cores |
| RAM | 8 GB | 16+ GB |
| Disk | 100 GB SSD | 500+ GB NVMe |
| Network | 100 Mbps | 1 Gbps |

### Phần Mềm

- **OS**: Ubuntu 20.04+, Windows 10/11, macOS 12+
- **Rust**: 1.75+ (stable)
- **Git**: 2.30+

---

## Cài Đặt

### 1. Clone Repository

```bash
git clone https://github.com/sonson0910/moderntensor.git
cd moderntensor/luxtensor
```

### 2. Build từ Source

```bash
# Build release version
cargo build --release -p luxtensor-node

# Verify build
./target/release/luxtensor-node --version
```

### 3. Tạo Thư Mục Data

```bash
mkdir -p data/blocks data/state
```

---

## Cấu Hình

### File `config.toml`

```toml
[node]
name = "my-luxtensor-node"
role = "full"  # full, light, validator

[network]
listen_port = 30333
bootstrap_nodes = []

[rpc]
enabled = true
port = 8545
ws_port = 8546
external = false

[storage]
data_dir = "./data"
db_type = "rocksdb"

[consensus]
block_time = 6000
min_validators = 1
```

### Các Loại Node

| Loại | Mô tả | Config |
|------|-------|--------|
| **Full** | Lưu toàn bộ data | `role = "full"` |
| **Light** | Chỉ lưu headers | `role = "light"` |
| **Validator** | Tham gia consensus | `role = "validator"` |

---

## Chạy Node

### Windows (PowerShell)

```powershell
.\target\release\luxtensor-node.exe --config config.toml
```

### Linux/macOS

```bash
./target/release/luxtensor-node --config config.toml
```

### Với Log Chi Tiết

```bash
RUST_LOG=info ./target/release/luxtensor-node --config config.toml
```

### Chạy như Systemd Service (Linux)

```ini
# /etc/systemd/system/luxtensor.service
[Unit]
Description=Luxtensor Node
After=network.target

[Service]
Type=simple
User=luxtensor
WorkingDirectory=/opt/luxtensor
ExecStart=/opt/luxtensor/target/release/luxtensor-node --config config.toml
Restart=always

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable luxtensor
sudo systemctl start luxtensor
```

---

## Kiểm Tra Trạng Thái

### Kiểm Tra RPC

```bash
# Block number
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Chain ID
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'
```

### Kết quả mong đợi

```json
{"jsonrpc":"2.0","result":"0x539","id":1}
```

---

## Troubleshooting

| Lỗi | Nguyên nhân | Giải pháp |
|-----|------------|-----------|
| Address already in use | Port đang bị dùng | `lsof -i :8545` → kill process |
| Database corruption | Data bị hỏng | Backup và reset data folder |
| Out of memory | RAM không đủ | Giảm cache_size trong config |
| Not syncing | Mạng có vấn đề | Kiểm tra bootstrap nodes |

---

## Các Lệnh Hữu Ích

| Lệnh | Mô tả |
|------|-------|
| `--config <file>` | Chỉ định config file |
| `--base-path <dir>` | Thư mục data |
| `--rpc-port <port>` | Port RPC (default: 8545) |
| `--ws-port <port>` | Port WebSocket (default: 8546) |
| `--validator` | Chạy như validator |
| `--dev` | Dev mode |

---

## Liên Hệ

- Discord: [discord.gg/moderntensor](https://discord.gg/moderntensor)
- GitHub: [github.com/sonson0910/moderntensor](https://github.com/sonson0910/moderntensor)
