# tests/formulas/test_trust_score.py
import pytest
import math
from sdk.formulas import update_trust_score, calculate_selection_probability
from sdk.formulas.utils import sigmoid, calculate_alpha_effective # Import để kiểm tra

# --- Tham số ví dụ (CẦN THAY THẾ BẰNG GIÁ TRỊ THỰC TẾ SAU KHI XÁC ĐỊNH) ---
EXAMPLE_DELTA_TRUST = 0.1
EXAMPLE_ALPHA_BASE = 0.1
EXAMPLE_K_ALPHA = 1.0
EXAMPLE_UPDATE_SIG_L = 1.0
EXAMPLE_UPDATE_SIG_K = 5.0
EXAMPLE_UPDATE_SIG_X0 = 0.5
EXAMPLE_BETA = 0.2
EXAMPLE_MAX_TIME_BONUS = 10

def test_update_trust_score_with_performance():
    """Kiểm tra cập nhật trust score khi có hiệu suất mới."""
    trust_old = 0.7
    time_since = 1
    score_new = 0.8 # P_adj hoặc E_val

    trust_new = update_trust_score(
        trust_old, time_since, score_new,
        delta_trust=EXAMPLE_DELTA_TRUST,
        alpha_base=EXAMPLE_ALPHA_BASE,
        k_alpha=EXAMPLE_K_ALPHA,
        update_sigmoid_L=EXAMPLE_UPDATE_SIG_L,
        update_sigmoid_k=EXAMPLE_UPDATE_SIG_K,
        update_sigmoid_x0=EXAMPLE_UPDATE_SIG_X0
    )

    assert isinstance(trust_new, float)
    assert 0 <= trust_new <= 1
    # Tính toán giá trị mong đợi dựa trên tham số ví dụ:
    decay = 0.7 * math.exp(-0.1*1) # ~ 0.6334
    alpha_eff = 0.1 * (1 - 1.0 * abs(0.7 - 0.5)) # = 0.08
    mapped_score = 1 / (1 + math.exp(-5*(0.8-0.5))) # ~ 0.8176
    expected = decay + alpha_eff * mapped_score # ~ 0.6334 + 0.08 * 0.8176 ~ 0.6988
    assert trust_new == pytest.approx(expected, abs=1e-4)
    # !!! Cần thêm test cases với các giá trị và tham số khác !!!

def test_update_trust_score_no_performance():
    """Kiểm tra cập nhật trust score khi không được đánh giá (chỉ suy giảm)."""
    trust_old = 0.5
    time_since = 2
    score_new = 0.0 # Không được đánh giá

    trust_new = update_trust_score(
        trust_old, time_since, score_new, delta_trust=EXAMPLE_DELTA_TRUST
    )

    expected_decay = 0.5 * math.exp(-EXAMPLE_DELTA_TRUST * time_since) # ~0.409
    assert trust_new == pytest.approx(expected_decay, abs=1e-4)

def test_calculate_selection_probability():
    """Kiểm tra tính xác suất chọn lựa."""
    trust = 0.6
    time_since = 5
    prob = calculate_selection_probability(
        trust, time_since, beta=EXAMPLE_BETA, max_time_bonus_effect=EXAMPLE_MAX_TIME_BONUS
    )
    # Sửa lỗi cú pháp: Chỉ giữ lại phần tính toán giá trị mong đợi
    expected = 0.6 * (1 + EXAMPLE_BETA * min(time_since, EXAMPLE_MAX_TIME_BONUS)) # = 0.6 * (1 + 0.2 * 5) = 1.2
    assert prob == pytest.approx(expected)

def test_calculate_selection_probability_max_bonus():
    """Kiểm tra khi thời gian vượt quá giới hạn bonus."""
    trust = 0.6
    time_since = 15 # Lớn hơn MaxTimeBonusEffect = 10
    prob = calculate_selection_probability(
        trust, time_since, beta=EXAMPLE_BETA, max_time_bonus_effect=EXAMPLE_MAX_TIME_BONUS
    )
    # Bonus chỉ tính cho 10 chu kỳ
    expected = 0.6 * (1 + EXAMPLE_BETA * EXAMPLE_MAX_TIME_BONUS) # = 0.6 * (1 + 0.2 * 10) = 1.8
    assert prob == pytest.approx(expected)
