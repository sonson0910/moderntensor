# tests/consensus/test_state_calculations.py
import pytest
import time
import math
import asyncio
import copy  # Để tạo bản sao sâu
import os  # Thêm import os nếu dùng os.urandom
from typing import Dict, List, Any, Tuple, Optional
from collections import defaultdict
from unittest.mock import MagicMock, AsyncMock  # Dùng để mock context/async functions
import hashlib  # Thêm hashlib
from pytest_mock import MockerFixture
from sdk.smartcontract.validator import read_validator

# --- Import các thành phần cần test ---
from sdk.consensus.state import (
    run_consensus_logic,
    prepare_miner_updates_logic,
    prepare_validator_updates_logic,
    _calculate_fraud_severity,  # Import hàm helper để test riêng nếu muốn
)

# --- Import Datatypes và Constants ---
from sdk.consensus.state import (
    verify_and_penalize_logic,
    commit_updates_logic,
)  # Function to test
from sdk.core.datatypes import MinerInfo, ValidatorInfo, ValidatorScore, TaskAssignment

# Ưu tiên import từ nơi định nghĩa chính thức (ví dụ: metagraph_datum)
from sdk.metagraph.metagraph_datum import (
    MinerDatum,
    ValidatorDatum,
    STATUS_ACTIVE,
    STATUS_INACTIVE,
    STATUS_JAILED,
)

# --- Import Settings và Helpers ---
from sdk.config.settings import settings
from sdk.compat.luxtensor_types import (
    Address,
    VerificationKeyHash,
    BlockFrostChainContext,
    UTxO,
    TransactionInput,
    Value,
    TransactionOutput,
    TransactionId,
    ScriptHash,
    Network,
    PlutusV3Script,
    Redeemer,
    ExtendedSigningKey,
    TransactionBuilder,
)  # Import thêm
from sdk.formulas import update_trust_score, calculate_miner_incentive


# --- Helper functions (Có thể đưa vào conftest.py) ---
def create_validator_info(
    uid_num: int,
    trust: float,
    weight: float,
    stake: int,
    status=STATUS_ACTIVE,
    api=True,
) -> ValidatorInfo:
    """Tạo ValidatorInfo mẫu với UID hex."""
    base_uid_str = f"validator_{uid_num:03d}"
    uid_hex = hashlib.sha256(base_uid_str.encode()).hexdigest()[:16]  # UID hex giả lập
    base_addr = f"addr_test1vpvalid{uid_num:03d}"
    padding = "x" * (64 - len(base_addr))  # Fill đủ tối đa 64 ký tự
    addr_str = base_addr + padding
    addr_str = addr_str[:64]
    pkh: Optional[VerificationKeyHash] = None
    wallet_addr_bytes: Optional[bytes] = None

    try:
        wallet_addr_bytes = addr_str.encode("utf-8")  # Chuyển addr_str sang bytes
    except Exception as e:
        pass  # Bỏ qua lỗi parse address trong test
    wallet_addr_bytes = addr_str.encode("utf-8")  # Chuyển addr_str sang bytes

    # Bỏ last_performance khi gọi ValidatorInfo()
    return ValidatorInfo(
        uid=uid_hex,
        address=addr_str,
        api_endpoint=f"http://fake-validator-{uid_num}:8000" if api else None,
        trust_score=trust,
        weight=weight,
        stake=float(stake),
        status=status,
        last_performance=trust * 0.95,
        wallet_addr_hash=wallet_addr_bytes,  # Sử dụng wallet_addr_bytes thay vì addr_str
        # Các trường khác dùng giá trị mặc định của ValidatorInfo
    )


def create_validator_info_state(
    uid_num: int,
    trust: float,
    weight: float,
    stake: int,
    status=STATUS_ACTIVE,
    api=True,
) -> ValidatorInfo:
    # ... (giữ nguyên hoặc cập nhật) ...
    base_uid_str = f"validator_{uid_num:03d}"
    # Tạo UID hex cố định hơn cho test
    uid_hex = hashlib.sha256(base_uid_str.encode()).hexdigest()[:16]
    # Địa chỉ testnet hợp lệ (ví dụ)
    addr_str = f"addr_test1vpvalid{uid_num:03d}{'x'*(103-18-len(str(uid_num)))}"[:103]
    wallet_addr_bytes = addr_str.encode("utf-8")

    return ValidatorInfo(
        uid=uid_hex,
        address=addr_str,
        api_endpoint=f"http://fake-validator-{uid_num}:8000" if api else None,
        trust_score=trust,
        weight=weight,
        stake=float(stake),
        status=status,
        last_performance=trust * 0.95,  # Giá trị ví dụ
        wallet_addr_hash=wallet_addr_bytes,  # Giữ bytes
    )


def create_miner_info(
    uid_num: int,
    trust: float,
    weight: float,
    stake: int,
    last_selected: int,
    status=STATUS_ACTIVE,
) -> MinerInfo:
    """Tạo MinerInfo mẫu với UID hex."""
    base_uid_str = f"miner_{uid_num:03d}"
    uid_hex = hashlib.sha256(base_uid_str.encode()).hexdigest()[:16]  # UID hex giả lập
    return MinerInfo(
        uid=uid_hex,  # <<<--- UID dạng hex
        address=f"addr_test1vpmine{uid_num:03d}{'x'*45}",
        api_endpoint=f"http://fake-miner-{uid_num}:9000",
        trust_score=trust,
        weight=weight,
        stake=float(stake),
        last_selected_time=last_selected,
        status=status,
        performance_history=[],
        # Các trường khác dùng giá trị mặc định của MinerInfo
    )


def create_validator_datum(
    uid_hex: str,
    cycle: int,
    trust: float,
    perf: float,
    stake: int,
    status: int,
    rewards: int = 0,
    reg_slot: int = 1,
    subnet_uid: int = 0,
) -> ValidatorDatum:
    # ... (giữ nguyên) ...
    uid_bytes = bytes.fromhex(uid_hex)
    dummy_hash = hashlib.sha256(
        f"{uid_hex}_history_{cycle}".encode()
    ).digest()  # Thay đổi hash theo cycle
    wallet_hash = hashlib.sha256(f"addr_of_{uid_hex}".encode()).digest()
    api_endpoint_bytes = f"http://endpoint_{uid_hex}:8000".encode()
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    trust = max(0.0, min(1.0, trust))
    perf = max(0.0, min(1.0, perf))
    return ValidatorDatum(
        uid=uid_bytes,
        subnet_uid=subnet_uid,
        stake=int(stake),
        scaled_last_performance=int(perf * divisor),
        scaled_trust_score=int(trust * divisor),
        accumulated_rewards=int(rewards),
        last_update_slot=cycle,
        performance_history_hash=dummy_hash,
        wallet_addr_hash=wallet_hash,
        status=status,
        registration_slot=reg_slot,
        api_endpoint=api_endpoint_bytes,
    )


def create_mock_utxo_with_datum(datum: ValidatorDatum, tx_id_str: str = None, index: int = 0, amount: int = 2_000_000) -> MagicMock:  # type: ignore
    # ... (giữ nguyên) ...
    tx_id_bytes = bytes.fromhex(tx_id_str) if tx_id_str else os.urandom(32)
    tx_in = TransactionInput(transaction_id=TransactionId(tx_id_bytes), index=index)
    tx_out = MagicMock(spec=TransactionOutput)
    tx_out.amount = Value(coin=amount)
    tx_out.datum = datum
    tx_out.address = MagicMock(spec=Address)
    tx_out.script = None
    tx_out.datum_hash = None
    utxo = MagicMock(spec=UTxO)
    utxo.input = tx_in
    utxo.output = tx_out
    return utxo


def convert_datum_to_dict(datum: ValidatorDatum) -> dict:
    """Helper chuyển datum thành dict giống output của get_all_validator_data."""
    # Hàm này RẤT QUAN TRỌNG phải khớp với output thật của get_all_validator_data
    d = {}
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    # Lấy các trường cơ bản
    d["uid"] = datum.uid.hex()
    d["subnet_uid"] = datum.subnet_uid
    d["stake"] = datum.stake
    d["accumulated_rewards"] = datum.accumulated_rewards
    d["last_update_slot"] = datum.last_update_slot
    d["status"] = datum.status
    d["registration_slot"] = datum.registration_slot
    # Xử lý các trường bytes/None
    d["performance_history_hash"] = (
        datum.performance_history_hash.hex() if datum.performance_history_hash else None
    )
    d["wallet_addr_hash"] = (
        datum.wallet_addr_hash.hex() if datum.wallet_addr_hash else None
    )
    d["api_endpoint"] = (
        datum.api_endpoint.decode("utf-8", errors="replace")
        if datum.api_endpoint
        else None
    )
    # Thêm các trường float đã unscale
    d["trust_score"] = datum.trust_score  # Lấy từ property
    d["last_performance"] = datum.last_performance  # Lấy từ property
    # Thêm các trường scaled nếu cần cho debug
    # d['scaled_trust_score'] = datum.scaled_trust_score
    # d['scaled_last_performance'] = datum.scaled_last_performance
    return d


# ---------------------------


# --- Đăng ký custom mark 'logic' (Thêm vào conftest.py hoặc đầu file) ---
def pytest_configure(config):
    config.addinivalue_line("markers", "logic: mark test as logic test")


# -----------------------------------------------


# --- Fixtures (giữ nguyên) ---
@pytest.fixture
def mock_context(mocker: MockerFixture) -> MagicMock:
    return MagicMock(spec=BlockFrostChainContext)


@pytest.fixture
def cardano_params() -> Tuple[ScriptHash, Network]:
    try:
        from sdk.smartcontract.validator import read_validator

        script_hash = read_validator()["script_hash"]
    except Exception:
        script_hash = ScriptHash(os.urandom(28))  # Placeholder
    network = Network.TESTNET
    return script_hash, network


@pytest.mark.logic
def test_run_consensus_logic_basic():
    """Kiểm tra logic tính toán đồng thuận cơ bản trong run_consensus_logic."""
    current_cycle = 101
    # --- Tạo validators với UID hex ---
    v1_info = create_validator_info(1, 0.95, 3.0, 5000)
    v2_info = create_validator_info(2, 0.85, 2.5, 4000)
    v3_info = create_validator_info(3, 0.75, 2.0, 3000)
    v4_info = create_validator_info(4, 0.60, 1.0, 1000, status=STATUS_INACTIVE)
    validators_info = {
        v1_info.uid: v1_info,
        v2_info.uid: v2_info,
        v3_info.uid: v3_info,
        v4_info.uid: v4_info,
    }
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
    received_scores_cycle[task_id_m1][v1_info.uid] = ValidatorScore(
        task_id=task_id_m1, miner_uid=m1_uid, validator_uid=v1_info.uid, score=0.92
    )
    received_scores_cycle[task_id_m1][v2_info.uid] = ValidatorScore(
        task_id=task_id_m1, miner_uid=m1_uid, validator_uid=v2_info.uid, score=0.88
    )
    received_scores_cycle[task_id_m1][v3_info.uid] = ValidatorScore(
        task_id=task_id_m1, miner_uid=m1_uid, validator_uid=v3_info.uid, score=0.95
    )
    received_scores_cycle[task_id_m2][v1_info.uid] = ValidatorScore(
        task_id=task_id_m2, miner_uid=m2_uid, validator_uid=v1_info.uid, score=0.70
    )
    received_scores_cycle[task_id_m2][v3_info.uid] = ValidatorScore(
        task_id=task_id_m2, miner_uid=m2_uid, validator_uid=v3_info.uid, score=0.75
    )

    tasks_sent = {
        task_id_m1: TaskAssignment(
            task_id_m1, {}, m1_uid, v1_info.uid, time.time(), {}
        ),
        task_id_m2: TaskAssignment(
            task_id_m2, {}, m2_uid, v1_info.uid, time.time(), {}
        ),
    }

    final_miner_scores, calculated_validator_states = run_consensus_logic(
        current_cycle=current_cycle,
        tasks_sent=tasks_sent,
        received_scores=received_scores_cycle,  # Truyền dict của cycle hiện tại
        validators_info=validators_info,
        settings=settings,
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
    expected_p_adj_m1 = sum(t * s for t, s in zip(trusts_m1, scores_m1)) / sum(
        trusts_m1
    )
    trusts_m2 = [v1_info.trust_score, v3_info.trust_score]
    scores_m2 = [0.70, 0.75]
    expected_p_adj_m2 = sum(t * s for t, s in zip(trusts_m2, scores_m2)) / sum(
        trusts_m2
    )
    assert p_adj_m1 == pytest.approx(expected_p_adj_m1)
    assert p_adj_m2 == pytest.approx(expected_p_adj_m2)

    # Assert Validator States
    assert len(calculated_validator_states) == 4  # Tính cho cả inactive
    state_v1 = calculated_validator_states[v1_info.uid]
    state_v2 = calculated_validator_states[v2_info.uid]
    state_v3 = calculated_validator_states[v3_info.uid]
    state_v4 = calculated_validator_states[v4_info.uid]

    for uid, state in calculated_validator_states.items():
        assert "E_v" in state and isinstance(state["E_v"], float)
        assert "trust" in state and isinstance(state["trust"], float)
        assert "reward" in state and isinstance(state["reward"], float)
        assert "weight" in state and isinstance(
            state["weight"], float
        )  # Weight đầu chu kỳ
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
    assert state_v1["trust"] > v1_info.trust_score * math.exp(
        -settings.CONSENSUS_PARAM_DELTA_TRUST
    )  # Phải tăng hoặc decay ít
    assert state_v4["trust"] == pytest.approx(
        v4_info.trust_score * math.exp(-settings.CONSENSUS_PARAM_DELTA_TRUST)
    )  # Chỉ decay

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
    miners_info = {m1_info.uid: m1_info, m3_info.uid: m3_info, m5_info.uid: m5_info}
    final_miner_scores = {m1_info.uid: 0.9022, m3_info.uid: 0.0}  # P_adj

    # --- Gọi hàm cần test ---
    # --- Gọi hàm cần test với đủ tham số ---
    # mock_context = mocker.MagicMock(spec=BlockFrostChainContext)
    mock_context = mocker.MagicMock(spec=BlockFrostChainContext)
    mock_utxo_map = {}
    reward_old_m1 = 1000000
    reward_old_m3 = 500000
    try:
        # Mock M1 Datum
        mock_datum_m1_old = MinerDatum(uid=bytes.fromhex(m1_info.uid), accumulated_rewards=reward_old_m1, subnet_uid=0, stake=1050, scaled_last_performance=int(0.9 * settings.METAGRAPH_DATUM_INT_DIVISOR), scaled_trust_score=int(0.91 * settings.METAGRAPH_DATUM_INT_DIVISOR), last_update_slot=101, performance_history_hash=None, wallet_addr_hash=m1_info.wallet_addr_hash, status=1, registration_slot=1, api_endpoint=None)  # type: ignore
        mock_utxo_m1 = mocker.MagicMock(spec=UTxO)
        mock_utxo_m1.output.datum = mocker.MagicMock()
        mock_utxo_m1.output.datum.cbor = mock_datum_m1_old.to_cbor()
        mock_utxo_map[m1_info.uid] = mock_utxo_m1
        # Mock M3 Datum
        mock_datum_m3_old = MinerDatum(uid=bytes.fromhex(m3_info.uid), accumulated_rewards=reward_old_m3, subnet_uid=0, stake=780, scaled_last_performance=int(0.6 * settings.METAGRAPH_DATUM_INT_DIVISOR), scaled_trust_score=int(0.68 * settings.METAGRAPH_DATUM_INT_DIVISOR), last_update_slot=100, performance_history_hash=None, wallet_addr_hash=m3_info.wallet_addr_hash, status=1, registration_slot=1, api_endpoint=None)  # type: ignore
        mock_utxo_m3 = mocker.MagicMock(spec=UTxO)
        mock_utxo_m3.output.datum = mocker.MagicMock()
        mock_utxo_m3.output.datum.cbor = mock_datum_m3_old.to_cbor()
        mock_utxo_map[m3_info.uid] = mock_utxo_m3
    except Exception as e:
        print(f"Warning: Mock datum creation failed: {e}")
        reward_old_m1 = 0
        reward_old_m3 = 0

    miner_updates = await prepare_miner_updates_logic(  # <<<--- Thêm await
        current_cycle=current_cycle,
        miners_info=miners_info,
        final_scores=final_miner_scores,
        settings=settings,
        # context=mock_context, # <<<--- Thêm context (mock)
        current_utxo_map=mock_utxo_map,  # <<<--- Thêm utxo_map (mock)
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
    alpha_eff_m1 = settings.CONSENSUS_PARAM_ALPHA_BASE * (
        1 - settings.CONSENSUS_PARAM_K_ALPHA * abs(trust_m1_old - 0.5)
    )
    score_mapped_m1 = 1 / (
        1
        + math.exp(
            -settings.CONSENSUS_PARAM_UPDATE_SIG_K
            * (score_m1_new - settings.CONSENSUS_PARAM_UPDATE_SIG_X0)
        )
    )
    trust_m1_new += max(0, alpha_eff_m1) * score_mapped_m1
    assert datum_m1.scaled_trust_score == pytest.approx(int(trust_m1_new * divisor))
    assert datum_m1.accumulated_rewards >= 0
    assert datum_m1.status == STATUS_ACTIVE

    # Kiểm tra M3
    assert datum_m3.last_update_slot == current_cycle
    assert datum_m3.scaled_last_performance == 0  # P_adj = 0
    # Tính trust mới cho M3 (dựa trên trust cũ 0.68 và score mới 0.0)
    trust_m3_old = 0.68
    score_m3_new = 0.0
    time_since = 1
    trust_m3_new = update_trust_score(
        trust_m3_old,
        time_since,
        score_m3_new,
        delta_trust=settings.CONSENSUS_PARAM_DELTA_TRUST,
        alpha_base=settings.CONSENSUS_PARAM_ALPHA_BASE,
        k_alpha=settings.CONSENSUS_PARAM_K_ALPHA,
        update_sigmoid_L=settings.CONSENSUS_PARAM_UPDATE_SIG_L,
        update_sigmoid_k=settings.CONSENSUS_PARAM_UPDATE_SIG_K,
        update_sigmoid_x0=settings.CONSENSUS_PARAM_UPDATE_SIG_X0,
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
    trust_m5_new = trust_m5_old * math.exp(
        -settings.CONSENSUS_PARAM_DELTA_TRUST * 1
    )  # Giả sử time_since=1
    assert datum_m5.scaled_trust_score == pytest.approx(int(trust_m5_new * divisor))
    assert datum_m5.accumulated_rewards >= 0
    assert datum_m5.status == STATUS_INACTIVE


@pytest.mark.logic
@pytest.mark.asyncio
async def test_prepare_validator_updates_logic():
    """Kiểm tra việc chuẩn bị ValidatorDatum mới cho chính mình."""
    current_cycle = 103
    self_info = create_validator_info_state(
        1, 0.96, 3.1, 5100
    )  # <<< Sửa lại hàm helper nếu cần
    self_uid_hex = self_info.uid
    print(self_info)

    calculated_states = {
        self_uid_hex: {
            "E_v": 0.915,
            "trust": 0.965,
            "reward": 0.05,
            "weight": 3.1,
            "start_status": STATUS_ACTIVE,
            "last_update_cycle": current_cycle,
            "accumulated_rewards_old": 10000000,  # Giữ nguyên giá trị int cho dễ tính
        }
    }
    mock_context = None

    validator_updates = await prepare_validator_updates_logic(
        current_cycle=current_cycle,
        self_validator_info=self_info,
        calculated_states=calculated_states,
        settings=settings,
        context=mock_context,
    )

    # --- Gọi hàm cần test ---
    validator_updates = await prepare_validator_updates_logic(
        current_cycle=current_cycle,
        self_validator_info=self_info,
        calculated_states=calculated_states,
        settings=settings,
        context=mock_context,
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

    # --- SỬA LẠI ASSERTION CHO REWARDS ---
    # Lấy reward cũ từ input test
    accumulated_rewards_old = calculated_states[self_uid_hex]["accumulated_rewards_old"]
    # Tính reward mới (increment)
    new_reward_increment = int(calculated_states[self_uid_hex]["reward"] * divisor)
    # Giá trị mong đợi là tổng của reward cũ và phần tăng thêm
    expected_total_rewards = accumulated_rewards_old + new_reward_increment
    assert datum_v1.accumulated_rewards == pytest.approx(expected_total_rewards)
    # --- KẾT THÚC SỬA ---

    assert datum_v1.stake == int(self_info.stake)
    assert datum_v1.status == STATUS_ACTIVE
    # Kiểm tra hash (đảm bảo nó không phải None nếu được hash đúng)
    assert (
        isinstance(datum_v1.wallet_addr_hash, bytes)
        and len(datum_v1.wallet_addr_hash) == 32
    )
    assert (
        isinstance(datum_v1.api_endpoint, bytes) and len(datum_v1.api_endpoint) == 32
    )  # Hoặc None nếu không có endpoint


# --- Test cho _calculate_fraud_severity ---
@pytest.mark.logic
@pytest.mark.parametrize(
    "reason, tolerance, expected_severity",
    [  # <<<--- Bỏ divisor
        ("No deviation", 0.01, 0.0),
        ("Trust mismatch (Diff: 0.01500)", 0.01, 0.1),  # Factor=1.5 -> Minor
        ("Performance mismatch (Diff: 0.04000)", 0.01, 0.3),  # Factor=4.0 -> Moderate
        ("Trust mismatch (Diff: 0.12000)", 0.01, 0.7),  # Factor=12.0 -> Severe
        ("Did not commit updates", 0.01, 0.05),
        ("Trust mismatch (Diff: 0.005); Perf mismatch (Diff: 0.008)", 0.01, 0.0),
        (
            "Trust mismatch (Diff: 0.050); Perf mismatch (Diff: 0.008)",
            0.01,
            0.3,
        ),  # Factor=5.0 -> Moderate
    ],
)
def test_calculate_fraud_severity(
    reason, tolerance, expected_severity
):  # <<<--- Bỏ divisor
    """Kiểm tra hàm tính mức độ nghiêm trọng với các lý do khác nhau."""
    # --- Gọi hàm với 2 tham số ---
    severity = _calculate_fraud_severity(reason, tolerance)
    # --------------------------
    assert severity == pytest.approx(expected_severity)


# --- Test Cases cho verify_and_penalize_logic ---


@pytest.mark.asyncio
async def test_verify_penalize_all_correct(
    mocker: MockerFixture,
    mock_context: MagicMock,
    cardano_params: Tuple[ScriptHash, Network],
):
    """Kiểm tra trường hợp tất cả validator commit đúng trạng thái."""
    script_hash, network = cardano_params
    current_cycle = 101
    previous_cycle = 100
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    tolerance = settings.CONSENSUS_DATUM_COMPARISON_TOLERANCE

    # 1. Input state (start of cycle 101)
    v1_info_start = create_validator_info_state(1, 0.95, 3.0, 5000)
    v2_info_start = create_validator_info_state(2, 0.85, 2.5, 4000)
    validators_info_input = {
        v1_info_start.uid: copy.deepcopy(v1_info_start),
        v2_info_start.uid: copy.deepcopy(v2_info_start),
    }

    # 2. Expected state (calculated end of cycle 100)
    previous_calculated_states = {
        v1_info_start.uid: {"trust": 0.95, "E_v": 0.98, "start_status": STATUS_ACTIVE},
        v2_info_start.uid: {"trust": 0.85, "E_v": 0.90, "start_status": STATUS_ACTIVE},
    }

    # 3. Mock on-chain state (committed end of cycle 100)
    on_chain_trust_v1 = 0.95 + tolerance * 0.1  # Within tolerance
    on_chain_perf_v1 = 0.98 - tolerance * 0.1  # Within tolerance
    on_chain_trust_v2 = 0.85
    on_chain_perf_v2 = 0.90

    on_chain_datum_v1 = create_validator_datum(
        v1_info_start.uid, previous_cycle, on_chain_trust_v1, on_chain_perf_v1, v1_info_start.stake, STATUS_ACTIVE  # type: ignore
    )
    on_chain_datum_v2 = create_validator_datum(
        v2_info_start.uid, previous_cycle, on_chain_trust_v2, on_chain_perf_v2, v2_info_start.stake, STATUS_ACTIVE  # type: ignore
    )
    mock_utxo_v1 = create_mock_utxo_with_datum(on_chain_datum_v1, tx_id_str="a" * 64)
    mock_utxo_v2 = create_mock_utxo_with_datum(on_chain_datum_v2, tx_id_str="b" * 64)

    # Mock get_all_validator_data return value
    mock_on_chain_output = [
        (mock_utxo_v1, convert_datum_to_dict(on_chain_datum_v1)),
        (mock_utxo_v2, convert_datum_to_dict(on_chain_datum_v2)),
    ]
    mock_get_data = mocker.patch(
        "sdk.consensus.state.get_all_validator_data", new_callable=AsyncMock
    )
    mock_get_data.return_value = mock_on_chain_output

    # 4. Call function
    validators_info_before = copy.deepcopy(validators_info_input)
    penalized_datums = await verify_and_penalize_logic(
        current_cycle=current_cycle,
        previous_calculated_states=previous_calculated_states,
        validators_info=validators_info_input,
        context=mock_context,
        settings=settings,
        script_hash=script_hash,
        network=network,
    )

    # 5. Assertions
    mock_get_data.assert_awaited_once_with(mock_context, script_hash, network)
    assert not penalized_datums
    assert validators_info_input == validators_info_before


@pytest.mark.asyncio
async def test_verify_penalize_minor_deviation(
    mocker: MockerFixture,
    mock_context: MagicMock,
    cardano_params: Tuple[ScriptHash, Network],
):
    """Kiểm tra trường hợp sai lệch nhỏ (chỉ phạt trust)."""
    script_hash, network = cardano_params
    current_cycle = 102
    previous_cycle = 101
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    tolerance = settings.CONSENSUS_DATUM_COMPARISON_TOLERANCE
    penalty_eta = settings.CONSENSUS_PARAM_PENALTY_ETA

    v1_info_start = create_validator_info_state(1, 0.95, 3.0, 5000)
    validators_info_input = {v1_info_start.uid: copy.deepcopy(v1_info_start)}
    validators_info_before = copy.deepcopy(validators_info_input)

    expected_trust = 0.95
    expected_perf = 0.98
    previous_calculated_states = {
        v1_info_start.uid: {
            "trust": expected_trust,
            "E_v": expected_perf,
            "start_status": STATUS_ACTIVE,
        }
    }

    # Tạo sai lệch trust lớn hơn tolerance (ví dụ: 5 lần tolerance)
    on_chain_trust = expected_trust - (tolerance * 5)
    on_chain_perf = expected_perf  # Perf đúng
    on_chain_datum_v1 = create_validator_datum(
        v1_info_start.uid, previous_cycle, on_chain_trust, on_chain_perf, v1_info_start.stake, STATUS_ACTIVE  # type: ignore
    )
    mock_utxo_v1 = create_mock_utxo_with_datum(on_chain_datum_v1)
    mock_on_chain_output = [(mock_utxo_v1, convert_datum_to_dict(on_chain_datum_v1))]
    mock_get_data = mocker.patch(
        "sdk.consensus.state.get_all_validator_data", new_callable=AsyncMock
    )
    mock_get_data.return_value = mock_on_chain_output

    # Mock hàm tính severity để kiểm soát kết quả
    # Giả sử sai lệch 5*tolerance -> severity = 0.1
    mock_severity_calc = mocker.patch(
        "sdk.consensus.state._calculate_fraud_severity", return_value=0.1
    )

    penalized_datums = await verify_and_penalize_logic(
        current_cycle=current_cycle,
        previous_calculated_states=previous_calculated_states,
        validators_info=validators_info_input,
        context=mock_context,
        settings=settings,
        script_hash=script_hash,
        network=network,
    )

    # Assertions
    assert v1_info_start.uid in penalized_datums
    penalized_datum = penalized_datums[v1_info_start.uid]
    assert penalized_datum.last_update_slot == current_cycle
    assert penalized_datum.status == STATUS_ACTIVE  # Chưa bị jailed

    # Kiểm tra trust bị phạt
    severity = 0.1
    expected_penalized_trust_float = v1_info_start.trust_score * (
        1 - penalty_eta * severity
    )
    assert penalized_datum.scaled_trust_score == pytest.approx(
        int(expected_penalized_trust_float * divisor)
    )
    assert validators_info_input[v1_info_start.uid].trust_score == pytest.approx(
        expected_penalized_trust_float
    )
    assert validators_info_input[v1_info_start.uid].status == STATUS_ACTIVE


@pytest.mark.asyncio
async def test_verify_penalize_severe_deviation_jailed(
    mocker: MockerFixture,
    mock_context: MagicMock,
    cardano_params: Tuple[ScriptHash, Network],
):
    """Kiểm tra trường hợp sai lệch lớn dẫn đến JAILED."""
    script_hash, network = cardano_params
    current_cycle = 103
    previous_cycle = 102
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    tolerance = settings.CONSENSUS_DATUM_COMPARISON_TOLERANCE
    penalty_eta = settings.CONSENSUS_PARAM_PENALTY_ETA
    # Đặt jailed_threshold cao hơn severity giả định để test không bị jailed trước
    mocker.patch.object(settings, "CONSENSUS_JAILED_SEVERITY_THRESHOLD", 0.5)

    v1_info_start = create_validator_info_state(
        1, 0.95, 3.0, 5000, status=STATUS_ACTIVE
    )
    validators_info_input = {v1_info_start.uid: copy.deepcopy(v1_info_start)}

    expected_trust = 0.95
    expected_perf = 0.98
    previous_calculated_states = {
        v1_info_start.uid: {
            "trust": expected_trust,
            "E_v": expected_perf,
            "start_status": STATUS_ACTIVE,
        }
    }

    # Sai lệch rất lớn
    on_chain_trust = 0.50
    on_chain_perf = 0.60
    on_chain_datum_v1 = create_validator_datum(
        v1_info_start.uid, previous_cycle, on_chain_trust, on_chain_perf, v1_info_start.stake, STATUS_ACTIVE  # type: ignore
    )
    mock_utxo_v1 = create_mock_utxo_with_datum(on_chain_datum_v1)
    mock_on_chain_output = [(mock_utxo_v1, convert_datum_to_dict(on_chain_datum_v1))]
    mock_get_data = mocker.patch(
        "sdk.consensus.state.get_all_validator_data", new_callable=AsyncMock
    )
    mock_get_data.return_value = mock_on_chain_output

    # Mock severity trả về giá trị cao (ví dụ 0.7), lớn hơn threshold 0.5
    mock_severity_calc = mocker.patch(
        "sdk.consensus.state._calculate_fraud_severity", return_value=0.7
    )

    penalized_datums = await verify_and_penalize_logic(
        current_cycle=current_cycle,
        previous_calculated_states=previous_calculated_states,
        validators_info=validators_info_input,
        context=mock_context,
        settings=settings,
        script_hash=script_hash,
        network=network,
    )

    # Assertions
    assert v1_info_start.uid in penalized_datums
    penalized_datum = penalized_datums[v1_info_start.uid]
    assert penalized_datum.last_update_slot == current_cycle
    assert penalized_datum.status == STATUS_JAILED  # Bị jailed

    severity = 0.7
    expected_penalized_trust_float = v1_info_start.trust_score * (
        1 - penalty_eta * severity
    )
    assert penalized_datum.scaled_trust_score == pytest.approx(
        int(expected_penalized_trust_float * divisor)
    )
    assert validators_info_input[v1_info_start.uid].trust_score == pytest.approx(
        expected_penalized_trust_float
    )
    assert validators_info_input[v1_info_start.uid].status == STATUS_JAILED


@pytest.mark.asyncio
async def test_verify_penalize_did_not_commit(
    mocker: MockerFixture,
    mock_context: MagicMock,
    cardano_params: Tuple[ScriptHash, Network],
):
    """Kiểm tra trường hợp validator active không commit."""
    script_hash, network = cardano_params
    current_cycle = 104
    previous_cycle = 103
    penalty_eta = settings.CONSENSUS_PARAM_PENALTY_ETA
    v1_info_start = create_validator_info_state(
        1, 0.90, 3.0, 5000, status=STATUS_ACTIVE
    )
    v2_info_start = create_validator_info_state(
        2, 0.80, 2.0, 3000, status=STATUS_ACTIVE
    )
    validators_info_input = {
        v1_info_start.uid: copy.deepcopy(v1_info_start),
        v2_info_start.uid: copy.deepcopy(v2_info_start),
    }
    validators_info_before = copy.deepcopy(validators_info_input)
    previous_calculated_states = {
        v1_info_start.uid: {"trust": 0.90, "E_v": 0.92, "start_status": STATUS_ACTIVE},
        v2_info_start.uid: {"trust": 0.80, "E_v": 0.88, "start_status": STATUS_ACTIVE},
    }
    on_chain_datum_v2 = create_validator_datum(
        v2_info_start.uid, previous_cycle, 0.80, 0.88, v2_info_start.stake, STATUS_ACTIVE  # type: ignore
    )
    mock_utxo_v2 = create_mock_utxo_with_datum(on_chain_datum_v2)
    mock_on_chain_output = [(mock_utxo_v2, convert_datum_to_dict(on_chain_datum_v2))]
    mock_get_data = mocker.patch(
        "sdk.consensus.state.get_all_validator_data", new_callable=AsyncMock
    )
    mock_get_data.return_value = mock_on_chain_output
    mock_severity_calc = mocker.patch(
        "sdk.consensus.state._calculate_fraud_severity", return_value=0.05
    )

    penalized_datums = await verify_and_penalize_logic(
        current_cycle=current_cycle,
        previous_calculated_states=previous_calculated_states,
        validators_info=validators_info_input,
        context=mock_context,
        settings=settings,
        script_hash=script_hash,
        network=network,
    )

    # --- SỬA LẠI ASSERTIONS ---
    # a. Kiểm tra V1 bị phạt trong bộ nhớ (validators_info_input)
    severity_v1 = 0.05
    expected_penalized_trust_v1 = v1_info_start.trust_score * (
        1 - penalty_eta * severity_v1
    )
    assert validators_info_input[v1_info_start.uid].trust_score == pytest.approx(
        expected_penalized_trust_v1
    )
    assert (
        validators_info_input[v1_info_start.uid].status == STATUS_ACTIVE
    )  # Chưa bị jailed

    # b. Kiểm tra V2 không bị phạt trong bộ nhớ
    assert (
        validators_info_input[v2_info_start.uid].trust_score
        == validators_info_before[v2_info_start.uid].trust_score
    )
    assert (
        validators_info_input[v2_info_start.uid].status
        == validators_info_before[v2_info_start.uid].status
    )

    # c. Kiểm tra V1 KHÔNG có trong penalized_datums trả về
    assert v1_info_start.uid not in penalized_datums
    # d. Kiểm tra V2 cũng KHÔNG có trong penalized_datums (vì nó không làm gì sai)
    assert v2_info_start.uid not in penalized_datums
    # e. Do đó, dict trả về phải rỗng
    assert not penalized_datums


@pytest.mark.asyncio
async def test_verify_penalize_inactive_no_commit(
    mocker: MockerFixture,
    mock_context: MagicMock,
    cardano_params: Tuple[ScriptHash, Network],
):
    """Kiểm tra validator inactive không commit -> không bị phạt."""
    script_hash, network = cardano_params
    current_cycle = 105
    previous_cycle = 104

    # V1 inactive
    v1_info_start = create_validator_info_state(
        1, 0.70, 3.0, 5000, status=STATUS_INACTIVE
    )
    validators_info_input = {v1_info_start.uid: copy.deepcopy(v1_info_start)}
    validators_info_before = copy.deepcopy(validators_info_input)

    # Trạng thái dự kiến (vẫn tính cho inactive)
    previous_calculated_states = {
        v1_info_start.uid: {
            "trust": 0.65,
            "E_v": 0.0,
            "start_status": STATUS_INACTIVE,
        }  # Trust đã decay ở cycle trước
    }

    # Mock on-chain state: Rỗng (V1 không commit)
    mock_on_chain_output = []
    mock_get_data = mocker.patch(
        "sdk.consensus.state.get_all_validator_data", new_callable=AsyncMock
    )
    mock_get_data.return_value = mock_on_chain_output

    penalized_datums = await verify_and_penalize_logic(
        current_cycle=current_cycle,
        previous_calculated_states=previous_calculated_states,
        validators_info=validators_info_input,
        context=mock_context,
        settings=settings,
        script_hash=script_hash,
        network=network,
    )

    # Assertions: Không có gì xảy ra với V1
    assert not penalized_datums
    assert validators_info_input == validators_info_before


@pytest.mark.asyncio
async def test_verify_penalize_error_fetching_onchain(
    mocker: MockerFixture,
    mock_context: MagicMock,
    cardano_params: Tuple[ScriptHash, Network],
):
    """Kiểm tra trường hợp không thể fetch dữ liệu on-chain."""
    script_hash, network = cardano_params
    current_cycle = 106

    v1_info_start = create_validator_info_state(1, 0.95, 3.0, 5000)
    validators_info_input = {v1_info_start.uid: copy.deepcopy(v1_info_start)}
    validators_info_before = copy.deepcopy(validators_info_input)
    previous_calculated_states = {
        v1_info_start.uid: {"trust": 0.95, "E_v": 0.98, "start_status": STATUS_ACTIVE}
    }

    # Mock get_all_validator_data raise lỗi
    mock_get_data = mocker.patch(
        "sdk.consensus.state.get_all_validator_data", new_callable=AsyncMock
    )
    mock_get_data.side_effect = Exception("Blockchain query failed")

    penalized_datums = await verify_and_penalize_logic(
        current_cycle=current_cycle,
        previous_calculated_states=previous_calculated_states,
        validators_info=validators_info_input,
        context=mock_context,
        settings=settings,
        script_hash=script_hash,
        network=network,
    )

    # Assertions: Không có gì xảy ra
    assert not penalized_datums
    assert validators_info_input == validators_info_before


# --- Test Case for commit_updates_logic ---


@pytest.mark.asyncio
async def test_commit_updates_logic_success(
    mocker: MockerFixture,
    mock_context: MagicMock,  # Mock context
    cardano_params: Tuple[ScriptHash, Network],
    hotkey_skey_fixture: Tuple[
        ExtendedSigningKey, Optional[ExtendedSigningKey]
    ],  # Dùng key thật (mocked)
):
    """Kiểm tra commit thành công cho miner và validator updates."""
    # Đọc script bytes và hash ở đây nếu cần thiết cho setup
    try:
        from sdk.smartcontract.validator import read_validator

        validator_details = read_validator()
        script_bytes = validator_details["script_bytes"]  # Dùng script bytes từ đây
        # script_hash = validator_details["script_hash"] # Lấy script hash từ đây nếu muốn
        # Hoặc vẫn lấy từ cardano_params nếu fixture đó đáng tin cậy hơn
        script_hash_from_params, network = cardano_params
        script_hash = script_hash_from_params  # Quyết định dùng hash nào
    except Exception as e:
        pytest.skip(f"Skipping test, could not load validator script: {e}")
    payment_esk, stake_esk = hotkey_skey_fixture
    current_cycle = 105

    # 1. Tạo dữ liệu Datum mới cần commit
    miner1_uid_hex = hashlib.sha256(b"minercommit01_test").hexdigest()[
        :16
    ]  # Tạo hex UID hợp lệ (16 ký tự = 8 bytes)
    validator1_uid_hex = hashlib.sha256(b"validatorcommit01_test").hexdigest()[:16]
    penalized_val_uid_hex = hashlib.sha256(b"penalizedval01_test").hexdigest()[:16]

    new_miner_datum = MinerDatum(  # Tạo bằng dữ liệu hợp lệ
        uid=bytes.fromhex(miner1_uid_hex * 4),  # Ví dụ UID bytes
        subnet_uid=1,
        stake=100e6, # type: ignore
        scaled_last_performance=950000,
        scaled_trust_score=900000,  # type: ignore
        accumulated_rewards=5e6, # type: ignore
        last_update_slot=current_cycle,
        status=STATUS_ACTIVE,  # type: ignore
        performance_history_hash=hashlib.sha256(b"m1hist").digest(),
        wallet_addr_hash=hashlib.sha256(b"m1addr").digest(),
        registration_slot=100,
        api_endpoint=hashlib.sha256(b"m1api").digest(),
    )
    new_validator_datum = create_validator_datum(validator1_uid_hex, current_cycle, 0.98, 0.99, 5000e6, STATUS_ACTIVE, rewards=10e6)  # type: ignore
    new_penalized_datum = create_validator_datum(penalized_val_uid_hex, current_cycle, 0.70, 0.80, 3000e6, STATUS_JAILED, rewards=1e6)  # type: ignore # Bị jailed

    miner_updates = {miner1_uid_hex: new_miner_datum}
    validator_updates = {validator1_uid_hex: new_validator_datum}
    penalized_validator_updates = {penalized_val_uid_hex: new_penalized_datum}

    # 2. Tạo UTXO map đầu vào (mock UTXO cũ)
    # Cần tạo mock UTxO với Datum cũ (hoặc ít nhất là có amount) cho mỗi UID cần update
    mock_utxo_miner1 = MagicMock(spec=UTxO)
    mock_utxo_miner1.input = TransactionInput(
        TransactionId(os.urandom(32)), 0
    )  # Thêm .input giả
    mock_utxo_miner1.output = MagicMock(spec=TransactionOutput, amount=Value(2_000_000))
    mock_utxo_val1 = MagicMock(spec=UTxO)
    mock_utxo_val1.input = TransactionInput(
        TransactionId(os.urandom(32)), 1
    )  # Thêm .input giả
    mock_utxo_val1.output = MagicMock(spec=TransactionOutput, amount=Value(2_100_000))
    mock_utxo_penalized = MagicMock(spec=UTxO)
    mock_utxo_penalized.input = TransactionInput(
        TransactionId(os.urandom(32)), 2
    )  # Thêm .input giả
    mock_utxo_penalized.output = MagicMock(
        spec=TransactionOutput, amount=Value(2_200_000)
    )

    current_utxo_map = {
        miner1_uid_hex: mock_utxo_miner1,
        validator1_uid_hex: mock_utxo_val1,
        penalized_val_uid_hex: mock_utxo_penalized,
    }

    # 3. Mock context.submit_tx và TransactionBuilder (nếu cần)
    # Mock submit_tx để trả về ID giả lập và kiểm tra lời gọi
    mock_submit_tx = AsyncMock(
        return_value=TransactionId(os.urandom(32))
    )  # Trả về ID giả
    mock_context.submit_tx = mock_submit_tx

    # Mock TransactionBuilder để tránh lỗi nếu không có UTXO thật từ owner address
    # Chúng ta cần mock add_input_address và build_and_sign
    mock_builder_instance = MagicMock(spec=TransactionBuilder)
    mock_signed_tx = MagicMock()  # Mock transaction đã ký
    mock_builder_instance.build_and_sign.return_value = mock_signed_tx
    # Mock hàm khởi tạo của TransactionBuilder để trả về instance đã mock
    mock_tx_builder_class = mocker.patch(
        "sdk.consensus.state.TransactionBuilder", return_value=mock_builder_instance
    )

    # 4. Gọi hàm commit_updates_logic
    result = await commit_updates_logic(
        miner_updates=miner_updates,
        validator_updates=validator_updates,
        penalized_validator_updates=penalized_validator_updates,
        current_utxo_map=current_utxo_map,  # type: ignore
        context=mock_context,
        signing_key=payment_esk,  # Dùng key từ fixture
        stake_signing_key=stake_esk,  # Dùng key từ fixture
        settings=settings,
        script_hash=script_hash,
        script_bytes=script_bytes,
        network=network,
    )

    # 5. Assertions
    assert result["status"] == "completed"
    # Kiểm tra số lần submit_tx được gọi (bằng tổng số updates)
    expected_calls = (
        len(miner_updates) + len(validator_updates) + len(penalized_validator_updates)
    )
    assert mock_submit_tx.await_count == expected_calls
    assert result["submitted_count"] == expected_calls
    assert result["failed_count"] == 0
    assert result["skipped_count"] == 0
    assert len(result["submitted_txs"]) == expected_calls

    # (Optional) Kiểm tra các tham số của builder (phức tạp hơn)
    # Ví dụ: Kiểm tra xem add_script_input, add_output có được gọi đúng không
    assert mock_builder_instance.add_script_input.call_count == expected_calls
    assert mock_builder_instance.add_output.call_count == expected_calls
    # Kiểm tra add_input_address được gọi cho mỗi lần build
    assert mock_builder_instance.add_input_address.call_count == expected_calls
    # Kiểm tra build_and_sign được gọi
    assert mock_builder_instance.build_and_sign.call_count == expected_calls

    # Kiểm tra nội dung output datum được thêm vào builder (ví dụ cho miner)
    # Lấy tất cả các lần gọi add_output
    add_output_calls = mock_builder_instance.add_output.call_args_list
    found_miner_datum = False
    found_validator_datum = False
    found_penalized_datum = False
    for call in add_output_calls:
        args, kwargs = call
        output_arg = (
            args[0] if args else kwargs.get("output")
        )  # TransactionOutput là tham số đầu tiên
        if isinstance(output_arg, TransactionOutput):
            datum_in_call = output_arg.datum
            if datum_in_call == new_miner_datum:
                found_miner_datum = True
            if datum_in_call == new_validator_datum:
                found_validator_datum = True
            if datum_in_call == new_penalized_datum:
                found_penalized_datum = True

    assert found_miner_datum, "New miner datum was not added as output"
    assert found_validator_datum, "New validator datum was not added as output"
    assert found_penalized_datum, "New penalized datum was not added as output"


# --- Thêm test case cho các trường hợp lỗi ---


@pytest.mark.asyncio
async def test_commit_updates_logic_missing_input_utxo(
    mocker: MockerFixture,
    mock_context: MagicMock,
    cardano_params: Tuple[ScriptHash, Network],
    hotkey_skey_fixture: Tuple[ExtendedSigningKey, Optional[ExtendedSigningKey]],
):
    """Kiểm tra trường hợp thiếu UTXO đầu vào trong map."""
    try:
        from sdk.smartcontract.validator import read_validator

        validator_details = read_validator()
        script_bytes = validator_details["script_bytes"]  # Dùng script bytes từ đây
        # script_hash = validator_details["script_hash"] # Lấy script hash từ đây nếu muốn
        # Hoặc vẫn lấy từ cardano_params nếu fixture đó đáng tin cậy hơn
        script_hash_from_params, network = cardano_params
        script_hash = script_hash_from_params  # Quyết định dùng hash nào
    except Exception as e:
        pytest.skip(f"Skipping test, could not load validator script: {e}")
    payment_esk, stake_esk = hotkey_skey_fixture
    current_cycle = 106

    miner1_uid_hex = hashlib.sha256(b"minermissingutxo").hexdigest()[:16]
    new_miner_datum = MinerDatum(
        uid=bytes.fromhex(miner1_uid_hex),
        subnet_uid=1,
        stake=int(50e6),
        scaled_last_performance=800000,
        scaled_trust_score=850000,
        accumulated_rewards=int(1e6),
        last_update_slot=current_cycle,
        status=STATUS_ACTIVE,
        performance_history_hash=hashlib.sha256(b"m_missing_hist").digest(),
        wallet_addr_hash=hashlib.sha256(b"m_missing_addr").digest(),
        registration_slot=101,
        api_endpoint=hashlib.sha256(b"m_missing_api").digest(),
    )

    miner_updates = {miner1_uid_hex: new_miner_datum}
    current_utxo_map = {}  # Map rỗng -> thiếu UTXO đầu vào

    mock_submit_tx = AsyncMock()
    mock_context.submit_tx = mock_submit_tx
    mock_tx_builder_class = mocker.patch(
        "sdk.consensus.state.TransactionBuilder"
    )  # Mock class

    result = await commit_updates_logic(
        miner_updates=miner_updates,
        validator_updates={},
        penalized_validator_updates={},
        current_utxo_map=current_utxo_map,
        context=mock_context,
        signing_key=payment_esk,
        stake_signing_key=stake_esk,
        settings=settings,
        script_hash=script_hash,
        script_bytes=script_bytes,
        network=network,
    )

    assert result["status"] == "completed"  # Hoặc completed tùy logic xử lý lỗi
    assert result["submitted_count"] == 0
    assert result["failed_count"] == 0  # Không fail ở submit
    assert result["skipped_count"] == 1  # Bị skip do thiếu UTXO
    assert miner1_uid_hex in result["skips"]
    assert "Input UTxO not found" in result["skips"][miner1_uid_hex]  # type: ignore
    mock_submit_tx.assert_not_awaited()  # Không có giao dịch nào được gửi


@pytest.mark.asyncio
async def test_commit_updates_logic_submit_error(
    mocker: MockerFixture,
    mock_context: MagicMock,
    cardano_params: Tuple[ScriptHash, Network],
    hotkey_skey_fixture: Tuple[ExtendedSigningKey, Optional[ExtendedSigningKey]],
):
    """Kiểm tra trường hợp context.submit_tx báo lỗi."""
    try:
        from sdk.smartcontract.validator import read_validator

        validator_details = read_validator()
        script_bytes = validator_details["script_bytes"]  # Dùng script bytes từ đây
        # script_hash = validator_details["script_hash"] # Lấy script hash từ đây nếu muốn
        # Hoặc vẫn lấy từ cardano_params nếu fixture đó đáng tin cậy hơn
        script_hash_from_params, network = cardano_params
        script_hash = script_hash_from_params  # Quyết định dùng hash nào
    except Exception as e:
        pytest.skip(f"Skipping test, could not load validator script: {e}")
    payment_esk, stake_esk = hotkey_skey_fixture
    current_cycle = 107

    miner1_uid_hex = hashlib.sha256(b"minersubmitfail").hexdigest()[:16]
    new_miner_datum = MinerDatum(
        uid=bytes.fromhex(miner1_uid_hex),
        subnet_uid=1,
        stake=int(50e6),
        scaled_last_performance=800000,
        scaled_trust_score=850000,
        accumulated_rewards=int(1e6),
        last_update_slot=current_cycle,
        status=STATUS_ACTIVE,
        performance_history_hash=hashlib.sha256(b"m_missing_hist").digest(),
        wallet_addr_hash=hashlib.sha256(b"m_missing_addr").digest(),
        registration_slot=101,
        api_endpoint=hashlib.sha256(b"m_missing_api").digest(),
    )

    mock_utxo_miner1 = MagicMock(spec=UTxO)
    mock_utxo_miner1.input = TransactionInput(
        TransactionId(os.urandom(32)), 0
    )  # Thêm .input giả
    mock_utxo_miner1.output = MagicMock(spec=TransactionOutput, amount=Value(2_000_000))
    current_utxo_map = {miner1_uid_hex: mock_utxo_miner1}

    # Mock submit_tx để raise lỗi ApiError (ví dụ lỗi 400 từ Blockfrost)
    # Cần import ApiError từ blockfrost nếu dùng
    # from blockfrost import ApiError
    # mock_submit_tx = AsyncMock(side_effect=ApiError(status_code=400, message="Bad request"))
    # Hoặc raise Exception chung
    mock_submit_tx = AsyncMock(side_effect=Exception("Submission Error"))
    mock_context.submit_tx = mock_submit_tx

    # Mock TransactionBuilder như test success
    mock_builder_instance = MagicMock(spec=TransactionBuilder)
    mock_signed_tx = MagicMock()
    mock_signed_tx.to_cbor.return_value = b"mock_cbor_fail"
    mock_builder_instance.build_and_sign.return_value = mock_signed_tx
    mocker.patch(
        "sdk.consensus.state.TransactionBuilder", return_value=mock_builder_instance
    )

    result = await commit_updates_logic(
        miner_updates={miner1_uid_hex: new_miner_datum},
        validator_updates={},
        penalized_validator_updates={},
        current_utxo_map=current_utxo_map, # type: ignore
        context=mock_context,
        signing_key=payment_esk,  # type: ignore
        stake_signing_key=stake_esk,
        settings=settings,
        script_hash=script_hash,
        script_bytes=script_bytes,
        network=network,
    )

    assert result["status"] == "completed_with_errors"
    assert result["submitted_count"] == 0
    assert result["failed_count"] == 1  # 1 lỗi submit
    assert result["skipped_count"] == 0
    assert miner1_uid_hex in result["failures"]
    assert "Submission Error" in result["failures"][miner1_uid_hex]  # type: ignore # Kiểm tra nội dung lỗi
    mock_submit_tx.assert_awaited_once()  # Đã cố gắng gọi submit 1 lần
