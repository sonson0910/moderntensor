# tests/consensus/test_selection.py
import pytest
import random
from typing import Dict

# --- Import các thành phần cần test ---
from sdk.consensus.selection import select_miners_logic
from sdk.core.datatypes import MinerInfo
from sdk.metagraph.metagraph_datum import STATUS_ACTIVE, STATUS_INACTIVE
from sdk.config.settings import settings # Import settings để lấy tham số

# --- Hàm tạo MinerInfo mẫu (Có thể đưa vào conftest.py nếu dùng nhiều) ---
def create_miner_info(uid_num: int, trust: float, weight: float, stake: int, last_selected: int, status=STATUS_ACTIVE) -> MinerInfo:
    """Hàm helper tạo MinerInfo cho test."""
    uid = f"miner_{uid_num:03d}"
    # Giữ các trường tối thiểu cần thiết cho logic select_miners_logic
    return MinerInfo(
        uid=uid,
        address=f"addr_test_miner_{uid_num}",
        api_endpoint=None,
        trust_score=trust,
        weight=weight, # Giờ MinerInfo đã có weight
        stake=float(stake), # Giờ MinerInfo đã có stake
        last_selected_time=last_selected,
        performance_history=[],
        status=status, # Gán vào trường status mới
        subnet_uid=0,
        registration_slot=0,
        wallet_addr_hash=None,
        performance_history_hash=None
    )
# ---------------------------------------------------------------------

@pytest.mark.logic
def test_select_miners_logic_selection():
    """Kiểm tra logic chọn miner cơ bản."""
    current_cycle = 100
    miners = {
        "miner_001": create_miner_info(1, 0.9, 2.0, 1000, 99, status=STATUS_ACTIVE),
        "miner_002": create_miner_info(2, 0.5, 1.0, 500, 90, status=STATUS_ACTIVE),
        "miner_003": create_miner_info(3, 0.7, 1.5, 800, 95, status=STATUS_ACTIVE),
        "miner_004": create_miner_info(4, 0.95, 2.2, 1200, 98, status=STATUS_ACTIVE),
        "miner_005": create_miner_info(5, 0.4, 0.8, 400, 85, status=STATUS_INACTIVE),
        "miner_006": create_miner_info(6, 0.6, 1.1, 600, 99, status=STATUS_ACTIVE),
        "miner_007": create_miner_info(7, 0.1, 0.5, 100, 90, status=STATUS_ACTIVE),
    }
    # --- Lấy tham số từ settings ---
    num_to_select = settings.CONSENSUS_NUM_MINERS_TO_SELECT
    beta = settings.CONSENSUS_PARAM_BETA
    max_bonus = settings.CONSENSUS_PARAM_MAX_TIME_BONUS
    # -----------------------------

    print("\nCalculating selection factors:")
    factors = {}
    for m_uid, m_info in miners.items():
         if m_info.status == STATUS_ACTIVE:
             time_since = max(0, current_cycle - m_info.last_selected_time)
             effective_time = min(time_since, max_bonus)
             factor = m_info.trust_score * (1 + beta * effective_time)
             factors[m_uid] = factor
             print(f" - {m_uid}: Trust={m_info.trust_score:.2f}, TimeSince={time_since}, Factor={factor:.3f}")

    selected = select_miners_logic(
        miners_info=miners,
        current_cycle=current_cycle,
        num_to_select=num_to_select, # <<<--- Thêm
        beta=beta,                   # <<<--- Thêm
        max_time_bonus=max_bonus     # <<<--- Thêm
    )

    print(f"Selected miners ({len(selected)}/{num_to_select}): {[m.uid for m in selected]}")

    # Assertions cơ bản
    assert 0 < len(selected) <= num_to_select
    selected_uids = set()
    for miner in selected:
        assert miner.status == STATUS_ACTIVE
        assert miner.uid != "miner_005" # Không chọn inactive
        assert miner.uid in miners # UID hợp lệ
        selected_uids.add(miner.uid)
    assert len(selected_uids) == len(selected) # Đảm bảo không chọn trùng

    # Kiểm tra xem miner chờ lâu (M2, M3, M7) có khả năng được chọn cao hơn không
    # Test này có thể fail ngẫu nhiên, nhưng giúp hình dung
    # assert "miner_002" in selected_uids or "miner_003" in selected_uids


@pytest.mark.logic
def test_select_miners_logic_no_active():
    """Kiểm tra khi không có miner active."""
    current_cycle = 100
    miners = {
        "miner_005": create_miner_info(5, 0.4, 0.8, 400, 85, status=STATUS_INACTIVE),
    }
    num_to_select = settings.CONSENSUS_NUM_MINERS_TO_SELECT
    beta = settings.CONSENSUS_PARAM_BETA
    max_bonus = settings.CONSENSUS_PARAM_MAX_TIME_BONUS
    selected = select_miners_logic(miners, current_cycle, num_to_select, beta, max_bonus)
    assert len(selected) == 0

@pytest.mark.logic
def test_select_miners_logic_empty():
    """Kiểm tra khi không có miner nào."""
    current_cycle = 100
    miners = {}
    num_to_select = settings.CONSENSUS_NUM_MINERS_TO_SELECT
    beta = settings.CONSENSUS_PARAM_BETA
    max_bonus = settings.CONSENSUS_PARAM_MAX_TIME_BONUS
    # --- Gọi hàm với đủ tham số ---
    selected = select_miners_logic(miners, current_cycle, num_to_select, beta, max_bonus) # <<<--- Sửa lỗi thiếu tham số
    # ---------------------------
    assert len(selected) == 0

@pytest.mark.logic
def test_select_miners_logic_all_equal_prob():
    """Kiểm tra khi tất cả active miner có xác suất xấp xỉ bằng nhau."""
    current_cycle = 50
    miners = {
        "miner_101": create_miner_info(101, 0.8, 1.0, 500, 49, status=STATUS_ACTIVE),
        "miner_102": create_miner_info(102, 0.8, 1.0, 500, 49, status=STATUS_ACTIVE),
        "miner_103": create_miner_info(103, 0.8, 1.0, 500, 49, status=STATUS_ACTIVE),
        "miner_104": create_miner_info(104, 0.8, 1.0, 500, 49, status=STATUS_ACTIVE),
    }
    num_to_select = 2
    beta = settings.CONSENSUS_PARAM_BETA
    max_bonus = settings.CONSENSUS_PARAM_MAX_TIME_BONUS

    # --- Gọi hàm với đủ tham số ---
    selected = select_miners_logic(miners, current_cycle, num_to_select, beta, max_bonus)
    # ---------------------------
    assert len(selected) == num_to_select
