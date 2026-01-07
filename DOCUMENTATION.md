# ModernTensor Documentation Index

**Last Updated:** January 7, 2026

This is the main documentation index for ModernTensor. All essential documentation is organized here.

---

## 📚 Core Documentation

### Getting Started
- **[README.md](README.md)** - Project overview, quick start, installation
- **[MODERNTENSOR_WHITEPAPER_VI.md](MODERNTENSOR_WHITEPAPER_VI.md)** - Technical whitepaper (Vietnamese)

### AI/ML Layer
- **[AI_ML_IMPLEMENTATION_GUIDE.md](AI_ML_IMPLEMENTATION_GUIDE.md)** - Complete usage guide for AI/ML layer
- **[COMPLETE_AI_ML_IMPLEMENTATION.md](COMPLETE_AI_ML_IMPLEMENTATION.md)** - Final implementation report
- **[AI_ML_IMPROVEMENTS_SUMMARY_VI.md](AI_ML_IMPROVEMENTS_SUMMARY_VI.md)** - Comparison with Bittensor (Vietnamese)
- **[TOM_TAT_AI_ML_IMPLEMENTATION_VI.md](TOM_TAT_AI_ML_IMPLEMENTATION_VI.md)** - Summary (Vietnamese)

### Layer 1 Blockchain
- **[LAYER1_ROADMAP.md](LAYER1_ROADMAP.md)** - Layer 1 blockchain roadmap
- **[LAYER1_FOCUS.md](LAYER1_FOCUS.md)** - Current focus and priorities

### LuxTensor Integration
- **[LUXTENSOR_USAGE_GUIDE.md](LUXTENSOR_USAGE_GUIDE.md)** - How to use LuxTensor
- **[LUXTENSOR_TECHNICAL_FAQ_VI.md](LUXTENSOR_TECHNICAL_FAQ_VI.md)** - Technical FAQ (Vietnamese)

### Project Management
- **[CHANGELOG.md](CHANGELOG.md)** - Version history and changes

---

## 🏗️ Architecture Overview

```
ModernTensor/
├── sdk/                     # Core SDK
│   ├── ai_ml/              # AI/ML Layer (NEW!)
│   │   ├── core/           # Protocol definitions
│   │   ├── subnets/        # Subnet implementations
│   │   ├── models/         # Model management
│   │   ├── processors/     # Batch/parallel processing
│   │   ├── zkml/           # Zero-knowledge ML
│   │   ├── scoring/        # Advanced scoring
│   │   └── agent/          # AI agents
│   ├── blockchain/         # Layer 1 blockchain
│   ├── consensus/          # Consensus mechanisms
│   ├── network/            # P2P networking
│   └── ...
├── examples/               # Usage examples
├── tests/                  # Test suite
└── docs/                   # Additional documentation
```

---

## 🚀 Quick Links

### For Developers
- [AI/ML Implementation Guide](AI_ML_IMPLEMENTATION_GUIDE.md) - How to build AI/ML subnets
- [Examples Directory](examples/) - Code examples
- [Tests Directory](tests/) - Test examples

### For Validators
- [Layer 1 Roadmap](LAYER1_ROADMAP.md) - Roadmap and milestones
- [LuxTensor Guide](LUXTENSOR_USAGE_GUIDE.md) - Validator setup

### For Miners
- [AI/ML Guide](AI_ML_IMPLEMENTATION_GUIDE.md) - How to create mining subnets
- [Complete Implementation](COMPLETE_AI_ML_IMPLEMENTATION.md) - Technical details

---

## 📖 Documentation by Topic

### AI/ML Features
ModernTensor's AI/ML layer surpasses Bittensor with:
- ✅ **Model Management** - Versioning, tracking, caching
- ✅ **Batch Processing** - 5x throughput improvement
- ✅ **Parallel Processing** - 8x throughput improvement  
- ✅ **zkML Proofs** - Zero-knowledge ML (unique to ModernTensor)
- ✅ **Multi-Criteria Scoring** - 6 scoring methods
- ✅ **Robust Consensus** - 6 consensus methods with outlier detection
- ✅ **Production LLM** - HuggingFace Transformers integration
- ✅ **Reward Models** - ML-based quality scoring

See [AI/ML Implementation Guide](AI_ML_IMPLEMENTATION_GUIDE.md) for details.

### Blockchain Features
ModernTensor's custom Layer 1 blockchain:
- ✅ **PoS Consensus** - Proof of Stake with validator sets
- ✅ **Account Model** - ETH-style account-based state
- ✅ **Smart Contracts** - Native contract support
- ✅ **P2P Network** - Kademlia DHT-based networking
- ✅ **Storage Layer** - LevelDB with state trie
- ✅ **RPC API** - JSON-RPC 2.0 interface

See [Layer 1 Roadmap](LAYER1_ROADMAP.md) for details.

---

## 🔧 Development

### Running Examples
```bash
# AI/ML batch processing demo
PYTHONPATH=. python3 examples/advanced_ai_ml_example.py

# Complete AI/ML demo (all phases)
PYTHONPATH=. python3 examples/complete_ai_ml_demo.py
```

### Running Tests
```bash
# Run AI/ML tests
python3 -m pytest tests/ai_ml/ -v

# Run all tests
python3 -m pytest tests/ -v
```

---

## 📝 Contributing

See [README.md](README.md) for contribution guidelines.

---

## 📄 License

MIT License - See LICENSE file for details.

---

## 📞 Support

- GitHub Issues: https://github.com/sonson0910/moderntensor/issues
- Documentation: This index and linked files

---

**Note:** This documentation index replaces all previous completion/summary documents. Only the files listed above are maintained going forward.
