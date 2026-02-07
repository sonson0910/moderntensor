# ModernTensor Miner Guide

## Overview

Miners are AI model providers in the ModernTensor network. They contribute AI capabilities and earn rewards based on model quality and performance. This guide explains how to become a successful miner.

---

## Why Become a Miner?

| Benefit | Description |
|---------|-------------|
| **Earn Rewards** | Get paid for AI model performance |
| **Monetize Models** | Turn your AI research into income |
| **No Entry Barrier** | Start with 0 tokens |
| **Flexible** | Provide any AI capability |

---

## How Mining Works

```
┌─────────────────────────────────────────────────────────────┐
│                      MINING FLOW                             │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  1. REGISTER          2. RECEIVE           3. RESPOND         │
│  └─────────────────►  └─────────────────►  └─────────────►   │
│  Register model       Get inference        Return output      │
│  on a subnet          requests             to validator       │
│                                                               │
│  4. SCORE             5. REWARD                              │
│  └─────────────────►  └─────────────────►                    │
│  Validators rate      Earn tokens based                       │
│  your output          on quality score                        │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

---

## Getting Started

### Step 1: Install SDK

```bash
pip install moderntensor
```

### Step 2: Create Wallet

```python
from moderntensor import Wallet

# Create new wallet
wallet = Wallet.create()
print(f"Address: {wallet.address}")
print(f"Save your mnemonic securely!")

# Or import existing
wallet = Wallet.from_mnemonic("your 24-word mnemonic")
```

### Step 3: Register on Subnet

```python
from moderntensor import LuxtensorClient

client = LuxtensorClient("http://localhost:8545")

# Register as miner on subnet 1 (LLM inference)
tx = client.register_miner(
    subnet_id=1,
    model_info={
        "name": "my-llm-model",
        "type": "text-generation",
        "version": "1.0.0"
    }
)
print(f"Registration TX: {tx}")
```

### Step 4: Set Up Inference Server

```python
from moderntensor import MinerServer
import your_model

class MyMiner(MinerServer):
    def __init__(self):
        super().__init__()
        self.model = your_model.load()

    def inference(self, request):
        """Handle inference request from network."""
        input_data = request.input
        output = self.model.generate(input_data)
        return {
            "output": output,
            "latency_ms": request.elapsed_ms
        }

# Start server
server = MyMiner()
server.run(port=5000)
```

---

## Subnets

Different subnets specialize in different AI tasks:

| Subnet ID | Task Type | Example Models |
|-----------|-----------|----------------|
| 1 | Text Generation | GPT, Llama, Claude |
| 2 | Image Generation | Stable Diffusion, DALL-E |
| 3 | Code Generation | CodeLlama, StarCoder |
| 4 | Audio/Speech | Whisper, TTS models |
| 5+ | Custom | Your specialized model |

### Choosing a Subnet
- **Match your expertise** — Pick subnets where you can excel
- **Check competition** — Fewer miners = easier to earn
- **Consider hardware** — Different models need different resources

---

## Quality Scoring

Your rewards depend on quality scores from validators:

### Scoring Criteria

| Criterion | Weight | How Measured |
|-----------|--------|--------------|
| **Accuracy** | 40% | Output correctness |
| **Latency** | 20% | Response time |
| **Availability** | 20% | Uptime percentage |
| **Consistency** | 20% | Same input = same output |

### Score Formula

```
MinerScore = (Accuracy × 0.4) + (Latency × 0.2) + (Availability × 0.2) + (Consistency × 0.2)
```

### Improving Your Score

1. **Optimize inference speed** — Use GPU, quantization
2. **Maintain uptime** — 99.9%+ availability
3. **Ensure quality** — Test thoroughly before deploying
4. **Be consistent** — Deterministic outputs when possible

---

## Rewards

### How Rewards Work

```
Epoch (15 minutes)
├── All miners scored by validators
├── Rewards distributed proportionally
└── Higher score = Higher reward share
```

### Reward Formula

```
MinerReward = EpochEmission × (YourScore / TotalScores) × SubnetWeight
```

### Example Calculation

```python
epoch_emission = 1000  # MDT emitted this epoch
your_score = 0.85
total_scores = 10.0
subnet_weight = 0.15  # 15% of network rewards

reward = 1000 * (0.85 / 10.0) * 0.15
# reward = 12.75 MDT per epoch
```

---

## Hardware Recommendations

### For LLM Models (Subnet 1)

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| GPU | RTX 3090 (24GB) | A100 (80GB) |
| CPU | 8 cores | 16+ cores |
| RAM | 32 GB | 64+ GB |
| Storage | 500 GB SSD | 1+ TB NVMe |

### For Image Models (Subnet 2)

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| GPU | RTX 3060 (12GB) | RTX 4090 (24GB) |
| CPU | 4 cores | 8+ cores |
| RAM | 16 GB | 32+ GB |
| Storage | 100 GB SSD | 500 GB SSD |

---

## Best Practices

### Performance
- [ ] Use GPU acceleration
- [ ] Implement batching for throughput
- [ ] Cache common requests
- [ ] Optimize model quantization

### Reliability
- [ ] Monitor server health
- [ ] Set up auto-restart on failure
- [ ] Use redundant infrastructure
- [ ] Regular model updates

### Security
- [ ] Secure API endpoints
- [ ] Validate all inputs
- [ ] Rate limit requests
- [ ] Keep software updated

---

## Troubleshooting

### Common Issues

**Not receiving requests:**
- Check subnet registration status
- Verify server is reachable
- Confirm correct port configuration

**Low quality scores:**
- Review validator feedback
- Test model accuracy locally
- Check for latency issues

**Rewards not appearing:**
- Confirm epoch has completed
- Check score threshold met
- Verify wallet address

---

## Support

- **Documentation:** /docs in repository
- **GitHub Issues:** Technical support
- **Discord:** Community help (coming soon)

---

*Start earning from your AI models today!*
