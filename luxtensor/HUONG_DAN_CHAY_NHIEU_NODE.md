# Hướng Dẫn Chạy Nhiều Node LuxTensor - Thiết Lập Mạng Local

Hướng dẫn này giải thích cách chạy nhiều node LuxTensor trên máy tính local của bạn để tạo một mạng test local. Điều này hữu ích cho việc phát triển, kiểm thử và hiểu cách các node giao tiếp với nhau.

## Mục Lục

- [Yêu Cầu](#yêu-cầu)
- [Bắt Đầu Nhanh](#bắt-đầu-nhanh)
- [Hướng Dẫn Thiết Lập Chi Tiết](#hướng-dẫn-thiết-lập-chi-tiết)
- [Giải Thích Cấu Hình](#giải-thích-cấu-hình)
- [Quản Lý Mạng Local](#quản-lý-mạng-local)
- [Xử Lý Sự Cố](#xử-lý-sự-cố)

## Yêu Cầu

Trước khi bắt đầu, đảm bảo bạn có:

- **Rust 1.75 trở lên** đã cài đặt ([rustup.rs](https://rustup.rs/))
- **Git** đã cài đặt
- **Ít nhất 2GB RAM** khả dụng
- **3 cửa sổ terminal** (hoặc sử dụng tmux/screen)

## Bắt Đầu Nhanh

### Bước 1: Build Dự Án

```bash
# Clone và build (nếu chưa làm)
cd /path/to/luxtensor
cargo build --release
```

### Bước 2: Tạo Thư Mục Cho Các Node

```bash
# Tạo thư mục cho 3 node
mkdir -p node1 node2 node3
```

### Bước 3: Copy File Cấu Hình

```bash
# Copy các file cấu hình mẫu
cp config.node1.toml node1/config.toml
cp config.node2.toml node2/config.toml
cp config.node3.toml node3/config.toml
```

### Bước 4: Khởi Động Các Node

Mở 3 cửa sổ terminal riêng biệt và chạy:

**Terminal 1 - Node 1:**
```bash
cd node1
../target/release/luxtensor-node --config config.toml
```

**Terminal 2 - Node 2:**
```bash
cd node2
../target/release/luxtensor-node --config config.toml
```

**Terminal 3 - Node 3:**
```bash
cd node3
../target/release/luxtensor-node --config config.toml
```

## Hướng Dẫn Thiết Lập Chi Tiết

### Hiểu Về Cấu Hình Node

Mỗi node cần có riêng:
1. **Thư mục data** - Nơi lưu trữ dữ liệu blockchain
2. **Cổng network** - Cổng giao tiếp P2P (phải là duy nhất)
3. **Cổng RPC** - Cổng JSON-RPC API (phải là duy nhất)

### File Cấu Hình

Repository bao gồm các cấu hình mẫu để chạy 3 node:

- `config.node1.toml` - Cấu hình Node 1 (Cổng: P2P=30303, RPC=8545)
- `config.node2.toml` - Cấu hình Node 2 (Cổng: P2P=30304, RPC=8555)
- `config.node3.toml` - Cấu hình Node 3 (Cổng: P2P=30305, RPC=8565)

### Các Điểm Khác Biệt Chính Trong Cấu Hình

Mỗi cấu hình node khác nhau ở:

```toml
[node]
name = "node-1"  # Tên duy nhất cho mỗi node
data_dir = "./data"  # Thư mục data local

[network]
listen_port = 30303  # Cổng P2P duy nhất (30303, 30304, 30305)
enable_mdns = true   # Bật tính năng tự động tìm node trên mạng local

[rpc]
listen_port = 8545   # Cổng RPC duy nhất (8545, 8555, 8565)
```

### Khám Phá Node

Các node sẽ tự động tìm thấy nhau thông qua mDNS (Multicast DNS) vì chúng ở trên cùng một mạng local. Tính năng này được bật bởi:

```toml
[network]
enable_mdns = true
```

Để kết nối peer thủ công, bạn có thể chỉ định bootstrap nodes sau khi lấy được node ID.

## Giải Thích Cấu Hình

### Phần Node
- **name**: Tên nhận dạng dễ đọc cho node
- **chain_id**: Phải giống nhau cho tất cả các node (1 cho local dev)
- **data_dir**: Nơi node lưu trữ dữ liệu blockchain
- **is_validator**: Đặt `true` nếu node này tham gia vào consensus

### Phần Network
- **listen_addr**: "0.0.0.0" cho phép kết nối từ tất cả các interface
- **listen_port**: Cổng P2P cho giao tiếp node (phải duy nhất cho mỗi node)
- **bootstrap_nodes**: Danh sách các seed node để kết nối ban đầu
- **max_peers**: Số lượng tối đa kết nối peer (mặc định: 50)
- **enable_mdns**: Tự động khám phá peer trên mạng local

### Phần Storage
- **db_path**: Vị trí cơ sở dữ liệu RocksDB
- **enable_compression**: Nén dữ liệu được lưu trữ (khuyến nghị: true)
- **cache_size**: Kích thước cache bộ nhớ tính bằng MB (mặc định: 256)

### Phần RPC
- **enabled**: Bật máy chủ JSON-RPC HTTP
- **listen_addr**: "127.0.0.1" chỉ cho local, "0.0.0.0" cho tất cả interface
- **listen_port**: Cổng HTTP API (phải duy nhất cho mỗi node)
- **cors_origins**: Chính sách CORS (["*"] cho development)

## Quản Lý Mạng Local

### Kiểm Tra Trạng Thái Node

Truy vấn trạng thái của mỗi node sử dụng RPC API:

```bash
# Trạng thái Node 1
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"lux_blockNumber","params":[],"id":1}'

# Trạng thái Node 2
curl -X POST http://localhost:8555 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"lux_blockNumber","params":[],"id":1}'

# Trạng thái Node 3
curl -X POST http://localhost:8565 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"lux_blockNumber","params":[],"id":1}'
```

### Xem Kết Nối Peer

Kiểm tra các peer đã kết nối:

```bash
# Kiểm tra peer của Node 1
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}'
```

### Theo Dõi Log

Mỗi node xuất log ra stdout. Chú ý các thông báo:
- ✅ "Node started" - Khởi tạo node thành công
- ✅ "Peer connected" - Kết nối peer thành công
- ✅ "Block received" - Nhận block từ peer
- ✅ "Block produced" - Node tạo ra block mới (validators)

### Dừng Các Node

Nhấn `Ctrl+C` trong mỗi terminal để tắt node một cách an toàn.

### Dọn Dẹp

Để bắt đầu lại với trạng thái sạch:

```bash
# Dừng tất cả node trước, sau đó:
rm -rf node1/data node2/data node3/data
```

## Sử Dụng Script Hỗ Trợ

### Khởi Động Tất Cả Node (Sử Dụng tmux)

Tạo script `start-nodes.sh`:

```bash
#!/bin/bash

# Khởi động 3 node trong các cửa sổ tmux riêng biệt
tmux new-session -d -s luxtensor 'cd node1 && ../target/release/luxtensor-node --config config.toml'
tmux split-window -h 'cd node2 && ../target/release/luxtensor-node --config config.toml'
tmux split-window -v 'cd node3 && ../target/release/luxtensor-node --config config.toml'
tmux select-layout tiled
tmux attach-session -t luxtensor
```

Làm cho nó có thể thực thi:
```bash
chmod +x start-nodes.sh
./start-nodes.sh
```

### Dừng Tất Cả Node

Tạo script `stop-nodes.sh`:

```bash
#!/bin/bash

# Dừng tất cả process luxtensor-node
pkill -SIGTERM luxtensor-node
echo "Đã dừng tất cả node"
```

## Cấu Hình Nâng Cao

### Chạy Các Node Validator

Để chạy node như validator:

1. Tạo key validator cho mỗi node:
```bash
./target/release/luxtensor validator keygen --output node1/validator.key
./target/release/luxtensor validator keygen --output node2/validator.key
./target/release/luxtensor validator keygen --output node3/validator.key
```

2. Cập nhật cấu hình của mỗi node:
```toml
[node]
is_validator = true
validator_key_path = "./validator.key"
```

3. Stake token (cần có token trong tài khoản):
```bash
./target/release/luxtensor stake --amount 10000000000000000000 --rpc http://localhost:8545
```

### Cấu Hình Genesis Tùy Chỉnh

Để có một mạng local tùy chỉnh với trạng thái khởi tạo cụ thể:

1. Tạo file cấu hình genesis
2. Tất cả node phải sử dụng cùng một file genesis
3. Chỉ định file genesis trong cấu hình node

## Xử Lý Sự Cố

### Cổng Đã Được Sử Dụng

**Lỗi**: "Address already in use"

**Giải pháp**: Một process khác đang sử dụng cổng. Hoặc:
- Dừng process khác: `lsof -ti:8545 | xargs kill`
- Thay đổi cổng trong file cấu hình

### Node Không Tìm Thấy Nhau

**Vấn đề**: Các node vẫn bị cô lập

**Giải pháp**:
1. Đảm bảo `enable_mdns = true` trong tất cả cấu hình node
2. Kiểm tra cài đặt firewall - cho phép UDP multicast (5353)
3. Thêm kết nối peer thủ công bằng bootstrap_nodes
4. Đảm bảo tất cả node ở trên cùng một network interface

### Lỗi Khóa Database

**Lỗi**: "Database is locked" hoặc "Cannot acquire lock"

**Giải pháp**: 
- Chỉ một node có thể sử dụng một thư mục data tại một thời điểm
- Đảm bảo bạn đang sử dụng các thư mục data khác nhau cho mỗi node
- Kiểm tra xem process node khác có còn đang chạy: `ps aux | grep luxtensor-node`

### Sử Dụng CPU Cao

**Vấn đề**: Node tiêu thụ quá nhiều CPU

**Giải pháp**:
- Điều này bình thường trong quá trình sync ban đầu
- Giảm `max_peers` trong cấu hình
- Tăng `block_time` trong cấu hình consensus

### Vấn Đề Bộ Nhớ

**Vấn đề**: Lỗi hết bộ nhớ

**Giải pháp**:
- Giảm `cache_size` trong cấu hình storage (ví dụ: từ 256 xuống 128 MB)
- Bật pruning trong cấu hình
- Đóng các ứng dụng khác để giải phóng bộ nhớ

### Kiểm Tra Log Chi Tiết

Để logging chi tiết:

```bash
# Đặt log level thành debug trong config.toml
[logging]
level = "debug"

# Hoặc sử dụng biến môi trường
RUST_LOG=debug ./target/release/luxtensor-node --config config.toml
```

## Ví Dụ Về Topology Mạng

### Topology Tuyến Tính
```
Node1 <-> Node2 <-> Node3
```
Đặt bootstrap_nodes để kết nối tuần tự.

### Topology Hình Sao
```
    Node1
    /  \
Node2  Node3
```
Node2 và Node3 kết nối đến Node1 như bootstrap.

### Full Mesh
```
Node1 <-> Node2
  \      /
   Node3
```
Tất cả node tự động tìm thấy nhau qua mDNS.

## Mẹo Hiệu Suất

1. **Lưu trữ SSD**: Sử dụng SSD cho thư mục data để có hiệu suất I/O tốt hơn
2. **Bộ nhớ**: Phân bổ đủ RAM (512MB-1GB cho mỗi node)
3. **CPU**: Bộ vi xử lý đa nhân hưởng lợi từ xử lý block song song
4. **Mạng**: Sử dụng kết nối có dây để ổn định

## Các Bước Tiếp Theo

Sau khi chạy thành công mạng local của bạn:

1. **Tương tác với node** sử dụng công cụ CLI
2. **Gửi transaction** giữa các node
3. **Deploy smart contract** trên mạng local
4. **Kiểm tra consensus** bằng cách dừng/khởi động validator
5. **Theo dõi hiệu suất** sử dụng metrics endpoint

## Tài Nguyên Bổ Sung

- [README Chính](README.md) - Tổng quan dự án và tính năng
- [Hướng Dẫn Data Sync](DATA_SYNC_TEST_GUIDE.md) - Hiểu về đồng bộ node
- [Tài Liệu API](docs/api.md) - Tham khảo RPC API
- [Ví Dụ](examples/) - Các ví dụ code để tương tác với node

## Hỗ Trợ

Đối với các vấn đề hoặc câu hỏi:
- Mở issue trên GitHub: https://github.com/sonson0910/luxtensor/issues
- Kiểm tra tài liệu hiện có trong thư mục `/docs`
- Xem các test case trong `/crates/luxtensor-tests`

---

**Chúc bạn chạy node thành công! 🚀**
