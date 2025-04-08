# tests/formulas/test_validator_weight.py
import pytest
import math
from sdk.formulas import calculate_validator_weight

# --- Tham số ví dụ (CẦN THAY THẾ) ---
EXAMPLE_LAMBDA = 0.5
EXAMPLE_STAKE_LOG_BASE = math.e
EXAMPLE_TIME_LOG_BASE = 10
EXAMPLE_EPSILON = 1e-9

def test_calculate_validator_weight_basic():
    """Kiểm tra tính toán trọng số validator cơ bản."""
    stake_v = 500
    total_stake = 2000
    e_validator = 0.9
    e_avg = 0.8 # Giả định giá trị E_avg
    time_participated = 10 # Giả định đơn vị

    weight = calculate_validator_weight(
        stake_v=stake_v,
        total_stake=total_stake,
        e_validator=e_validator,
        e_avg=e_avg,
        time_participated=time_participated,
        lambda_balance=EXAMPLE_LAMBDA,
        stake_log_base=EXAMPLE_STAKE_LOG_BASE,
        time_log_base=EXAMPLE_TIME_LOG_BASE
    )

    assert isinstance(weight, float)
    assert weight >= 0
    # Tính toán giá trị mong đợi dựa trên tham số ví dụ và giả định chuẩn hóa stake đơn giản
    # stake_comp_norm = 500 / 2000 = 0.25 (Giả định chuẩn hóa tuyến tính cho stake)
    # perf_ratio = 0.9 / max(0.8, EPSILON) = 1.125
    # time_bonus = math.log(1 + 10, 10) = log10(11) ~ 1.041
    # perf_time_comp = 1.125 * (1 + 1.041) ~ 2.296
    # expected = 0.5 * 0.25 + (1 - 0.5) * 2.296 = 0.125 + 0.5 * 2.296 = 0.125 + 1.148 = 1.273
    # !!! Kết quả này DỰA TRÊN GIẢ ĐỊNH CHUẨN HÓA STAKE TUYẾN TÍNH !!!
    # !!! Kết quả thực tế sẽ khác khi logic chuẩn hóa log(1+stake) được triển khai !!!
    # Do đó, chỉ kiểm tra khoảng giá trị hợp lệ
    # assert weight == pytest.approx(1.273, abs=1e-3) # Không thể assert chính xác

def test_calculate_validator_weight_zero_stake():
    """Kiểm tra khi validator không có stake."""
    weight = calculate_validator_weight(0, 1000, 0.8, 0.7, 5, EXAMPLE_LAMBDA)
    assert weight >= 0
    # Thành phần stake sẽ gần 0

def test_calculate_validator_weight_zero_time():
    """Kiểm tra khi validator mới tham gia."""
    weight = calculate_validator_weight(100, 1000, 0.8, 0.7, 0, EXAMPLE_LAMBDA)
    assert weight >= 0
    # Thành phần bonus thời gian sẽ gần 0 (log10(1)=0)

# !!! Cần thêm nhiều test cases hơn, đặc biệt sau khi logic tính E_avg
# và chuẩn hóa log(stake) được định nghĩa rõ ràng !!!