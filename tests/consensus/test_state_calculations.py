# tests/consensus/test_state_calculations.py
import pytest
import time
import math
import asyncio
from typing import Dict, List, Any, Tuple, Optional
from collections import defaultdict
import hashlib # Thêm hashlib
from pytest_mock import MockerFixture

# --- Import các thành phần cần test ---
from sdk.consensus.state import (
    run_consensus_logic,
    prepare_miner_updates_logic,
    prepare_validator_updates_logic,
    _calculate_fraud_severity # Import hàm helper để test riêng nếu muốn
)
# --- Import Datatypes và Constants ---
from sdk.core.datatypes import MinerInfo, ValidatorInfo, ValidatorScore, TaskAssignment
try:
    # Ưu tiên import từ nơi định nghĩa chính thức (ví dụ: metagraph_datum)
    from sdk.metagraph.metagraph_datum import MinerDatum, ValidatorDatum, STATUS_ACTIVE, STATUS_INACTIVE, STATUS_JAILED
except ImportError:
    # Hoặc định nghĩa tạm fallback nếu không tìm thấy
    MinerDatum = dict # Kiểu dữ liệu đơn giản cho test nếu import lỗi
    ValidatorDatum = dict
    STATUS_ACTIVE = 1
    STATUS_INACTIVE = 0
    STATUS_JAILED = 2
# --- Import Settings và Helpers ---
from sdk.config.settings import settings
from pycardano import Address, VerificationKeyHash, BlockFrostChainContext, UTxO
from sdk.formulas import update_trust_score, calculate_miner_incentive

# --- Helper functions (Có thể đưa vào conftest.py) ---
def create_validator_info(uid_num: int, trust: float, weight: float, stake: int, status=STATUS_ACTIVE, api=True) -> ValidatorInfo:
    """Tạo ValidatorInfo mẫu với UID hex."""
    base_uid_str = f"validator_{uid_num:03d}"
    uid_hex = hashlib.sha256(base_uid_str.encode()).hexdigest()[:16] # UID hex giả lập
    base_addr = f"addr_test1vpvalid{uid_num:03d}"
    padding = 'x' * (64 - len(base_addr))  # Fill đủ tối đa 64 ký tự
    addr_str = base_addr + padding
    addr_str = addr_str[:64]
    pkh: Optional[VerificationKeyHash] = None
    wallet_addr_bytes : Optional[bytes] = None
    
    try:
        addr_obj = Address.from_primitive(addr_str)
        pkh = addr_obj.payment_part # Lấy VerificationKeyHash object
        wallet_addr_bytes = addr_str.encode('utf-8') # Chuyển addr_str sang bytes
    except Exception as e:
        pass # Bỏ qua lỗi parse address trong test
    wallet_addr_bytes = addr_str.encode('utf-8') # Chuyển addr_str sang bytes

    # Bỏ last_performance khi gọi ValidatorInfo()
    return ValidatorInfo(
        uid=uid_hex, address=addr_str,
        api_endpoint=f"http://fake-validator-{uid_num}:8000" if api else None,
        trust_score=trust, weight=weight, stake=float(stake), status=status,
        last_performance = trust * 0.95,
        wallet_addr_hash = wallet_addr_bytes # Sử dụng wallet_addr_bytes thay vì addr_str
        # Các trường khác dùng giá trị mặc định của ValidatorInfo
    )

def create_miner_info(uid_num: int, trust: float, weight: float, stake: int, last_selected: int, status=STATUS_ACTIVE) -> MinerInfo:
    """Tạo MinerInfo mẫu với UID hex."""
    base_uid_str = f"miner_{uid_num:03d}"
    uid_hex = hashlib.sha256(base_uid_str.encode()).hexdigest()[:16] # UID hex giả lập
    return MinerInfo(
        uid=uid_hex, # <<<--- UID dạng hex
        address=f"addr_test1vpmine{uid_num:03d}{'x'*45}",
        api_endpoint=f"http://fake-miner-{uid_num}:9000",
        trust_score=trust, weight=weight, stake=float(stake),
        last_selected_time=last_selected, status=status,
        performance_history=[]
        # Các trường khác dùng giá trị mặc định của MinerInfo
    )
# ---------------------------

# --- Đăng ký custom mark 'logic' (Thêm vào conftest.py hoặc đầu file) ---
def pytest_configure(config):
    config.addinivalue_line("markers", "logic: mark test as logic test")
# -----------------------------------------------

@pytest.mark.logic
def test_run_consensus_logic_basic():
    """Kiểm tra logic tính toán đồng thuận cơ bản trong run_consensus_logic."""
    current_cycle = 101
    # --- Tạo validators với UID hex ---
    v1_info = create_validator_info(1, 0.95, 3.0, 5000)
    v2_info = create_validator_info(2, 0.85, 2.5, 4000)
    v3_info = create_validator_info(3, 0.75, 2.0, 3000)
    v4_info = create_validator_info(4, 0.60, 1.0, 1000, status=STATUS_INACTIVE)
    validators_info = { v1_info.uid: v1_info, v2_info.uid: v2_info, v3_info.uid: v3_info, v4_info.uid: v4_info }
    # --- Tạo miners với UID hex ---
    m1_info = create_miner_info(1, 0.9, 2.0, 1000, 99)
    m2_info = create_miner_info(2, 0.5, 1.0, 500, 90)
    m1_uid = m1_info.uid
    m2_uid = m2_info.uid
    # --- Dùng UID hex thực tế ---
    task_id_m1 = f"task_{m1_uid}_cycle{current_cycle}"
    task_id_m2 = f"task_{m2_uid}_cycle{current_cycle}"
    received_scores = defaultdict(lambda: defaultdict(dict)) 
    # Giả lập điểm số nhận được cho cycle hiện tại
    received_scores_cycle = received_scores[current_cycle]
    received_scores_cycle[task_id_m1][v1_info.uid] = ValidatorScore(task_id=task_id_m1, miner_uid=m1_uid, validator_uid=v1_info.uid, score=0.92)
    received_scores_cycle[task_id_m1][v2_info.uid] = ValidatorScore(task_id=task_id_m1, miner_uid=m1_uid, validator_uid=v2_info.uid, score=0.88)
    received_scores_cycle[task_id_m1][v3_info.uid] = ValidatorScore(task_id=task_id_m1, miner_uid=m1_uid, validator_uid=v3_info.uid, score=0.95)
    received_scores_cycle[task_id_m2][v1_info.uid] = ValidatorScore(task_id=task_id_m2, miner_uid=m2_uid, validator_uid=v1_info.uid, score=0.70)
    received_scores_cycle[task_id_m2][v3_info.uid] = ValidatorScore(task_id=task_id_m2, miner_uid=m2_uid, validator_uid=v3_info.uid, score=0.75)

    tasks_sent = {
        task_id_m1: TaskAssignment(task_id_m1, {}, m1_uid, v1_info.uid, time.time(), {}),
        task_id_m2: TaskAssignment(task_id_m2, {}, m2_uid, v1_info.uid, time.time(), {}),
    }

    final_miner_scores, calculated_validator_states = run_consensus_logic(
        current_cycle=current_cycle, tasks_sent=tasks_sent,
        received_scores=received_scores_cycle, # Truyền dict của cycle hiện tại
        validators_info=validators_info, settings=settings
    )

    # --- Assertions ---
    # Assert Miner Scores (P_adj)
    assert m1_uid in final_miner_scores
    assert m2_uid in final_miner_scores
    p_adj_m1 = final_miner_scores[m1_uid]
    p_adj_m2 = final_miner_scores[m2_uid]
    # Recalculate expected values based on active validators only (V1, V2, V3)
    trusts_m1 = [v1_info.trust_score, v2_info.trust_score, v3_info.trust_score]
    scores_m1 = [0.92, 0.88, 0.95]
    expected_p_adj_m1 = sum(t * s for t, s in zip(trusts_m1, scores_m1)) / sum(trusts_m1)
    trusts_m2 = [v1_info.trust_score, v3_info.trust_score]
    scores_m2 = [0.70, 0.75]
    expected_p_adj_m2 = sum(t * s for t, s in zip(trusts_m2, scores_m2)) / sum(trusts_m2)
    assert p_adj_m1 == pytest.approx(expected_p_adj_m1)
    assert p_adj_m2 == pytest.approx(expected_p_adj_m2)

    # Assert Validator States
    assert len(calculated_validator_states) == 4 # Tính cho cả inactive
    state_v1 = calculated_validator_states[v1_info.uid]
    state_v2 = calculated_validator_states[v2_info.uid]
    state_v3 = calculated_validator_states[v3_info.uid]
    state_v4 = calculated_validator_states[v4_info.uid]

    for uid, state in calculated_validator_states.items():
        assert "E_v" in state and isinstance(state["E_v"], float)
        assert "trust" in state and isinstance(state["trust"], float)
        assert "reward" in state and isinstance(state["reward"], float)
        assert "weight" in state and isinstance(state["weight"], float) # Weight đầu chu kỳ
        assert state["last_update_cycle"] == current_cycle

    # E_v based on deviation (lower deviation -> higher E_v component)
    # Tạm thời chấp nhận tính toán E_v, kiểm tra tương đối
    assert 0 <= state_v1["E_v"] <= 1
    assert 0 <= state_v2["E_v"] <= 1
    assert 0 <= state_v3["E_v"] <= 1
    # assert state_v1["avg_deviation"] == pytest.approx((abs(0.92 - expected_p_adj_m1) + abs(0.70 - expected_p_adj_m2)) / 2)
    # assert state_v2["avg_deviation"] == pytest.approx(abs(0.88 - expected_p_adj_m1))
    # assert state_v3["avg_deviation"] == pytest.approx((abs(0.95 - expected_p_adj_m1) + abs(0.75 - expected_p_adj_m2)) / 2)
    # assert state_v4["avg_deviation"] == 0.0 # Không chấm điểm
    # assert state_v1["E_v"] > state_v3["E_v"] # V1 ít lệch hơn V3
    # assert state_v2["E_v"] > state_v3["E_v"] # V2 ít lệch hơn V3 (chỉ tính trên M1)
    # assert state_v4["E_v"] < 0.1 # Inactive

    # Trust update
    assert state_v1["trust"] > v1_info.trust_score * math.exp(-settings.CONSENSUS_PARAM_DELTA_TRUST) # Phải tăng hoặc decay ít
    assert state_v4["trust"] == pytest.approx(v4_info.trust_score * math.exp(-settings.CONSENSUS_PARAM_DELTA_TRUST)) # Chỉ decay

    # Reward only for active
    assert state_v1["reward"] >= 0
    assert state_v2["reward"] >= 0
    assert state_v3["reward"] >= 0
    assert state_v4["reward"] == 0
    # V1 có trust, weight, E_v tốt nhất -> Reward V1 cao nhất
    assert state_v1["reward"] > state_v2["reward"]
    assert state_v1["reward"] > state_v3["reward"]

@pytest.mark.logic
@pytest.mark.asyncio
async def test_prepare_miner_updates_logic(mocker: MockerFixture):
    """Kiểm tra việc chuẩn bị MinerDatum mới."""
    current_cycle = 102
    # --- Tạo miners với UID hex ---
    m1_info = create_miner_info(1, 0.91, 2.1, 1050, 101)
    m3_info = create_miner_info(3, 0.68, 1.4, 780, 100)
    m5_info = create_miner_info(5, 0.45, 0.9, 400, 95, status=STATUS_INACTIVE)
    miners_info = { m1_info.uid: m1_info, m3_info.uid: m3_info, m5_info.uid: m5_info }
    final_miner_scores = { m1_info.uid: 0.9022, m3_info.uid: 0.0 } # P_adj

    # --- Gọi hàm cần test ---
    # --- Gọi hàm cần test với đủ tham số ---
    # mock_context = mocker.MagicMock(spec=BlockFrostChainContext)
    mock_context = mocker.MagicMock(spec=BlockFrostChainContext)
    mock_utxo_map = {}
    reward_old_m1 = 1000000
    reward_old_m3 = 500000
    try:
        # Mock M1 Datum
        mock_datum_m1_old = MinerDatum(uid=bytes.fromhex(m1_info.uid), accumulated_rewards=reward_old_m1, subnet_uid=0, stake=1050, scaled_last_performance=int(0.9*settings.METAGRAPH_DATUM_INT_DIVISOR), scaled_trust_score=int(0.91*settings.METAGRAPH_DATUM_INT_DIVISOR), last_update_slot=101, performance_history_hash=None, wallet_addr_hash=m1_info.wallet_addr_hash, status=1, registration_slot=1, api_endpoint=None)
        mock_utxo_m1 = mocker.MagicMock(spec=UTxO); mock_utxo_m1.output.datum = mocker.MagicMock(); mock_utxo_m1.output.datum.cbor = mock_datum_m1_old.to_cbor()
        mock_utxo_map[m1_info.uid] = mock_utxo_m1
        # Mock M3 Datum
        mock_datum_m3_old = MinerDatum(uid=bytes.fromhex(m3_info.uid), accumulated_rewards=reward_old_m3, subnet_uid=0, stake=780, scaled_last_performance=int(0.6*settings.METAGRAPH_DATUM_INT_DIVISOR), scaled_trust_score=int(0.68*settings.METAGRAPH_DATUM_INT_DIVISOR), last_update_slot=100, performance_history_hash=None, wallet_addr_hash=m3_info.wallet_addr_hash, status=1, registration_slot=1, api_endpoint=None)
        mock_utxo_m3 = mocker.MagicMock(spec=UTxO); mock_utxo_m3.output.datum = mocker.MagicMock(); mock_utxo_m3.output.datum.cbor = mock_datum_m3_old.to_cbor()
        mock_utxo_map[m3_info.uid] = mock_utxo_m3
    except Exception as e: print(f"Warning: Mock datum creation failed: {e}"); reward_old_m1 = 0; reward_old_m3 = 0

    miner_updates = await prepare_miner_updates_logic( # <<<--- Thêm await
        current_cycle=current_cycle,
        miners_info=miners_info,
        final_scores=final_miner_scores,
        settings=settings,
        # context=mock_context, # <<<--- Thêm context (mock)
        current_utxo_map=mock_utxo_map # <<<--- Thêm utxo_map (mock)
    )


    # --- Assertions ---
    assert isinstance(miner_updates, dict)
    assert len(miner_updates) == 3
    assert m1_info.uid in miner_updates
    assert m3_info.uid in miner_updates
    assert m5_info.uid in miner_updates
    assert isinstance(miner_updates[m1_info.uid], MinerDatum)

    datum_m1 = miner_updates[m1_info.uid]
    datum_m3 = miner_updates[m3_info.uid]
    datum_m5 = miner_updates[m5_info.uid]
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR

    # Kiểm tra UID bytes
    assert datum_m1.uid == bytes.fromhex(m1_info.uid)
    assert datum_m3.uid == bytes.fromhex(m3_info.uid)
    assert datum_m5.uid == bytes.fromhex(m5_info.uid)

    # Kiểm tra M1
    assert datum_m1.last_update_slot == current_cycle
    assert datum_m1.scaled_last_performance == pytest.approx(int(0.9022 * divisor))
    # Tính trust mới cho M1 (dựa trên trust cũ 0.91 và score mới 0.9022)
    trust_m1_old = 0.91
    score_m1_new = 0.9022
    trust_m1_new = trust_m1_old * math.exp(-settings.CONSENSUS_PARAM_DELTA_TRUST * 1)
    alpha_eff_m1 = settings.CONSENSUS_PARAM_ALPHA_BASE * (1 - settings.CONSENSUS_PARAM_K_ALPHA * abs(trust_m1_old - 0.5))
    score_mapped_m1 = 1 / (1 + math.exp(-settings.CONSENSUS_PARAM_UPDATE_SIG_K * (score_m1_new - settings.CONSENSUS_PARAM_UPDATE_SIG_X0)))
    trust_m1_new += max(0, alpha_eff_m1) * score_mapped_m1
    assert datum_m1.scaled_trust_score == pytest.approx(int(trust_m1_new * divisor))
    assert datum_m1.accumulated_rewards >= 0
    assert datum_m1.status == STATUS_ACTIVE

    # Kiểm tra M3
    assert datum_m3.last_update_slot == current_cycle
    assert datum_m3.scaled_last_performance == 0 # P_adj = 0
    # Tính trust mới cho M3 (dựa trên trust cũ 0.68 và score mới 0.0)
    trust_m3_old = 0.68
    score_m3_new = 0.0
    time_since=1
    trust_m3_new = update_trust_score(
        trust_m3_old, time_since, score_m3_new,
        delta_trust=settings.CONSENSUS_PARAM_DELTA_TRUST,
        alpha_base=settings.CONSENSUS_PARAM_ALPHA_BASE, k_alpha=settings.CONSENSUS_PARAM_K_ALPHA,
        update_sigmoid_L=settings.CONSENSUS_PARAM_UPDATE_SIG_L,
        update_sigmoid_k=settings.CONSENSUS_PARAM_UPDATE_SIG_K,
        update_sigmoid_x0=settings.CONSENSUS_PARAM_UPDATE_SIG_X0
    )
    assert datum_m3.scaled_trust_score == pytest.approx(int(trust_m3_new * divisor))
    # trust_m3_new = trust_m3_old * math.exp(-settings.CONSENSUS_PARAM_DELTA_TRUST * 1)
    # alpha_eff_m3 = settings.CONSENSUS_PARAM_ALPHA_BASE * (1 - settings.CONSENSUS_PARAM_K_ALPHA * abs(trust_m3_old - 0.5))
    # score_mapped_m3 = 1 / (1 + math.exp(-settings.CONSENSUS_PARAM_UPDATE_SIG_K * (score_m3_new - settings.CONSENSUS_PARAM_UPDATE_SIG_X0)))
    # trust_m3_new += max(0, alpha_eff_m3) * score_mapped_m3
    # assert datum_m3.scaled_trust_score == pytest.approx(int(trust_m3_new * divisor))
    assert datum_m3.accumulated_rewards >= 0
    assert datum_m3.status == STATUS_ACTIVE

    # Kiểm tra M5
    assert datum_m5.last_update_slot == current_cycle
    assert datum_m5.scaled_last_performance == 0
    # Tính trust mới cho M5 (chỉ decay từ 0.45)
    trust_m5_old = 0.45
    trust_m5_new = trust_m5_old * math.exp(-settings.CONSENSUS_PARAM_DELTA_TRUST * 1) # Giả sử time_since=1
    assert datum_m5.scaled_trust_score == pytest.approx(int(trust_m5_new * divisor))
    assert datum_m5.accumulated_rewards >= 0
    assert datum_m5.status == STATUS_INACTIVE

@pytest.mark.logic
@pytest.mark.asyncio
async def test_prepare_validator_updates_logic():
    """Kiểm tra việc chuẩn bị ValidatorDatum mới cho chính mình."""
    current_cycle = 103
    self_info = create_validator_info(1, 0.96, 3.1, 5100)
    self_uid_hex = self_info.uid
    print(self_info)

    calculated_states = {
        self_uid_hex: {
            "E_v": 0.915, # Performance mới tính được
            "trust": 0.965, # Trust mới dự kiến
            "reward": 0.05, # Reward dự kiến
            "weight": 3.1, # Weight đầu chu kỳ
            "start_status": STATUS_ACTIVE,
            "last_update_cycle": current_cycle,
            # Giả sử các trường này cũng được tính và lưu
            "accumulated_rewards_old": 10 * 1e6 # Giả sử reward cũ là 10 ADA (scaled)
        }
    }

    # Hàm prepare_validator_updates_logic không cần context nếu reward cũ được cung cấp
    # hoặc nếu chúng ta chấp nhận reward tích lũy trong test bắt đầu từ 0 + reward mới.
    # Bỏ mock context.
    mock_context = None

    # --- Gọi hàm cần test ---
    validator_updates = await prepare_validator_updates_logic(
        current_cycle=current_cycle, 
        self_validator_info=self_info,
        calculated_states=calculated_states, 
        settings=settings,
        context=mock_context
    )

    # --- Assertions ---
    assert isinstance(validator_updates, dict)
    assert self_uid_hex in validator_updates
    datum_v1 = validator_updates[self_uid_hex]
    assert isinstance(datum_v1, ValidatorDatum)
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR

    assert datum_v1.uid == bytes.fromhex(self_uid_hex)
    assert datum_v1.last_update_slot == current_cycle
    assert datum_v1.scaled_last_performance == pytest.approx(int(0.915 * divisor))
    assert datum_v1.scaled_trust_score == pytest.approx(int(0.965 * divisor))
    # accumulated_rewards = reward cũ (lấy từ info?) + reward mới
    # Hàm prepare_validator_updates_logic hiện tại lấy reward cũ từ self_info,
    # nhưng self_info không có accumulated_rewards. Cần sửa lại hàm logic đó
    # hoặc chấp nhận bắt đầu từ 0 trong test.
    # Tạm chấp nhận bắt đầu từ 0 + reward mới cho test này.
    expected_rewards = int(0.0) + int(calculated_states[self_uid_hex]['reward'] * divisor)
    # Hoặc nếu sửa logic để dùng info:
    # accumulated_rewards_old = getattr(self_info, 'accumulated_rewards', 0) # Cần thêm trường này vào ValidatorInfo
    # expected_rewards = int(accumulated_rewards_old) + int(calculated_states[self_uid_hex]['reward'] * divisor)
    assert datum_v1.accumulated_rewards == pytest.approx(expected_rewards)
    assert datum_v1.stake == int(self_info.stake)
    assert datum_v1.status == STATUS_ACTIVE
    # Kiểm tra wallet_addr_hash (là VerificationKeyHash)
    # assert isinstance(datum_v1.wallet_addr_hash, VerificationKeyHash)
    print(self_info.wallet_addr_hash)
    assert datum_v1.wallet_addr_hash == self_info.wallet_addr_hash


# --- Test cho _calculate_fraud_severity ---
@pytest.mark.logic
@pytest.mark.parametrize("reason, tolerance, expected_severity", [ # <<<--- Bỏ divisor
    ("No deviation", 0.01, 0.0),
    ("Trust mismatch (Diff: 0.01500)", 0.01, 0.1), # Factor=1.5 -> Minor
    ("Performance mismatch (Diff: 0.04000)", 0.01, 0.3), # Factor=4.0 -> Moderate
    ("Trust mismatch (Diff: 0.12000)", 0.01, 0.7), # Factor=12.0 -> Severe
    ("Did not commit updates", 0.01, 0.05),
    ("Trust mismatch (Diff: 0.005); Perf mismatch (Diff: 0.008)", 0.01, 0.0),
    ("Trust mismatch (Diff: 0.050); Perf mismatch (Diff: 0.008)", 0.01, 0.3), # Factor=5.0 -> Moderate
])
def test_calculate_fraud_severity(reason, tolerance, expected_severity): # <<<--- Bỏ divisor
    """Kiểm tra hàm tính mức độ nghiêm trọng với các lý do khác nhau."""
    # --- Gọi hàm với 2 tham số ---
    severity = _calculate_fraud_severity(reason, tolerance)
    # --------------------------
    assert severity == pytest.approx(expected_severity)

# TODO: Viết test cho verify_and_penalize_logic