# ModernTensor SDK Documentation

This directory contains the API documentation for the ModernTensor SDK.

## Building the Documentation

### Prerequisites

```bash
pip install sphinx sphinx-rtd-theme sphinx-autodoc-typehints
```

### Build HTML Documentation

```bash
cd docs/api
sphinx-build -b html . _build/html
```

The documentation will be generated in `_build/html/`. Open `_build/html/index.html` in your browser.

### Build PDF Documentation (requires LaTeX)

```bash
cd docs/api
sphinx-build -b latex . _build/latex
cd _build/latex
make
```

## Documentation Structure

- `index.rst` - Main documentation index
- `getting_started.rst` - Getting started guide
- `api/` - API reference documentation
  - `transactions.rst` - Transaction system API
  - `axon.rst` - Axon server API
  - `dendrite.rst` - Dendrite client API
  - `metagraph.rst` - Metagraph API
- `guides/` - Usage guides and tutorials
  - `transaction_usage.rst` - Transaction usage guide
  - `subnet_development.rst` - Subnet development guide

## Contributing

When adding new modules or features:

1. Add docstrings to all classes and functions
2. Create corresponding `.rst` files in the appropriate directory
3. Update `index.rst` to include new documentation
4. Build and verify the documentation locally

## Style Guide

- Use Google-style docstrings
- Include examples in docstrings where appropriate
- Keep descriptions clear and concise
- Cross-reference related modules using Sphinx directives
