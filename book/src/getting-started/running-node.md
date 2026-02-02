# Running a Node

## Basic Usage

Start a full node connected to the default network (testnet):

```bash
./target/release/luxtensor-node
```

## Configuration

You can configure the node using command line flags or a config file.

```bash
# Custom P2P port
./target/release/luxtensor-node --p2p-port 30333

# Custom RPC port
./target/release/luxtensor-node --rpc-port 8545

# Connect to specific bootnode
./target/release/luxtensor-node --bootnodes /ip4/1.2.3.4/tcp/30333/p2p/Qm...
```

## Resetting Chain Data

To wipe the database and start fresh (useful for devnet):

```bash
rm -rf ~/.luxtensor/data
```
