# tests/formulas/test_incentive.py
import pytest
import math
from sdk.formulas import calculate_miner_incentive, calculate_validator_incentive

# --- Tham số ví dụ (CẦN THAY THẾ BẰNG GIÁ TRỊ THỰC TẾ SAU KHI XÁC ĐỊNH) ---
EXAMPLE_INCENTIVE_SIGMOID_L = 1.0
EXAMPLE_INCENTIVE_SIGMOID_K = 10.0
EXAMPLE_INCENTIVE_SIGMOID_X0 = 0.5

def test_calculate_miner_incentive_basic():
    """Kiểm tra tính toán cơ bản incentive cho miner."""
    trust_score = 0.85
    miner_weight = 2.0
    # Giả sử P_adj đã được tính toán
    miner_performance_scores = [0.9] # Đơn giản hóa: Coi như chỉ có 1 validator đánh giá P_adj
    total_system_value = 50.0

    incentive = calculate_miner_incentive(
        trust_score=trust_score,
        miner_weight=miner_weight,
        miner_performance_scores=miner_performance_scores, # Cần P_xj thực tế
        total_system_value=total_system_value,
        incentive_sigmoid_L=EXAMPLE_INCENTIVE_SIGMOID_L,
        incentive_sigmoid_k=EXAMPLE_INCENTIVE_SIGMOID_K,
        incentive_sigmoid_x0=EXAMPLE_INCENTIVE_SIGMOID_X0
    )

    assert isinstance(incentive, float)
    assert 0 <= incentive <= 1.1 # Kiểm tra khoảng giá trị hợp lý (cho phép L > 1)
    # !!! Cần thay thế bằng assert incentive == pytest.approx(GIÁ_TRỊ_MONG_ĐỢI_CHÍNH_XÁC) !!!

def test_calculate_miner_incentive_zero_total_value():
    """Kiểm tra trường hợp total_system_value = 0."""
    incentive = calculate_miner_incentive(0.8, 2.0, [0.9], 0.0)
    assert incentive == 0.0

def test_calculate_miner_incentive_edge_trust():
    """Kiểm tra với trust score ở các biên."""
    # Trust = 0
    inc_0 = calculate_miner_incentive(0.0, 2.0, [0.9], 50.0, incentive_sigmoid_k=10, incentive_sigmoid_x0=0.5)
    assert inc_0 >= 0
    # Trust = 1
    inc_1 = calculate_miner_incentive(1.0, 2.0, [0.9], 50.0, incentive_sigmoid_k=10, incentive_sigmoid_x0=0.5)
    assert inc_1 >= 0
    # !!! Cần tính giá trị mong đợi chính xác dựa trên tham số sigmoid !!!

# --- Tương tự cho calculate_validator_incentive ---

def test_calculate_validator_incentive_basic():
    """Kiểm tra tính toán cơ bản incentive cho validator."""
    trust_score = 0.9
    validator_weight = 1.5
    validator_performance = 0.95 # E_v
    total_validator_value = 60.0

    incentive = calculate_validator_incentive(
        trust_score=trust_score,
        validator_weight=validator_weight,
        validator_performance=validator_performance,
        total_validator_value=total_validator_value,
        incentive_sigmoid_L=EXAMPLE_INCENTIVE_SIGMOID_L,
        incentive_sigmoid_k=EXAMPLE_INCENTIVE_SIGMOID_K,
        incentive_sigmoid_x0=EXAMPLE_INCENTIVE_SIGMOID_X0
    )

    assert isinstance(incentive, float)
    assert 0 <= incentive <= 1.1 # Kiểm tra khoảng giá trị hợp lý
    # !!! Cần thay thế bằng assert incentive == pytest.approx(GIÁ_TRỊ_MONG_ĐỢI_CHÍNH_XÁC) !!!

def test_calculate_validator_incentive_zero_total_value():
    """Kiểm tra trường hợp total_validator_value = 0."""
    incentive = calculate_validator_incentive(0.9, 1.5, 0.95, 0.0)
    assert incentive == 0.0