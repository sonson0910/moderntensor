# Installation

## 1. Install Dependencies

### Linux (Ubuntu/Debian)

```bash
sudo apt update
sudo apt install build-essential cmake clang pkg-config libssl-dev protobuf-compiler
```

### macOS

```bash
brew install cmake llvm protobuf
```

## 2. Install Rust

If you haven't already:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

## 3. Clone & Build

```bash
git clone https://github.com/sonson0910/luxtensor
cd luxtensor
cargo build --release
```

The binary will be located at `./target/release/luxtensor-node`.
