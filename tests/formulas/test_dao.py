# tests/formulas/test_dao.py
import pytest
import math
from sdk.formulas import calculate_voting_power

# --- Tham số ví dụ (CẦN THAY THẾ) ---
EXAMPLE_KG = 1.0 # Hệ số bonus thời gian
EXAMPLE_TOTAL_TIME = 10.0 # Thời gian tham chiếu (ví dụ: 10 năm/tháng/ngày?)

def test_calculate_voting_power_basic():
    """Kiểm tra tính voting power cơ bản (không khóa stake)."""
    stake = 1000.0
    time_staked = 5.0 # Cùng đơn vị với total_time
    lockup_multiplier = 1.0 # Không khóa

    vp = calculate_voting_power(
        stake_p=stake,
        time_staked_p=time_staked,
        total_time=EXAMPLE_TOTAL_TIME,
        lockup_multiplier=lockup_multiplier,
        time_bonus_factor_kg=EXAMPLE_KG
    )

    assert isinstance(vp, float)
    assert vp >= 0
    # Tính toán mong đợi:
    # time_ratio = 5.0 / 10.0 = 0.5
    # time_bonus = 1.0 * sqrt(0.5) ~ 0.707
    # expected = 1000 * (1 + 0.707) * 1.0 = 1707
    assert vp == pytest.approx(1000 * (1 + EXAMPLE_KG * math.sqrt(0.5)))
    # !!! Cần kiểm tra lại giá trị nếu KG hoặc logic thay đổi !!!

def test_calculate_voting_power_with_lockup():
    """Kiểm tra voting power khi có khóa stake."""
    stake = 1000.0
    time_staked = 5.0
    lockup_multiplier = 1.5 # Ví dụ khóa 6 tháng

    vp = calculate_voting_power(
        stake_p=stake,
        time_staked_p=time_staked,
        total_time=EXAMPLE_TOTAL_TIME,
        lockup_multiplier=lockup_multiplier, # > 1
        time_bonus_factor_kg=EXAMPLE_KG
    )
    expected_no_lockup = 1000 * (1 + EXAMPLE_KG * math.sqrt(0.5))
    assert vp == pytest.approx(expected_no_lockup * lockup_multiplier)

def test_calculate_voting_power_zero_time():
    """Kiểm tra khi thời gian stake bằng 0."""
    stake = 1000.0
    time_staked = 0.0
    vp = calculate_voting_power(stake, time_staked, EXAMPLE_TOTAL_TIME)
    # Bonus thời gian = 0
    assert vp == pytest.approx(stake * (1 + 0) * 1.0)

def test_calculate_voting_power_zero_stake():
    """Kiểm tra khi stake bằng 0."""
    vp = calculate_voting_power(0, 5.0, EXAMPLE_TOTAL_TIME)
    assert vp == 0.0

# !!! Cần định nghĩa rõ đơn vị thời gian và giá trị total_time !!!
# !!! Cần thiết kế cơ chế xác định lockup_multiplier !!!