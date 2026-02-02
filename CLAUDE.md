# ModernTensor Project Guidelines

> AI agent configuration file. Follow these rules when working on this project.

## 📁 Project Structure

```
moderntensor/
├── luxtensor/          # L1 Blockchain (Rust)
│   ├── crates/         # Rust crates
│   ├── contracts/      # Solidity smart contracts
│   └── docs/           # Technical docs
├── sdk/                # Python AI SDK
├── indexer/            # Blockchain indexer (Rust)
├── book/               # mdbook documentation
└── tools/              # Faucet, explorer, etc.
```

## 🛠️ Commands

### Rust (LuxTensor)

```bash
cd luxtensor
cargo build --release    # Build
cargo test               # Unit tests
cargo clippy             # Lint
cargo fmt                # Format
```

### Python (SDK)

```bash
cd sdk
pip install -e .         # Install dev mode
pytest                   # Tests
ruff check .             # Lint
mypy .                   # Type check
```

### Solidity (Contracts)

```bash
cd luxtensor/contracts
npm install              # Install deps
npx hardhat compile      # Compile
npx hardhat test         # Tests
```

### Documentation

```bash
cd book
mdbook serve --open      # Live preview
mdbook build             # Build static
```

## 📝 Code Style

| Language | Formatter | Linter |
|----------|-----------|--------|
| Rust | `rustfmt` | `clippy` |
| Python | `black` | `ruff`, `mypy` |
| Solidity | `prettier` | `solhint` |
| Markdown | - | `markdownlint` |

## 🏗️ Architecture

- **Clean Code**: SRP, DRY, KISS, YAGNI
- **Rust**: Prefer `Result<T, E>` over panics
- **Python**: Type hints required
- **Solidity**: Follow OpenZeppelin patterns

## ⚠️ Before Editing

1. **Understand context**: Read related files first
2. **Check dependencies**: Who imports this file?
3. **Run tests**: Verify changes don't break existing code
4. **Update docs**: Keep documentation in sync

## 🔐 Security

- Never commit secrets or API keys
- Use `.env` for environment variables
- Validate all inputs
- Follow OWASP guidelines for contracts

## 🧪 Testing

- Unit tests required for new functions
- Integration tests for cross-module features
- Target: 80%+ coverage

## 📚 Key Files

| File | Purpose |
|------|---------|
| `luxtensor/Cargo.toml` | Workspace config |
| `sdk/pyproject.toml` | Python package config |
| `book/book.toml` | mdbook config |
| `.env.example` | Environment template |
