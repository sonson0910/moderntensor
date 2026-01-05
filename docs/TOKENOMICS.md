# ModernTensor Tokenomics ($MDT)

## 1. Core Philosophy: Adaptive & Sustainable Emission

Khác với Bittensor (in tiền cố định bất kể nhu cầu), ModernTensor áp dụng cơ chế **"Phát thải thích ứng" (Adaptive Emission)**. Mục tiêu là giảm tối đa lạm phát khi mạng lưới chưa tạo ra giá trị thực, và chỉ tăng cung khi nhu cầu sử dụng mạng lưới tăng.

## 2. Key Metrics
*   **Token Name:** ModernTensor ($MDT)
*   **Max Supply:** 21,000,000 MDT
*   **Halving Interval:** 210,000 epochs (~4 years at 10min/epoch)
*   **Emission Type:** Deflationary (Giảm phát)
*   **Base Reward:** 1,000 tokens per epoch

## 3. Implementation Status ✅

**Status:** IMPLEMENTED & TESTED

ModernTensor's adaptive tokenomics is now fully implemented and integrated with Layer 1 blockchain:

- ✅ **Adaptive Emission Controller** (`sdk/tokenomics/emission_controller.py`)
- ✅ **Token Recycling Pool** (`sdk/tokenomics/recycling_pool.py`)
- ✅ **Burn Manager** (`sdk/tokenomics/burn_manager.py`)
- ✅ **Reward Distributor** (`sdk/tokenomics/reward_distributor.py`)
- ✅ **Merkle Claim System** (`sdk/tokenomics/claim_manager.py`)
- ✅ **Layer 1 Integration** (`sdk/consensus/layer1_integration.py`)
- ✅ **37 Unit Tests Passing** (`tests/tokenomics/`)
- ✅ **Working Demo** (`examples/tokenomics_demo.py`)

## 4. Cơ chế Giảm Phác Thải (Emission Reduction Mechanisms)

### A. Utility-Based Minting (In tiền dựa trên Tiện ích)
Thay vì in cố định `1 MDT` mỗi block, lượng in ra (`MintAmount`) phụ thuộc vào **Điểm Tiện ích Mạng lưới (Network Utility Score - $U$)**:

$$ MintAmount = BaseReward \times U $$

Trong đó $U$ (từ 0.0 đến 1.0) được tính dựa trên:
1.  **Task Volume:** Số lượng task AI được giải quyết trong chu kỳ.
2.  **Task Difficulty:** Độ khó trung bình của các task (zkML proof complexity).
3.  **Validator Participation:** Tỷ lệ Validator tham gia đồng thuận.

*   **Hệ quả:** Nếu mạng lưới "vắng khách" (ít task), $U$ thấp $\rightarrow$ Lượng token in ra cực thấp. Tránh lạm phát vô nghĩa.

### B. The Recycling Pool (Bể Tái Chế)
Ưu tiên sử dụng token đã có sẵn thay vì in mới.
1.  **Nguồn thu:**
    *   Phí đăng ký Miner (Registration Fee).
    *   Phí phạt Validator (Slashing).
    *   Phí người dùng gửi Task (nếu có).
2.  **Cơ chế:**
    *   Khi đến kỳ trả thưởng, Smart Contract kiểm tra **Recycling Pool**.
    *   Nếu Pool có tiền $\rightarrow$ Lấy tiền từ Pool trả cho Miner/Validator.
    *   Nếu Pool thiếu $\rightarrow$ Mới kích hoạt Minting để bù phần thiếu.

### C. Burn Mechanism (Cơ chế Đốt)
*   **Unmet Quota Burn:** Nếu trong một chu kỳ, chất lượng mạng lưới kém (không đạt chuẩn đồng thuận), phần thưởng dự kiến của chu kỳ đó sẽ bị **ĐỐT BỎ** thay vì tích lũy.
*   **Transaction Fee Burn:** 50% phí giao dịch trên mạng lưới (nếu có Layer 2 fees) sẽ bị đốt.

## 4. Phân Phối (Distribution)
Mỗi đợt trả thưởng (sau khi đã tính toán giảm phát) được chia như sau:

*   **40% - Miners:** Dựa trên Proof of Useful Work (zkML).
*   **40% - Validators:** Dựa trên Consensus Weight & Staking.
*   **20% - DAO Treasury:** Dùng cho R&D và vận hành hạ tầng (Oracle/Hydra).

## 5. Technical Implementation

### Quick Start

```python
from sdk.tokenomics import TokenomicsIntegration, ConsensusData, NetworkMetrics

# Initialize
tokenomics = TokenomicsIntegration()

# Process epoch
result = tokenomics.process_epoch_tokenomics(
    epoch=0,
    consensus_data=ConsensusData(
        miner_scores={'miner1': 0.8},
        validator_stakes={'val1': 100000},
        quality_score=0.9
    ),
    network_metrics=NetworkMetrics(
        task_count=5000,
        avg_difficulty=0.8,
        validator_ratio=1.0
    )
)

print(f"Emission: {result.emission_amount} tokens")
print(f"Utility: {result.utility_score}")
```

### Layer 1 Integration

Tokenomics is automatically processed during consensus:

```python
from sdk.consensus.layer1_integration import Layer1ConsensusIntegrator

integrator = Layer1ConsensusIntegrator()

# Process consensus (includes tokenomics)
await integrator.process_consensus_round(...)

# Access tokenomics data from aggregated state
state = integrator.state_manager.get_state(subnet_uid)
print(f"Utility: {state.utility_score}")
print(f"Emission: {state.epoch_emission}")
print(f"Pool Balance: {state.recycling_pool_balance}")
```

### Running Tests

```bash
# Run all tokenomics tests
pytest tests/tokenomics/ -v

# Run demo
PYTHONPATH=. python examples/tokenomics_demo.py
```

### Documentation

For detailed documentation, see:
- **Full Guide:** `docs/TOKENOMICS_DETAILED.md` (to be created)
- **Implementation Plan:** `TOKENOMICS_IMPLEMENTATION_PLAN.md`
- **API Reference:** Module docstrings in `sdk/tokenomics/`

## 6. Comparison with Bittensor

| Feature | Bittensor | ModernTensor |
|---------|-----------|--------------|
| Emission | Fixed | ✅ Adaptive |
| Recycling | ❌ No | ✅ Yes |
| Burns | ❌ No | ✅ Yes |
| DAO Treasury | Limited | ✅ 20% |
| Claim System | On-chain | ✅ Merkle proofs |
| Supply Cap | ~21M TAO | 21M MDT |
