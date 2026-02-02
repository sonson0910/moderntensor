# Contributing to ModernTensor

Thank you for your interest in contributing to ModernTensor! 🎉

## 🚀 Quick Start

1. **Fork** the repository
2. **Clone** your fork locally
3. **Create** a feature branch
4. **Make** your changes
5. **Test** your changes
6. **Submit** a Pull Request

## 📋 Development Setup

### Prerequisites

- Rust 1.75+ (`rustup`)
- Python 3.10+ (`pyenv` recommended)
- Node.js 18+ (`nvm` recommended)
- Git

### Setup

```bash
# Clone
git clone https://github.com/YOUR_USERNAME/moderntensor.git
cd moderntensor

# Rust
cd luxtensor
cargo build

# Python SDK
cd ../sdk
pip install -e ".[dev]"

# Contracts
cd ../luxtensor/contracts
npm install
```

## 📝 Code Standards

### General

- Follow existing code style
- Write meaningful commit messages
- Add tests for new features
- Update documentation

### Rust

- `cargo fmt` before committing
- `cargo clippy` with no warnings
- Use `Result<T, E>` for error handling
- Document public APIs with `///`

### Python

- Type hints required
- `ruff` for linting
- `black` for formatting
- `pytest` for testing

### Solidity

- Follow OpenZeppelin patterns
- Comprehensive NatSpec comments
- Gas optimization where appropriate

## 🔀 Pull Request Process

1. **Title**: Clear, descriptive title (e.g., "Add zkML proof verification")
2. **Description**: Explain what and why
3. **Tests**: Include relevant tests
4. **Docs**: Update if needed
5. **Review**: Address feedback promptly

## 🐛 Reporting Bugs

- Search existing issues first
- Use the bug report template
- Include reproduction steps
- Attach relevant logs

## 💡 Feature Requests

- Check roadmap first
- Explain the use case
- Be specific about requirements

## 📜 License

By contributing, you agree that your contributions will be licensed under MIT.
