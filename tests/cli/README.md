# CLI Testing Guide

This document provides instructions for running and understanding the test suite for the ModernTensor CLI (mtcli).

## Overview

The CLI test suite consists of:
- **94 Unit Tests**: Testing individual command functionality
- **25 Integration Tests**: Testing workflows and component integration
- **Total: 119 Tests** - All passing ✅

## Test Structure

```
tests/cli/
├── __init__.py
├── unit/                          # Unit tests for CLI commands
│   ├── test_utils_commands.py     # 10 tests - utility commands
│   ├── test_wallet_commands.py    # 16 tests - wallet management
│   ├── test_query_commands.py     # 14 tests - blockchain queries
│   ├── test_tx_commands.py        # 14 tests - transactions
│   ├── test_stake_commands.py     # 15 tests - staking operations
│   ├── test_subnet_commands.py    # 13 tests - subnet management
│   └── test_validator_commands.py # 12 tests - validator operations
└── integration/                   # Integration tests
    ├── test_config_integration.py # 11 tests - configuration management
    └── test_key_management.py     # 14 tests - key generation & encryption
```

## Running Tests

### Prerequisites

Install dependencies:
```bash
pip install -r requirements.txt
```

### Run All Tests

```bash
# Run all CLI tests
pytest tests/cli/ -v

# Run with coverage
pytest tests/cli/ --cov=sdk.cli --cov-report=html
```

### Run Specific Test Suites

```bash
# Run only unit tests
pytest tests/cli/unit/ -v

# Run only integration tests
pytest tests/cli/integration/ -v

# Run specific command tests
pytest tests/cli/unit/test_wallet_commands.py -v
pytest tests/cli/unit/test_query_commands.py -v
```

### Run Specific Tests

```bash
# Run a specific test class
pytest tests/cli/unit/test_wallet_commands.py::TestWalletCommands -v

# Run a specific test method
pytest tests/cli/unit/test_wallet_commands.py::TestWalletCommands::test_wallet_help -v
```

## Test Categories

### 1. Unit Tests

Unit tests verify individual command functionality:

- **Command Validation**: Tests that commands reject invalid input
- **Parameter Requirements**: Tests for required vs optional parameters
- **Help Messages**: Tests that help text is displayed correctly
- **Error Handling**: Tests graceful failure modes

Example:
```python
def test_create_coldkey_no_name(self):
    """Test creating coldkey without name fails."""
    result = self.runner.invoke(cli, ['wallet', 'create-coldkey'])
    assert result.exit_code != 0
```

### 2. Integration Tests

Integration tests verify complete workflows:

- **Key Management**: Mnemonic generation, key derivation, encryption
- **Configuration**: Loading, saving, network configurations
- **End-to-End Workflows**: Complete user scenarios

Example:
```python
def test_complete_coldkey_workflow(self):
    """Test complete coldkey creation and storage workflow."""
    # 1. Generate mnemonic
    # 2. Encrypt mnemonic
    # 3. Save to file
    # 4. Load from file
    # 5. Verify
```

## Test Coverage

### Command Coverage

| Command Group | Commands | Unit Tests | Integration Tests |
|--------------|----------|------------|-------------------|
| wallet       | 11       | 16         | 3 workflows       |
| query        | 6        | 14         | -                 |
| tx           | 3        | 14         | -                 |
| stake        | 5        | 15         | -                 |
| subnet       | 4        | 13         | -                 |
| validator    | 4        | 12         | -                 |
| utils        | 3        | 10         | -                 |
| **Total**    | **36**   | **94**     | **25**            |

### Key Management Coverage

- ✅ Mnemonic generation (12 and 24 words)
- ✅ Key derivation (BIP39/BIP44)
- ✅ Multiple hotkeys from single coldkey
- ✅ Deterministic derivation
- ✅ Encryption/decryption (PBKDF2 + Fernet)
- ✅ Encrypted file storage

### Configuration Coverage

- ✅ Default configuration creation
- ✅ Configuration serialization (to_dict/from_dict)
- ✅ Network configuration (mainnet/testnet/local)
- ✅ YAML file operations
- ✅ CLI config option handling

## Writing New Tests

### Unit Test Template

```python
def test_command_name(self):
    """Test description."""
    result = self.runner.invoke(cli, ['command', 'subcommand', '--option', 'value'])
    assert result.exit_code == 0
    assert 'expected output' in result.output
```

### Integration Test Template

```python
def test_workflow_name(self):
    """Test complete workflow description."""
    # Setup
    setup_data = create_test_data()
    
    # Execute
    result = perform_operation(setup_data)
    
    # Verify
    assert result.success
    assert result.output == expected_output
    
    # Cleanup (in teardown_method)
```

## Continuous Integration

Tests should be run:
- Before committing changes
- In CI/CD pipeline
- Before releases

### CI Configuration Example

```yaml
# .github/workflows/test.yml
name: Test CLI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions/setup-python@v2
        with:
          python-version: '3.8'
      - run: pip install -r requirements.txt
      - run: pytest tests/cli/ -v
```

## Test Best Practices

1. **Isolation**: Each test should be independent
2. **Cleanup**: Use setup/teardown methods for temporary files
3. **Clear Names**: Test names should describe what they test
4. **Single Assertion**: Each test should verify one thing
5. **Mock External Dependencies**: Don't rely on network availability

## Debugging Tests

### Run with verbose output
```bash
pytest tests/cli/ -vv
```

### Run with print statements
```bash
pytest tests/cli/ -s
```

### Stop on first failure
```bash
pytest tests/cli/ -x
```

### Run last failed tests
```bash
pytest tests/cli/ --lf
```

### Debug with pdb
```python
def test_something(self):
    import pdb; pdb.set_trace()
    # Test code here
```

## Common Issues

### Issue: Import errors
**Solution**: Make sure you're running pytest from the repository root

### Issue: Temporary files not cleaned up
**Solution**: Use `teardown_method` to clean up in tests

### Issue: Tests pass locally but fail in CI
**Solution**: Check for environment-specific assumptions (file paths, network access)

## Performance

Current test suite performance:
- Unit tests: ~1.0 seconds
- Integration tests: ~0.8 seconds
- **Total: ~1.8 seconds** ⚡

## Future Improvements

- [ ] Add code coverage reporting
- [ ] Add performance benchmarks
- [ ] Add end-to-end tests with testnet
- [ ] Add security vulnerability scanning
- [ ] Add mutation testing

## Contributing

When adding new CLI commands:
1. Add corresponding unit tests
2. Add integration tests if needed
3. Ensure all existing tests still pass
4. Update this documentation

## Resources

- [pytest documentation](https://docs.pytest.org/)
- [Click testing utilities](https://click.palletsprojects.com/en/8.1.x/testing/)
- [ModernTensor CLI documentation](../MTCLI_IMPLEMENTATION_GUIDE.md)
