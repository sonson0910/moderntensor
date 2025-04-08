# tests/formulas/test_performance.py
import pytest
import math
from sdk.formulas import (
    calculate_task_completion_rate,
    calculate_adjusted_miner_performance,
    calculate_validator_performance,
    calculate_penalty_term
)

# --- Tham số ví dụ (CẦN THAY THẾ) ---
EXAMPLE_DECAY = 0.5
EXAMPLE_THETA1 = 0.3
EXAMPLE_THETA2 = 0.4
EXAMPLE_THETA3 = 0.3
EXAMPLE_PENALTY_THRESHOLD = 0.1
EXAMPLE_PENALTY_K = 5.0
EXAMPLE_PENALTY_P = 1.0

def test_calculate_task_completion_rate():
    """Kiểm tra tính Q_task / P_miner cơ bản."""
    success = [8, 9, 10]
    total = [10, 10, 10]
    current_time = 3
    rate = calculate_task_completion_rate(success, total, current_time, decay_constant=EXAMPLE_DECAY)
    assert isinstance(rate, float)
    assert 0 <= rate <= 1
    # Giá trị gốc là ~0.932, kiểm tra lại với delta=0.5
    assert rate == pytest.approx(0.932, abs=1e-3) # Giữ lại nếu delta không đổi
    # !!! Cần kiểm tra lại giá trị nếu delta thay đổi !!!

def test_calculate_task_completion_rate_empty():
    """Kiểm tra với danh sách rỗng."""
    rate = calculate_task_completion_rate([], [], 0)
    assert rate == 0.0

def test_calculate_adjusted_miner_performance():
    """Kiểm tra tính P_miner_adjusted."""
    scores = [0.9, 0.7]
    trusts = [0.8, 0.5]
    adjusted = calculate_adjusted_miner_performance(scores, trusts)
    expected = ((0.8 * 0.9) + (0.5 * 0.7)) / (0.8 + 0.5) # ~0.823
    assert adjusted == pytest.approx(expected)

def test_calculate_adjusted_miner_performance_zero_trust():
    """Kiểm tra khi tổng trust = 0."""
    adjusted = calculate_adjusted_miner_performance([0.9], [0.0])
    assert adjusted == 0.0

# --- Tests cho hàm mới ---

def test_calculate_penalty_term():
    """Kiểm tra hàm tính thành phần phạt."""
    # Không phạt khi dưới ngưỡng
    term1 = calculate_penalty_term(0.05, threshold_dev=EXAMPLE_PENALTY_THRESHOLD, k_penalty=EXAMPLE_PENALTY_K, p_penalty=EXAMPLE_PENALTY_P)
    assert term1 == pytest.approx(1.0)

    # Có phạt khi trên ngưỡng
    term2 = calculate_penalty_term(0.2, threshold_dev=EXAMPLE_PENALTY_THRESHOLD, k_penalty=EXAMPLE_PENALTY_K, p_penalty=EXAMPLE_PENALTY_P)
    assert isinstance(term2, float)
    assert 0 < term2 < 1
    # Expected = 1 / (1 + 5.0 * max(0, 0.2 - 0.1)**1.0) = 1 / (1 + 5.0 * 0.1) = 1 / 1.5 = 0.666...
    assert term2 == pytest.approx(1 / 1.5)
    # !!! Cần thêm test cases với các giá trị k', p, threshold khác !!!

def test_calculate_validator_performance():
    """Kiểm tra tính E_validator."""
    # Giả định các giá trị đầu vào
    q_task = 0.9
    metric_quality = 0.85 # Giả định giá trị cho chỉ số mới
    deviation = 0.2

    e_val = calculate_validator_performance(
        q_task_validator=q_task,
        metric_validator_quality=metric_quality,
        deviation=deviation,
        theta1=EXAMPLE_THETA1,
        theta2=EXAMPLE_THETA2,
        theta3=EXAMPLE_THETA3,
        penalty_threshold_dev=EXAMPLE_PENALTY_THRESHOLD,
        penalty_k_penalty=EXAMPLE_PENALTY_K,
        penalty_p_penalty=EXAMPLE_PENALTY_P
    )

    assert isinstance(e_val, float)
    assert 0 <= e_val <= 1
    # Tính toán giá trị mong đợi dựa trên tham số ví dụ:
    # penalty_term = 1 / 1.5 (từ test trên)
    # expected = 0.3*0.9 + 0.4*0.85 + 0.3*(1/1.5) = 0.27 + 0.34 + 0.2 = 0.81
    assert e_val == pytest.approx(0.81)
    # !!! Cần thêm test cases với các giá trị đầu vào và tham số khác !!!
    # !!! Cần định nghĩa rõ cách tính metric_quality và kiểm tra nó !!!