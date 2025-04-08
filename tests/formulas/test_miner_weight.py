# tests/formulas/test_miner_weight.py
import pytest
import math
from sdk.formulas.miner_weight import calculate_miner_weight

# --- Tham số ví dụ (CẦN THAY THẾ BẰNG GIÁ TRỊ THỰC TẾ SAU KHI XÁC ĐỊNH) ---
EXAMPLE_DECAY_W = 0.5

def test_calculate_miner_weight_basic():
    """Kiểm tra tính trọng số miner với lịch sử và decay mặc định."""
    performance_history = [0.8, 0.9, 1.0]
    current_time_step = len(performance_history) # T = 3
    decay_constant = EXAMPLE_DECAY_W # 0.5

    weight = calculate_miner_weight(performance_history, current_time_step, decay_constant)

    # Tính toán thủ công giá trị mong đợi:
    # t=0: 0.8 * exp(-0.5 * (3-0)) = 0.8 * exp(-1.5) ~ 0.1785
    # t=1: 0.9 * exp(-0.5 * (3-1)) = 0.9 * exp(-1.0) ~ 0.3311
    # t=2: 1.0 * exp(-0.5 * (3-2)) = 1.0 * exp(-0.5) ~ 0.6065
    # Expected = 0.1785 + 0.3311 + 0.6065 = 1.1161
    expected_weight = (0.8 * math.exp(-decay_constant * 3) +
                       0.9 * math.exp(-decay_constant * 2) +
                       1.0 * math.exp(-decay_constant * 1))

    assert isinstance(weight, float)
    assert weight >= 0
    assert weight == pytest.approx(expected_weight, abs=1e-4)
    # !!! Giá trị assert cần được kiểm tra lại với tham số delta_W cuối cùng !!!

def test_calculate_miner_weight_different_decay():
    """Kiểm tra tính trọng số miner với decay_constant_W khác."""
    performance_history = [0.8, 0.9, 1.0]
    current_time_step = len(performance_history) # T = 3
    decay_constant = 0.1 # Giá trị decay khác

    weight = calculate_miner_weight(performance_history, current_time_step, decay_constant)

    # Tính toán thủ công giá trị mong đợi:
    # t=0: 0.8 * exp(-0.1 * 3) ~ 0.5927
    # t=1: 0.9 * exp(-0.1 * 2) ~ 0.7369
    # t=2: 1.0 * exp(-0.1 * 1) ~ 0.9048
    # Expected = 0.5927 + 0.7369 + 0.9048 = 2.2344
    expected_weight = (0.8 * math.exp(-decay_constant * 3) +
                       0.9 * math.exp(-decay_constant * 2) +
                       1.0 * math.exp(-decay_constant * 1))

    assert isinstance(weight, float)
    assert weight >= 0
    assert weight == pytest.approx(expected_weight, abs=1e-4)
    # !!! Giá trị assert cần được kiểm tra lại với tham số delta_W cuối cùng !!!

def test_calculate_miner_weight_empty_history():
    """Kiểm tra khi lịch sử hiệu suất rỗng."""
    performance_history = []
    current_time_step = 0
    weight = calculate_miner_weight(performance_history, current_time_step, EXAMPLE_DECAY_W)
    assert weight == 0.0

def test_calculate_miner_weight_single_point():
    """Kiểm tra khi chỉ có một điểm dữ liệu trong lịch sử."""
    performance_history = [0.75]
    current_time_step = len(performance_history) # T = 1
    decay_constant = EXAMPLE_DECAY_W

    weight = calculate_miner_weight(performance_history, current_time_step, decay_constant)

    # Expected = 0.75 * exp(-0.5 * (1-0)) = 0.75 * exp(-0.5) ~ 0.4549
    expected_weight = 0.75 * math.exp(-decay_constant * 1)
    assert weight == pytest.approx(expected_weight, abs=1e-4)

def test_calculate_miner_weight_time_step_greater_than_history():
    """Kiểm tra khi current_time_step lớn hơn độ dài lịch sử (vẫn hợp lệ)."""
    performance_history = [0.8, 0.9] # len = 2
    current_time_step = 3 # T = 3
    decay_constant = EXAMPLE_DECAY_W

    weight = calculate_miner_weight(performance_history, current_time_step, decay_constant)

    # Logic trong hàm sẽ dùng effective_T = current_time_step = 3
    # t=0: 0.8 * exp(-0.5 * (3-0)) ~ 0.1785
    # t=1: 0.9 * exp(-0.5 * (3-1)) ~ 0.3311
    # Expected = 0.1785 + 0.3311 = 0.5096
    expected_weight = (0.8 * math.exp(-decay_constant * 3) +
                       0.9 * math.exp(-decay_constant * 2))

    assert weight == pytest.approx(expected_weight, abs=1e-4)

def test_calculate_miner_weight_negative_performance():
    """Kiểm tra với điểm hiệu suất âm (dù không mong đợi)."""
    performance_history = [0.8, -0.5, 1.0] # Điểm âm ở giữa
    current_time_step = len(performance_history)
    decay_constant = EXAMPLE_DECAY_W

    weight = calculate_miner_weight(performance_history, current_time_step, decay_constant)

    expected_weight = (0.8 * math.exp(-decay_constant * 3) +
                       -0.5 * math.exp(-decay_constant * 2) + # Điểm âm
                       1.0 * math.exp(-decay_constant * 1))
    # Kết quả có thể âm nếu điểm âm đủ lớn và gần đây
    assert isinstance(weight, float)
    # Hàm gốc có max(0.0, weight) nên kết quả cuối cùng không âm
    assert weight >= 0
    assert weight == pytest.approx(max(0.0, expected_weight), abs=1e-4)

