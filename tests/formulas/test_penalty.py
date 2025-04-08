# tests/formulas/test_penalty.py
import pytest
import math
from sdk.formulas import (
    calculate_performance_adjustment,
    calculate_slash_amount,
    # calculate_fraud_severity_value # Hàm này cần logic cụ thể để test
)

# --- Tham số ví dụ (CẦN THAY THẾ) ---
EXAMPLE_RECOVERY_RATE = 0.1
EXAMPLE_BASE_PERFORMANCE = 0.9
EXAMPLE_MAX_SLASH_RATE = 0.2

def test_calculate_performance_adjustment():
    """Kiểm tra hàm phục hồi hiệu suất."""
    score_new = 0.7
    adjusted = calculate_performance_adjustment(
        score_new,
        performance_score_base=EXAMPLE_BASE_PERFORMANCE,
        recovery_rate=EXAMPLE_RECOVERY_RATE
    )
    # Sửa lỗi: Chỉ giữ lại phần tính toán giá trị mong đợi
    expected = 0.7 + 0.1 * (0.9 - 0.7) # = 0.72
    assert adjusted == pytest.approx(expected)

def test_calculate_performance_adjustment_already_at_base():
    """Kiểm tra khi đã ở mức base."""
    score_new = 0.9
    adjusted = calculate_performance_adjustment(score_new, EXAMPLE_BASE_PERFORMANCE, EXAMPLE_RECOVERY_RATE)
    assert adjusted == pytest.approx(0.9)

# --- Test cho calculate_slash_amount ---

def test_calculate_slash_amount_severity_limited():
    """Kiểm tra khi mức phạt bị giới hạn bởi severity."""
    stake = 1000
    fraud_severity = 0.15 # Nhỏ hơn MaxSlashRate
    slash = calculate_slash_amount(stake, fraud_severity, max_slash_rate=EXAMPLE_MAX_SLASH_RATE)
    expected = stake * fraud_severity # 1000 * 0.15 = 150
    assert slash == pytest.approx(expected)

def test_calculate_slash_amount_max_rate_limited():
    """Kiểm tra khi mức phạt bị giới hạn bởi MaxSlashRate."""
    stake = 1000
    fraud_severity = 0.3 # Lớn hơn MaxSlashRate
    slash = calculate_slash_amount(stake, fraud_severity, max_slash_rate=EXAMPLE_MAX_SLASH_RATE)
    expected = stake * EXAMPLE_MAX_SLASH_RATE # 1000 * 0.2 = 200
    assert slash == pytest.approx(expected)

def test_calculate_slash_amount_zero_stake():
    """Kiểm tra với stake bằng 0."""
    slash = calculate_slash_amount(0, 0.5, EXAMPLE_MAX_SLASH_RATE)
    assert slash == 0.0

# --- Test cho calculate_fraud_severity_value ---
# !!! Cần viết test cases cụ thể sau khi logic phân loại bậc được định nghĩa !!!
# Ví dụ:
# def test_calculate_fraud_severity_tier1():
#     behavior_data = {"type": "Sustained_Deviation", "details": ...}
#     severity = calculate_fraud_severity_value(behavior_data)
#     assert severity == pytest.approx(EXPECTED_S1) # EXPECTED_S1 cần được định nghĩa
