# Contributing to ModernTensor

Thank you for your interest in contributing to ModernTensor! This guide covers everything you need to get started.

## Table of Contents

- [Quick Start](#-quick-start)
- [Development Setup](#-development-setup)
- [Running Tests](#-running-tests)
- [Code Style Guidelines](#-code-style-guidelines)
- [Pull Request Process](#-pull-request-process)
- [Reporting Bugs](#-reporting-bugs)
- [Feature Requests](#-feature-requests)
- [License](#-license)

## 🚀 Quick Start

1. **Fork** the repository on GitHub
2. **Clone** your fork locally
3. **Create** a feature branch (`git checkout -b feature/my-feature`)
4. **Make** your changes
5. **Test** your changes
6. **Commit** with a meaningful message
7. **Push** and open a Pull Request

## 📋 Development Setup

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.75+ | Luxtensor blockchain node |
| Python | 3.10+ | SDK, CLI, tests |
| Git | 2.x+ | Version control |

Optional but recommended: `pyenv` for Python version management, `rustup` for Rust toolchain management.

### Clone the repository

```bash
git clone https://github.com/YOUR_USERNAME/moderntensor.git
cd moderntensor
```

### Setting up the Rust node (Luxtensor)

```bash
cd luxtensor

# Build the blockchain node
cargo build

# Run tests
cargo test --workspace

# Check for lint warnings
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all
```

### Setting up the Python SDK

```bash
# From the repository root
# Install in editable mode with dev dependencies
pip install -e ".[dev]"

# Verify installation
mtcli --version
python -c "from sdk import connect; print('SDK OK')"
```

### Running a local node (for integration testing)

```bash
cd luxtensor
cargo run --release
# Node will be available at http://localhost:8545
```

## 🧪 Running Tests

### Python SDK tests

```bash
# Run all tests
pytest tests/ -v

# Run with coverage
pytest tests/ --cov=sdk --cov-report=term-missing

# Run only unit tests (no running node required)
pytest tests/ -v -m "not integration"

# Run integration tests (requires a running Luxtensor node)
pytest tests/ -v -m integration
```

### Rust tests

```bash
cd luxtensor
cargo test --workspace
```

### Linting

```bash
# Python
ruff check sdk/ tests/
black --check sdk/ tests/

# Rust
cd luxtensor
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## 📝 Code Style Guidelines

### General rules

- Write meaningful commit messages (see [Conventional Commits](https://www.conventionalcommits.org/))
- Add tests for new features and bug fixes
- Update documentation when changing public APIs
- Keep PRs focused — one feature or fix per PR

### Python

- **Formatter:** `black` (line-length 100)
- **Linter:** `ruff`
- **Type hints** are required for all public functions
- **Docstrings** for all public classes and functions (Google style)
- **Tests** with `pytest`

```python
def get_balance(self, address: str) -> int:
    """Get the balance of an account.

    Args:
        address: The account address (hex string with 0x prefix).

    Returns:
        The balance in the smallest token unit.

    Raises:
        ConnectionError: If the node is unreachable.
    """
```

### Rust

- Run `cargo fmt` before every commit
- `cargo clippy` must produce zero warnings
- Use `Result<T, E>` for fallible operations
- Document public APIs with `///` doc comments

### Commit messages

Use the format: `type(scope): description`

```
feat(sdk): add batch transaction submission
fix(consensus): correct slashing penalty calculation
docs(readme): update quick start example
test(cli): add wallet creation tests
chore(ci): upgrade Python matrix to 3.12
```

## 🔀 Pull Request Process

1. **Create an issue first** (unless it's a trivial fix)
2. **Branch from `main`**: `git checkout -b feature/short-description`
3. **Make focused changes** — one logical change per PR
4. **Write/update tests** — PRs without tests for new features will be requested to add them
5. **Run checks locally** before pushing:
   ```bash
   ruff check sdk/ tests/
   black --check sdk/ tests/
   pytest tests/ -v
   ```
6. **Push and open a PR** using the [pull request template](/.github/PULL_REQUEST_TEMPLATE.md)
7. **Address review feedback** promptly
8. **Squash or rebase** if requested by maintainers

### What we look for in reviews

- Correctness and security
- Test coverage
- Documentation updates
- Code clarity and maintainability
- No unnecessary dependencies

## 🐛 Reporting Bugs

Please use the [bug report template](https://github.com/sonson0910/moderntensor/issues/new?template=bug_report.md) and include:

- Clear reproduction steps
- Expected vs. actual behavior
- Environment details (OS, Python version, SDK version)
- Relevant logs or tracebacks

## 💡 Feature Requests

Please use the [feature request template](https://github.com/sonson0910/moderntensor/issues/new?template=feature_request.md) and include:

- A clear description of the proposed feature
- Motivation and use cases
- Suggested API design or code examples (if applicable)
- Which component it affects (SDK, blockchain, CLI, etc.)

## 📜 License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
