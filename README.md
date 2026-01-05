# ModernTensor ✨

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) <!-- Or Apache 2.0, depending on your choice -->

**ModernTensor** is an independent Layer 1 blockchain designed for decentralized machine intelligence. The network enables AI/ML models to compete, validate, and earn rewards through zero-knowledge proofs and Proof of Stake consensus, inspired by the vision of Bittensor but with a custom blockchain optimized for AI workloads.

![moderntensor.png](https://github.com/sonson0910/moderntensor/blob/main/moderntensor.png)

## 🚀 Introduction

**ModernTensor** is a **custom Layer 1 blockchain** optimized for AI/ML workloads with native support for zero-knowledge machine learning and decentralized AI validation.

### 🎯 Current Focus: Building Layer 1 Blockchain

**Status:** 83% complete - **AHEAD OF SCHEDULE!**  
**Target:** Mainnet Q1 2026 (2 months)  
**Priority:** Phase 9 Mainnet Launch

In the ModernTensor ecosystem:

*   **Miners:** Provide AI/ML services/models via API endpoints. They register their hotkey (representing the miner's identifier - UID) onto the network.
*   **Validators:** Evaluate the quality and performance of Miners, contributing to the consensus mechanism and reward distribution.
*   **Custom L1 Blockchain:** Independent blockchain with PoS consensus, optimized for AI validation and incentive distribution.

### Layer 1 Development Status

- ✅ **Phase 1:** On-Chain State Optimization (Complete)
- ✅ **Phase 2:** Core Blockchain - Block, Transaction, State (Complete)
- ✅ **Phase 3:** Consensus Layer - PoS (Complete)
- ✅ **Phase 4:** Network Layer - P2P (Complete)
- ✅ **Phase 5:** Storage Layer - LevelDB (Complete)
- ✅ **Phase 6:** RPC & API - JSON-RPC, GraphQL (Complete)
- ✅ **Phase 7:** Testing & DevOps (Complete - 71 tests passing)
- ✅ **Phase 8:** Testnet Infrastructure (Complete)
- ⏸️ **Phase 9:** Mainnet Launch (Q1 2026 Target - 2 months)

**Progress: 83% complete** (~9,715 lines production code)

**See [docs/implementation/LAYER1_IMPLEMENTATION_SUMMARY.md](docs/implementation/LAYER1_IMPLEMENTATION_SUMMARY.md) for complete implementation details and [LAYER1_FOCUS.md](LAYER1_FOCUS.md) for current priorities.**

This project includes an SDK toolkit and a command-line interface (CLI) for interacting with the network.

## 📋 Current Features

*   **Dynamic Subnets:** Create and deploy custom subnets with unique task logic and scoring mechanisms.
*   **zkML Integration:** Built-in support for Zero-Knowledge Machine Learning proofs using `ezkl`.
*   **Subnet Simulator:** Test your subnet logic locally with the `simulate` command.
*   **Wallet Management CLI (`mtcli w`):**
    *   Create Coldkey (`create-coldkey`): Generates a secure mnemonic phrase and encrypts it for storing the root key.
    *   Restore Coldkey (`restore-coldkey`): Recreates a coldkey from a saved mnemonic phrase.
    *   Generate Hotkey (`generate-hotkey`): Generates child keys (hotkeys) from the coldkey using standard HD derivation, used for Miner identification and signing operational transactions.
    *   Import Hotkey (`import-hotkey`): Imports an encrypted hotkey from an external source.
    *   Regenerate Hotkey (`regen-hotkey`): Recovers hotkey information if the `hotkeys.json` file is lost, requiring only the coldkey and the derivation index.
    *   List Wallets (`list`): Displays a list of coldkeys and their corresponding hotkeys.
    *   Register Hotkey (`register-hotkey`): Registers a hotkey as a Miner on the ModernTensor network, creating/updating a UTxO at the smart contract address with Miner information (UID, stake, API endpoint,...).

## 💡 Using the CLI (`mtcli`)

The main command-line tool is `mtcli`. The `w` (`wallet`) subcommand is used for wallet management, `tx` for transactions, and `query` for blockchain information.

**Help:**
```bash
mtcli --help
mtcli w --help
mtcli tx --help
mtcli query --help
mtcli w <command_name> --help # Example: mtcli w create-coldkey --help
mtcli query <command_name> --help # Example: mtcli query address --help
```

### Subnet & Simulation Commands

**Run a Validator:**
```bash
mtcli run_validator --subnet sdk.subnets.text_gen.TextGenerationSubnet --coldkey my_coldkey --hotkey my_hotkey --network testnet
```

**Run a Miner:**
```bash
mtcli run_miner --subnet sdk.subnets.text_gen.TextGenerationSubnet --coldkey my_coldkey --uid <UID> --network testnet
```

**Simulate a Subnet Locally:**
```bash
mtcli simulate --subnet sdk.subnets.text_gen.TextGenerationSubnet --miners 3 --steps 5
```

### Wallet Commands (`mtcli w`)

Manage Coldkeys & Hotkeys.

**Examples:**

```bash
# 1. Create a new coldkey named 'my_coldkey' in the './wallets' directory
#    - You will be prompted for a password to encrypt the mnemonic.
#    - !! SAVE THE DISPLAYED MNEMONIC PHRASE SECURELY !!
mtcli w create-coldkey --name my_coldkey --base-dir ./wallets

# 2. Restore a coldkey named 'restored_key' from its mnemonic phrase
#    - You will be prompted for the mnemonic phrase (12-24 words).
#    - You will be prompted to set a NEW password for the restored key.
mtcli w restore-coldkey --name restored_key --base-dir ./wallets

# 3. Generate a new hotkey named 'miner_hk1' derived from 'my_coldkey'
#    - You will be prompted for the password of 'my_coldkey'.
#    - Note the 'derivation_index' shown, needed for 'regen-hotkey'.
mtcli w generate-hotkey --coldkey my_coldkey --hotkey-name miner_hk1 --base-dir ./wallets

# 4. Import an exported encrypted hotkey string for 'my_coldkey'
#    - Replace "BASE64..." with the actual exported string.
mtcli w import-hotkey --coldkey my_coldkey --hotkey-name imported_hk \
    --encrypted-hotkey "BASE64_ENCRYPTED_STRING_HERE" \
    --base-dir ./wallets

# 5. Regenerate hotkey 'miner_hk1' using its derivation index (e.g., 0)
#    - Useful if hotkeys.json is lost but you have the coldkey mnemonic/password and index.
#    - You will be prompted for the password of 'my_coldkey'.
mtcli w regen-hotkey --coldkey my_coldkey --hotkey-name miner_hk1 --index 0 --base-dir ./wallets

# 6. List all coldkey names found in the './wallets' directory
mtcli w list --base-dir ./wallets

# 7. Register hotkey 'miner_hk1' as a miner for subnet 1 on testnet
#    - Sends a transaction to register on the network.
#    - Requires initial stake (in native tokens) and an API endpoint.
#    - You will be prompted for the password of 'my_coldkey'.
#    - Use '--yes' to skip the final confirmation.
mtcli w register-hotkey --coldkey my_coldkey --hotkey miner_hk1 \
    --subnet-uid 1 \
    --initial-stake 10000000 \
    --api-endpoint "http://123.45.67.89:8080" \
    --base-dir ./wallets \
    --network testnet \
    --yes

# 8. Show locally stored information for 'miner_hk1' (address, index, etc.)
#    - Reads from the local hotkeys.json file, no password needed.
mtcli w show-hotkey --coldkey my_coldkey --hotkey miner_hk1 --base-dir ./wallets

# 9. List all hotkey names associated with 'my_coldkey'
#    - Reads from the local hotkeys.json file.
mtcli w list-hotkeys --coldkey my_coldkey --base-dir ./wallets

# 10. Query balance and UTxOs of the *coldkey's main address* on testnet
#     - This address is derived directly from the mnemonic, often used for funding.
#     - You will be prompted for the password of 'my_coldkey'.
mtcli w query-address --coldkey my_coldkey --base-dir ./wallets --network testnet

# 11. Show the payment and stake addresses derived from 'my_coldkey' / 'miner_hk1' pair
#     - You will be prompted for the password of 'my_coldkey'.
mtcli w show-address --coldkey my_coldkey --hotkey miner_hk1 --base-dir ./wallets --network testnet
```

### Transaction Commands (`mtcli tx`)

Create and send transactions.

**Examples:**

```bash
# 1. Send tokens from 'miner_hk1' to a recipient address on testnet
#    - You will be prompted for the password of 'my_coldkey'.
mtcli tx send --coldkey my_coldkey --hotkey miner_hk1 \
    --to recipient_address... \
    --amount 5000000 \
    --base-dir ./wallets \
    --network testnet

# 2. Send tokens to another wallet
#    - You will be prompted for the password of 'my_coldkey'.
mtcli tx send --coldkey my_coldkey --hotkey miner_hk1 \
    --to other_coldkey/other_hk \
    --amount 100 \
    --base-dir ./wallets \
    --network testnet
```

### Query Commands (`mtcli query`)

Query blockchain information.

**Examples:**

```bash
# 1. Get detailed info for any address on testnet
mtcli query address address_here... --network testnet

# 2. Get the balance (ADA, tokens) for the 'miner_hk1' hotkey on testnet
#    - You will be prompted for the password of 'my_coldkey'.
mtcli query balance --coldkey my_coldkey --hotkey miner_hk1 --base-dir ./wallets --network testnet

# 3. List the UTxOs held by the 'miner_hk1' hotkey address on testnet
#    - You will be prompted for the password of 'my_coldkey'.
mtcli query utxos --coldkey my_coldkey --hotkey miner_hk1 --base-dir ./wallets --network testnet

# 4. Find a UTxO at a smart contract address containing a specific miner UID (hex) in its datum
mtcli query contract-utxo --contract-address addr_test1...validator_address... \
    --uid HEX_UID_STRING \
    --network testnet

# 5. Find the UTxO with the lowest performance score at a smart contract address
#    - Assumes MinerDatum format with a 'performance_score' field.
mtcli query lowest-performance --contract-address addr_test1...validator_address... \
    --network testnet

# 6. Query detailed static and dynamic information for Subnet UID 1 on testnet
mtcli query subnet --subnet-uid 1 --network testnet

# 7. List the UIDs of all registered subnets found on testnet
mtcli query list-subnets --network testnet
```

### Staking Commands (`mtcli stake`)

Manage staking operations for validator participation in the ModernTensor network.

**Examples:**

```bash
# 1. Stake tokens to become a validator or increase validator stake
#    - You will be prompted for the password of 'my_coldkey'.
mtcli stake add --coldkey my_coldkey --hotkey validator_hk \
    --amount 1000000 \
    --base-dir ./wallets \
    --network testnet

# 2. Withdraw staking rewards
#    - You will be prompted for the password of 'my_coldkey'.
mtcli stake withdraw --coldkey my_coldkey --hotkey validator_hk \
    --base-dir ./wallets \
    --network testnet

# 3. Show current staking info and rewards
#    - You will be prompted for the password of 'my_coldkey'.
mtcli stake info --coldkey my_coldkey --hotkey validator_hk \
    --base-dir ./wallets \
    --network testnet
```

## 🏗️ Architecture

*   `sdk/`: Core toolkit (Python SDK)
    *   `blockchain/`: Core blockchain primitives (Block, Transaction, State, Validation)
    *   `consensus/`: Proof of Stake consensus mechanism, validator management
    *   `network/`: P2P networking, synchronization
    *   `storage/`: Blockchain database and indexing
    *   `api/`: JSON-RPC and GraphQL APIs
    *   `keymanager/`: Coldkey/hotkey management, encryption, derivation
    *   `cli/`: Command-line interface (`mtcli`)
    *   `testnet/`: Testnet infrastructure (Genesis, Faucet, Bootstrap)
    *   `tokenomics/`: Token emission, rewards, burning
    *   `security/`: Security auditing and validation
    *   `optimization/`: Performance optimizations
    *   `monitoring/`: Metrics collection
*   `docs/`: Comprehensive documentation
    *   `implementation/`: Implementation details and phase summaries
    *   `architecture/`: System design and diagrams
    *   `reports/`: Audit reports and verification results
*   `tests/`: Test suite
*   `examples/`: Example scripts and demos

## ⚙️ Installation

1.  **Requirements:**
    *   Python 3.9+
    *   pip

2.  **Clone Repository:**
    ```bash
    git clone <your_repository_url>
    cd moderntensor
    ```

3.  **Create Virtual Environment (Recommended):**
    ```bash
    python -m venv venv
    source venv/bin/activate  # On Linux/macOS
    # venv\Scripts\activate   # On Windows
    ```

4.  **Install Dependencies:**
    ```bash
    pip install -r requirements.txt
    ```
    *(Note: Ensure you have a complete `requirements.txt` file with libraries like `click`, `rich`, `pycardano`, `blockfrost-python`, `cbor2`, `cryptography`, etc...)*

5.  **(Optional) Install in Editable Mode:** If you want the `mtcli` CLI to be runnable from anywhere and reflect code changes immediately. Requires a suitable `setup.py` or `pyproject.toml` file.
    ```bash
    pip install -e .
    ```

## 🤝 Contributing

We welcome contributions from the community! Please refer to `CONTRIBUTING.md` (if available) or follow standard procedures:

1.  Fork the repository.
2.  Create a new branch (`git checkout -b feature/AmazingFeature`).
3.  Commit your changes (`git commit -m 'Add some AmazingFeature'`).
4.  Push to the branch (`git push origin feature/AmazingFeature`).
5.  Open a Pull Request.

## 📚 Documentation

Comprehensive documentation is available in the [`docs/`](docs/) directory:

- **[Implementation Docs](docs/implementation/)** - Phase summaries, implementation details
- **[Architecture Docs](docs/architecture/)** - System design, diagrams, technical specifications
- **[Reports](docs/reports/)** - Audit reports, verification results (Vietnamese)

Key documents:
- [Layer 1 Roadmap](LAYER1_ROADMAP.md) - Development roadmap and milestones
- [Layer 1 Focus](LAYER1_FOCUS.md) - Current development priorities
- [Migration Guide](MIGRATION.md) - Transitioning to Layer 1
- [Changelog](CHANGELOG.md) - Version history

## 📄 License

This project is licensed under the MIT License - see the `LICENSE` file (if available) for details. (Or change to your chosen license, e.g., Apache 2.0)

## 📞 Contact

(Optional: Add contact information, Discord links, Twitter, etc.)
