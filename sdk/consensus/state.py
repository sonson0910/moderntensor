# sdk/consensus/state.py
"""
Logic tính toán đồng thuận, kiểm tra phạt, chuẩn bị và commit cập nhật trạng thái.
"""
import logging
import math
import asyncio
from typing import List, Dict, Any, Tuple, Optional, Set, Union
from collections import defaultdict

from sdk.config.settings import settings
from sdk.core.datatypes import MinerInfo, ValidatorInfo, ValidatorScore, TaskAssignment
from sdk.formulas import (
    calculate_adjusted_miner_performance,
    calculate_validator_performance,
    update_trust_score,
    calculate_fraud_severity_value, # Cần logic cụ thể
    calculate_slash_amount,
    calculate_miner_incentive,
    calculate_validator_incentive
    # Import các công thức khác nếu cần
)
from sdk.metagraph.update_metagraph import update_datum
from sdk.metagraph.metagraph_datum import MinerDatum, ValidatorDatum
from sdk.metagraph.metagraph_data import get_all_validator_data
# from sdk.metagraph.hash import hash_data # Cần hàm hash
def hash_data(data): return f"hashed_{str(data)[:10]}" # Mock hash
from pycardano import BlockFrostChainContext, PaymentSigningKey, StakeSigningKey, TransactionId, Network, ScriptHash, UTxO, Address, PlutusV3Script, Redeemer, VerificationKeyHash, PaymentVerificationKey, TransactionBuilder, Value, TransactionOutput
from sdk.metagraph.metagraph_datum import MinerDatum, ValidatorDatum, STATUS_ACTIVE, STATUS_JAILED, STATUS_INACTIVE
from blockfrost import ApiError

EPSILON=1e-9
logger = logging.getLogger(__name__)

# --- Logic Đồng thuận và Tính toán Trạng thái Validator ---

# --- Placeholder Function: Tìm UTXO theo UID ---
# Cần thay thế bằng logic thực tế, có thể cần sửa đổi get_all_miner/validator_data
async def find_utxo_by_uid(
    context: BlockFrostChainContext,
    script_hash: ScriptHash,
    network: Network,
    uid_bytes: bytes,
    datum_class: type # MinerDatum hoặc ValidatorDatum
) -> Optional[UTxO]:
    """
    (Placeholder/Mock) Tìm UTXO tại địa chỉ script chứa Datum có UID khớp.
    Trong thực tế, nên tối ưu hóa việc này.
    """
    logger.debug(f"Searching for UTxO with UID {uid_bytes.hex()} of type {datum_class.__name__}...")
    contract_address = Address(payment_part=script_hash, network=network)
    try:
        # TODO: Tối ưu hóa: chỉ fetch UTXO liên quan nếu có thể
        # Tạm thời fetch hết và lọc
        utxos = await context.utxos(str(contract_address))
        for utxo in utxos:
            if utxo.output.datum:
                try:
                    decoded_datum = datum_class.from_cbor(utxo.output.datum.cbor)
                    if hasattr(decoded_datum, 'uid') and getattr(decoded_datum, 'uid') == uid_bytes:
                        logger.debug(f"Found matching UTxO: {utxo.input}")
                        return utxo
                except Exception:
                    # logger.warning(f"Failed to decode datum for {utxo.input} as {datum_class.__name__}")
                    continue # Bỏ qua datum không decode được hoặc sai loại
    except Exception as e:
        logger.error(f"Failed to fetch UTxOs for {contract_address} while searching for UID {uid_bytes.hex()}: {e}")

    logger.warning(f"UTxO for UID {uid_bytes.hex()} of type {datum_class.__name__} not found.")
    return None
# -----------------------------------------

# --- Hàm tính Severity tinh chỉnh hơn ---
def _calculate_fraud_severity(reason: str, tolerance: float) -> float: # <<<--- Chỉ 2 tham số
    severity = 0.0
    max_deviation_factor = 0.0

    if "Did not commit" in reason: return 0.05

    parts = reason.split(';')
    for part in parts:
        part = part.strip()
        if "mismatch" in part:
            try:
                diff_str = part.split('Diff:')[-1].strip().rstrip(')')
                diff_float = float(diff_str)
                if tolerance > 1e-9:
                     deviation_factor = diff_float / tolerance
                     max_deviation_factor = max(max_deviation_factor, deviation_factor)
            except Exception: pass

    # --- Logic quyết định severity dựa trên factor và settings ---
    severe_threshold_factor = getattr(settings, 'CONSENSUS_SEVERITY_SEVERE_FACTOR', 10.0)
    moderate_threshold_factor = getattr(settings, 'CONSENSUS_SEVERITY_MODERATE_FACTOR', 3.0)

    if max_deviation_factor >= severe_threshold_factor:
        severity = 0.7 # Severe
    elif max_deviation_factor >= moderate_threshold_factor:
        severity = 0.3 # Moderate
    elif max_deviation_factor > 1.0: # Chỉ vượt tolerance một chút
        severity = 0.1 # Minor
    # else: severity = 0.0 (mặc định)

    logger.debug(f"Calculated fraud severity: {severity:.2f} (Factor: {max_deviation_factor:.1f}x)")
    return severity

# -----------------------------------------

def run_consensus_logic(
    current_cycle: int,
    tasks_sent: Dict[str, TaskAssignment],
    received_scores: Dict[str, Dict[str, ValidatorScore]], # {task_id: {validator_uid_hex: ValidatorScore}}
    validators_info: Dict[str, ValidatorInfo], # {validator_uid_hex: ValidatorInfo} - Trạng thái đầu chu kỳ
    settings: Any
) -> Tuple[Dict[str, float], Dict[str, Any]]:
    """
    Thực hiện đồng thuận điểm miners, tính toán trạng thái dự kiến VÀ phần thưởng dự kiến cho validators.
    """
    logger.info(f"Running consensus calculations for cycle {current_cycle}...")
    final_miner_scores: Dict[str, float] = {} # {miner_uid_hex: P_adj}
    validator_deviations: Dict[str, List[float]] = defaultdict(list) # {validator_uid_hex: [deviation1, deviation2,...]}
    calculated_validator_states: Dict[str, Any] = {} # {validator_uid_hex: {state}}
    total_validator_contribution: float = 0.0 # Tổng W*E để tính thưởng validator

    # --- 1. Tính điểm đồng thuận Miner (P_miner_adjusted) và độ lệch ---
    scores_by_miner: Dict[str, List[Tuple[float, float]]] = defaultdict(list) # {miner_uid_hex: [(score, validator_trust)]}
    tasks_processed_by_miner: Dict[str, Set[str]] = defaultdict(set) # {miner_uid_hex: {task_id1, task_id2,...}}
    validator_scores_by_task: Dict[str, Dict[str, float]] = defaultdict(dict) # {task_id: {validator_uid: score}}

    # Gom điểm theo miner VÀ theo task
    for task_id, validator_scores_dict in received_scores.items():
        first_score = next(iter(validator_scores_dict.values()), None)
        if not first_score: continue
        miner_uid_hex = first_score.miner_uid
        tasks_processed_by_miner[miner_uid_hex].add(task_id) # Lưu tất cả task miner đã làm

        for validator_uid_hex, score_entry in validator_scores_dict.items():
            validator = validators_info.get(validator_uid_hex)
            if validator and getattr(validator, 'status', STATUS_ACTIVE) == STATUS_ACTIVE: # Chỉ tính điểm từ validator active
                # Lưu (điểm, trust của validator chấm điểm) cho miner
                scores_by_miner[miner_uid_hex].append((score_entry.score, validator.trust_score))
                # Lưu điểm của validator cho task này
                validator_scores_by_task[task_id][validator_uid_hex] = score_entry.score
            # else: logger.warning(...) # Bỏ qua validator không tồn tại hoặc inactive

    # Tính P_adj và độ lệch
    for miner_uid_hex, scores_trusts in scores_by_miner.items():
        if not scores_trusts: continue
        scores = [s for s, t in scores_trusts]
        trusts = [t for s, t in scores_trusts]

        p_adj = calculate_adjusted_miner_performance(scores, trusts)
        final_miner_scores[miner_uid_hex] = p_adj
        logger.info(f"  Consensus score (P_adj) for Miner {miner_uid_hex}: {p_adj:.4f}")

        # Tính độ lệch cho từng validator đã chấm điểm miner này, trên từng task
        related_task_ids = tasks_processed_by_miner.get(miner_uid_hex, set())
        for task_id in related_task_ids:
            scores_for_this_task = validator_scores_by_task.get(task_id, {})
            for validator_uid_hex, score in scores_for_this_task.items():
                 # Độ lệch = |điểm validator chấm - điểm đồng thuận của miner|
                 deviation = abs(score - p_adj)
                 validator_deviations[validator_uid_hex].append(deviation)
                 logger.debug(f"  Deviation for V:{validator_uid_hex} on M:{miner_uid_hex} (Task:{task_id}): {deviation:.4f}")

    # --- 2. Tính E_validator, Trust mới dự kiến, và Đóng góp cho thưởng ---
    temp_validator_contributions: Dict[str, float] = {}

    # Cải thiện cách tính E_avg: Trung bình trọng số theo stake của các validator ACTIVE
    active_validators_info = {uid: v for uid, v in validators_info.items() if getattr(v, 'status', STATUS_ACTIVE) == STATUS_ACTIVE}
    total_active_stake = sum(v.stake for v in active_validators_info.values())
    e_avg_weighted = 0.0
    if total_active_stake > EPSILON:
         # Tính E_v trung bình dựa trên trạng thái *đầu chu kỳ* (last_performance từ ValidatorInfo)
         valid_e_validators_for_avg = [(v.stake, getattr(v, 'last_performance', 0.0)) for v in active_validators_info.values()]
         if valid_e_validators_for_avg:
              e_avg_weighted = sum(stake * perf for stake, perf in valid_e_validators_for_avg) / total_active_stake
    else:
         e_avg_weighted = 0.5 # Default nếu không có ai active hoặc stake=0

    logger.info(f"  Weighted E_avg (based on start-of-cycle active validator stake): {e_avg_weighted:.4f}")

    # Tính toán cho từng validator (kể cả inactive/jailed để có trạng thái dự kiến nếu họ quay lại)
    for validator_uid_hex, validator_info in validators_info.items():
        deviations = validator_deviations.get(validator_uid_hex, [])
        avg_dev = sum(deviations) / len(deviations) if deviations else 0.0

        # Nếu validator không chấm điểm nào thì avg_dev = 0.
        # Cân nhắc: Có nên phạt validator không tham gia chấm điểm không? (Hiện tại thì không)
        logger.debug(f"  Validator {validator_uid_hex}: Average deviation = {avg_dev:.4f} ({len(deviations)} scores evaluated)")

        # Metric Quality Placeholder
        metric_quality_example = max(0.0, 1.0 - avg_dev * 1.5)
        logger.debug(f"  Validator {validator_uid_hex}: Mock Metric Quality = {metric_quality_example:.3f}")

        # Q_task Placeholder (giả sử validator không làm task)
        q_task_val_example = 0.0

        # Tính E_validator mới
        new_e_validator = calculate_validator_performance(
            q_task_validator=q_task_val_example,
            metric_validator_quality=metric_quality_example,
            deviation=avg_dev, # Độ lệch trung bình của validator này
            theta1=settings.CONSENSUS_PARAM_THETA1,
            theta2=settings.CONSENSUS_PARAM_THETA2,
            theta3=settings.CONSENSUS_PARAM_THETA3,
            # Tham số Penalty Term lấy từ settings
            penalty_threshold_dev=settings.CONSENSUS_PARAM_PENALTY_THRESHOLD_DEV,
            penalty_k_penalty=settings.CONSENSUS_PARAM_PENALTY_K_PENALTY,
            penalty_p_penalty=settings.CONSENSUS_PARAM_PENALTY_P_PENALTY
        )
        logger.info(f"  Calculated performance (E_val) for Validator {validator_uid_hex}: {new_e_validator:.4f}")

        # Tính Trust Score mới dự kiến
        # Nếu validator không hoạt động (inactive/jailed), chỉ áp dụng suy giảm
        time_since_val_eval = 1 # Mặc định là 1 chu kỳ
        score_for_trust_update = 0.0
        if getattr(validator_info, 'status', STATUS_ACTIVE) == STATUS_ACTIVE:
            # Chỉ cập nhật trust dựa trên E_v mới nếu validator đang active
            score_for_trust_update = new_e_validator
        else:
            # Nếu không active, trust chỉ bị suy giảm (score_new = 0)
            # Có thể cần logic tính time_since phức tạp hơn nếu validator bị inactive/jailed lâu
            logger.debug(f"Validator {validator_uid_hex} is not active. Applying only trust decay.")

        new_val_trust_score = update_trust_score(
             validator_info.trust_score, # Trust score đầu chu kỳ
             time_since_val_eval,
             score_for_trust_update, # Dùng E_val mới tính nếu active, nếu không thì dùng 0
             delta_trust=settings.CONSENSUS_PARAM_DELTA_TRUST,
             alpha_base=settings.CONSENSUS_PARAM_ALPHA_BASE,
             k_alpha=settings.CONSENSUS_PARAM_K_ALPHA,
             update_sigmoid_L=settings.CONSENSUS_PARAM_UPDATE_SIG_L,
             update_sigmoid_k=settings.CONSENSUS_PARAM_UPDATE_SIG_K,
             update_sigmoid_x0=settings.CONSENSUS_PARAM_UPDATE_SIG_X0
        )
        logger.info(f"  Calculated next Trust for Validator {validator_uid_hex}: {new_val_trust_score:.4f}")

        # Tính đóng góp W*E cho việc tính thưởng (dùng weight đầu chu kỳ và E_v mới)
        current_weight = getattr(validator_info, 'weight', 0.0)
        # Chỉ validator active mới đóng góp vào việc chia thưởng
        contribution = 0.0
        if getattr(validator_info, 'status', STATUS_ACTIVE) == STATUS_ACTIVE:
            contribution = current_weight * new_e_validator
            temp_validator_contributions[validator_uid_hex] = contribution
            total_validator_contribution += contribution

        # Lưu trạng thái dự kiến (bao gồm cả E_v, trust cho validator inactive/jailed)
        calculated_validator_states[validator_uid_hex] = {
            "E_v": new_e_validator,
            "trust": new_val_trust_score, # Trust dự kiến cuối chu kỳ
            "weight": current_weight, # Weight đầu chu kỳ
            "contribution": contribution, # Đóng góp W*E (chỉ > 0 nếu active)
            "last_update_cycle": current_cycle,
            # Lưu thêm trạng thái đầu vào để tiện debug/kiểm tra
            "avg_deviation": avg_dev,
            "metric_quality": metric_quality_example,
            "start_trust": validator_info.trust_score,
            "start_status": getattr(validator_info, 'status', STATUS_ACTIVE),
        }

    # --- 3. Tính phần thưởng dự kiến cho từng validator (chỉ những ai active) ---
    logger.info(f"Total validator contribution (Sum W_current*E_new from Active): {total_validator_contribution:.4f}")
    if total_validator_contribution > EPSILON:
        for validator_uid_hex, state in calculated_validator_states.items():
            # Chỉ tính thưởng cho validator active
            if state.get("start_status") == STATUS_ACTIVE:
                trust_for_reward = state["start_trust"] # Dùng trust đầu chu kỳ
                reward = calculate_validator_incentive(
                    trust_score=trust_for_reward,
                    validator_weight=state["weight"], # Weight đầu chu kỳ
                    validator_performance=state["E_v"], # E_v mới tính
                    total_validator_value=total_validator_contribution, # Tổng contribution của những người active
                    incentive_sigmoid_L=settings.CONSENSUS_PARAM_INCENTIVE_SIG_L,
                    incentive_sigmoid_k=settings.CONSENSUS_PARAM_INCENTIVE_SIG_K,
                    incentive_sigmoid_x0=settings.CONSENSUS_PARAM_INCENTIVE_SIG_X0
                )
                state["reward"] = reward # Thêm phần thưởng vào trạng thái dự kiến
                logger.info(f"  Validator {validator_uid_hex}: Calculated Reward = {reward:.6f}")
            else:
                state["reward"] = 0.0 # Không có thưởng nếu không active
    else:
         logger.warning("Total active validator contribution is zero. No validator rewards calculated.")
         for state in calculated_validator_states.values():
             state["reward"] = 0.0

    logger.info("Finished consensus calculations and validator state estimation.")
    return final_miner_scores, calculated_validator_states


# --- Logic Kiểm tra và Phạt Validator ---

async def verify_and_penalize_logic(
    current_cycle: int,
    previous_calculated_states: Dict[str, Any], # State dự kiến cycle N-1
    validators_info: Dict[str, ValidatorInfo], # State hiện tại (đầu cycle N), sẽ bị sửa trực tiếp
    context: BlockFrostChainContext,
    settings: Any,
    script_hash: ScriptHash,
    network: Network,
    # Bỏ các key vì hàm này chỉ chuẩn bị datum phạt, không commit trực tiếp
    # signing_key: PaymentSigningKey,
    # stake_signing_key: Optional[StakeSigningKey]
) -> Dict[str, ValidatorDatum]:
    """
    Kiểm tra ValidatorDatum chu kỳ trước, áp dụng phạt trust/status vào validators_info
    và chuẩn bị ValidatorDatum mới để commit hình phạt đó.
    """
    logger.info(f"Verifying previous cycle ({current_cycle - 1}) validator updates...")
    previous_cycle = current_cycle - 1
    if previous_cycle < 0: return {}

    penalized_validator_datums: Dict[str, ValidatorDatum] = {}
    tolerance = settings.CONSENSUS_DATUM_COMPARISON_TOLERANCE # Sai số cho phép (float)
    # Chuyển tolerance sang dạng int để so sánh với scaled values
    scaled_tolerance = int(tolerance * settings.METAGRAPH_DATUM_INT_DIVISOR)
    logger.debug(f"Verification tolerance (float): {tolerance}, Scaled tolerance (int): {scaled_tolerance}")


    try:
        # 1. Lấy dữ liệu on-chain của chu kỳ TRƯỚC
        # Giả sử hàm get_all_validator_data trả về List[Tuple[UTxO, Dict]]
        all_validator_results = await get_all_validator_data(context, script_hash, network)

        # Xử lý dữ liệu on-chain
        on_chain_states: Dict[str, Dict] = {} # Lưu state on-chain đã decode (int scaled)
        datum_map: Dict[str, ValidatorDatum] = {} # Lưu Datum object gốc

        for utxo_obj, datum_dict in all_validator_results:
            uid_hex = datum_dict.get("uid")
            last_update = datum_dict.get("last_update_slot")

            if uid_hex and last_update == previous_cycle:
                try:
                    # Decode lại Datum object từ UTxO để lấy giá trị gốc (scaled int)
                    # Điều này giả định datum trong utxo_obj là PlutusData có thể decode
                    if utxo_obj.output.datum:
                        on_chain_datum = ValidatorDatum.from_cbor(utxo_obj.output.datum.cbor)
                        on_chain_states[uid_hex] = {
                            "scaled_trust": getattr(on_chain_datum, 'scaled_trust_score', -1),
                            "scaled_perf": getattr(on_chain_datum, 'scaled_last_performance', -1),
                            # Thêm các trường scaled int khác cần kiểm tra
                        }
                        datum_map[uid_hex] = on_chain_datum # Lưu lại datum object gốc
                    else:
                         logger.warning(f"UTXO {utxo_obj.input} for {uid_hex} has no inline datum for verification.")
                except Exception as decode_e:
                     logger.warning(f"Failed to decode on-chain datum for {uid_hex} (UTxO: {utxo_obj.input}): {decode_e}")


        logger.info(f"Found {len(on_chain_states)} validator datums updated in cycle {previous_cycle} for verification.")

        # 2. So sánh với trạng thái dự kiến đã lưu
        expected_states = previous_calculated_states # Đây là dict state dự kiến từ cycle N-1
        if not expected_states:
            logger.warning("No expected validator states found from previous cycle.")
            return {}

        suspicious_validators: Dict[str, str] = {} # {uid_hex: reason}

        for uid_hex, expected in expected_states.items():
            # Lấy state on-chain tương ứng
            actual_scaled = on_chain_states.get(uid_hex)
            reason_parts = [] # Thu thập các lý do sai lệch

            if not actual_scaled:
                # Chỉ coi là "Did not commit" nếu validator được kỳ vọng là active
                if expected.get("start_status") == STATUS_ACTIVE:
                    reason_parts.append(f"Did not commit updates in cycle {previous_cycle}")
                # Không cần kiểm tra sâu hơn nếu không có dữ liệu on-chain
                if reason_parts: suspicious_validators[uid_hex] = "; ".join(reason_parts)
                continue

            # --- So sánh Scaled Trust Score ---
            expected_trust_float = expected.get("trust", -1.0) # Trust dự kiến (float)
            expected_trust_scaled = int(expected_trust_float * settings.METAGRAPH_DATUM_INT_DIVISOR)
            actual_trust_scaled = actual_scaled.get("scaled_trust", -999)
            diff_trust_scaled = abs(actual_trust_scaled - expected_trust_scaled)

            if diff_trust_scaled > scaled_tolerance:
                 # Tính lại diff float để log cho dễ hiểu
                 actual_trust_float = actual_trust_scaled / settings.METAGRAPH_DATUM_INT_DIVISOR if actual_trust_scaled != -999 else -2.0
                 diff_trust_float = abs(actual_trust_float - expected_trust_float)
                 reason_parts.append(f"Trust mismatch (Expected: {expected_trust_float:.5f}, Actual: {actual_trust_float:.5f}, Diff: {diff_trust_float:.5f})")

            # --- So sánh Scaled Performance Score ---
            expected_perf_float = expected.get("E_v", -1.0) # E_v dự kiến (float)
            expected_perf_scaled = int(expected_perf_float * settings.METAGRAPH_DATUM_INT_DIVISOR)
            actual_perf_scaled = actual_scaled.get("scaled_perf", -999)
            diff_perf_scaled = abs(actual_perf_scaled - expected_perf_scaled)

            if diff_perf_scaled > scaled_tolerance:
                 actual_perf_float = actual_perf_scaled / settings.METAGRAPH_DATUM_INT_DIVISOR if actual_perf_scaled != -999 else -2.0
                 diff_perf_float = abs(actual_perf_float - expected_perf_float)
                 reason_parts.append(f"Performance mismatch (Expected: {expected_perf_float:.5f}, Actual: {actual_perf_float:.5f}, Diff: {diff_perf_float:.5f})")

            # --- Thêm so sánh các trường khác nếu cần ---

            # Lưu lý do nếu có sai lệch
            if reason_parts:
                suspicious_validators[uid_hex] = "; ".join(reason_parts)
                logger.warning(f"Deviation detected for Validator {uid_hex}: {suspicious_validators[uid_hex]}")

        # 3. Đồng thuận về Gian lận (Placeholder)
        confirmed_deviators: Dict[str, str] = {}
        if suspicious_validators:
            logger.info(f"Consensus on {len(suspicious_validators)} suspicious validators needed (currently mocked).")
            # TODO: Implement P2P fraud consensus logic
            confirmed_deviators = suspicious_validators # <<<--- Tạm thời xác nhận tất cả
            logger.warning(f"Deviation confirmed (mock): {list(confirmed_deviators.keys())}")

        # 4. Áp dụng Phạt Trust/Status và Chuẩn bị Datum Phạt
        for uid_hex, reason in confirmed_deviators.items():
            validator_info = validators_info.get(uid_hex) # Lấy info hiện tại (đầu cycle N)
            if not validator_info:
                logger.warning(f"Info for penalized validator {uid_hex} not found in current state.")
                continue

            logger.warning(f"Applying penalty to Validator {uid_hex} for: {reason}")

            # a. Xác định mức độ nghiêm trọng
            fraud_severity = _calculate_fraud_severity(reason, tolerance)

            # b. Tính lượng slash tiềm năng
            slash_amount = calculate_slash_amount(validator_info.stake, fraud_severity, settings.CONSENSUS_PARAM_MAX_SLASH_RATE)
            if slash_amount > 0:
                 logger.warning(f"Potential slash amount for {uid_hex}: {slash_amount / 1e6:.6f} ADA (Severity: {fraud_severity:.2f}). Needs trigger mechanism.")
                 # TODO: Trigger Slashing Mechanism (Future/DAO)

            # c. Phạt Trust Score (cập nhật vào validators_info)
            penalty_eta = settings.CONSENSUS_PARAM_PENALTY_ETA
            original_trust = validator_info.trust_score
            new_trust_score = max(0.0, original_trust * (1 - penalty_eta * fraud_severity))
            if abs(new_trust_score - original_trust) > EPSILON: # Chỉ log nếu có thay đổi
                logger.warning(f"Penalizing Trust Score for {uid_hex}: {original_trust:.4f} -> {new_trust_score:.4f} (Eta: {penalty_eta}, Severity: {fraud_severity:.2f})")
                validator_info.trust_score = new_trust_score # *** Cập nhật trust trong bộ nhớ ***
            else:
                logger.info(f"Trust score for {uid_hex} remains {original_trust:.4f} (penalty negligible).")


            # d. Phạt Status (cập nhật vào validators_info)
            new_status = validator_info.status
            # --- Logic Jailed linh hoạt hơn ---
            jailed_threshold = getattr(settings, 'CONSENSUS_JAILED_SEVERITY_THRESHOLD', 0.2) # Lấy ngưỡng từ settings (ví dụ 0.2)
            if fraud_severity >= jailed_threshold and validator_info.status == STATUS_ACTIVE:
                new_status = STATUS_JAILED
                logger.warning(f"Setting Validator {uid_hex} status to JAILED (Severity {fraud_severity:.2f} >= Threshold {jailed_threshold}).")
                validator_info.status = new_status # *** Cập nhật status trong bộ nhớ ***
            # ----------------------------------

            # e. Chuẩn bị ValidatorDatum mới để Commit Hình Phạt
            # Lấy Datum gốc on-chain đã đọc được ở bước 1 (nếu có)
            original_datum = datum_map.get(uid_hex)
            if not original_datum:
                 logger.error(f"Cannot prepare penalty datum for {uid_hex}: Original on-chain datum not found/decoded.")
                 continue # Không thể tạo datum phạt nếu không có datum gốc

            try:
                # Tạo datum mới dựa trên datum gốc, chỉ cập nhật trust, status, slot
                datum_to_commit = ValidatorDatum(
                    uid=original_datum.uid,
                    subnet_uid=original_datum.subnet_uid,
                    stake=original_datum.stake, # Giữ stake cũ
                    scaled_last_performance=original_datum.scaled_last_performance, # Giữ perf cũ
                    scaled_trust_score=int(new_trust_score * settings.METAGRAPH_DATUM_INT_DIVISOR), # <<< Trust mới bị phạt
                    accumulated_rewards=original_datum.accumulated_rewards, # Giữ rewards cũ
                    last_update_slot=current_cycle, # <<< Cập nhật slot là chu kỳ HIỆN TẠI
                    performance_history_hash=original_datum.performance_history_hash,
                    wallet_addr_hash=original_datum.wallet_addr_hash, # Giữ hash address cũ
                    status=new_status, # <<< Status mới
                    registration_slot=original_datum.registration_slot,
                    api_endpoint=original_datum.api_endpoint,
                )
                penalized_validator_datums[uid_hex] = datum_to_commit
                logger.info(f"Prepared penalty datum update for {uid_hex}.")
            except Exception as build_e:
                logger.error(f"Failed to build penalty datum for {uid_hex}: {build_e}")

    except Exception as e:
        logger.exception(f"Error during validator verification/penalization: {e}")

    return penalized_validator_datums


# --- Logic Chuẩn bị và Commit Cập nhật ---

async def prepare_miner_updates_logic( # <<<--- async vì cần lấy/decode datum cũ
    current_cycle: int,
    miners_info: Dict[str, MinerInfo], # Trạng thái miner đầu vào (có thể đã bị phạt)
    final_scores: Dict[str, float], # Điểm P_adj
    settings: Any,
    # --- Vẫn cần context và map UTXO để lấy reward cũ ---
    # context: BlockFrostChainContext, # Có thể là Optional nếu map UTXO đã chứa datum decode sẵn
    current_utxo_map: Dict[str, UTxO] # Map uid_hex -> UTxO object
    # -------------------------------------------------
) -> Dict[str, MinerDatum]:
    """
    Chuẩn bị dữ liệu MinerDatum mới để commit.
    Tính toán trust mới, rewards mới, history hash mới.
    Ưu tiên lấy thông tin tĩnh từ miners_info, nhưng cần Datum cũ cho reward tích lũy.
    """
    logger.info(f"Preparing miner state updates for cycle {current_cycle}...")
    miner_updates: Dict[str, MinerDatum] = {}
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR

    # Ước tính total_system_value cho incentive (tổng W*P_adj của miner active)
    total_weighted_perf = sum(
        getattr(minfo, 'weight', 0) * final_scores.get(uid, 0)
        for uid, minfo in miners_info.items() if getattr(minfo, 'status', STATUS_ACTIVE) == STATUS_ACTIVE
    )
    # Đặt giá trị tối thiểu để tránh chia cho 0 và incentive quá lớn khi mạng nhỏ
    min_total_value = 1.0 # Hoặc một giá trị phù hợp khác
    total_system_value = max(min_total_value, total_weighted_perf)
    logger.debug(f"Using total_system_value (Sum W*P_adj, min={min_total_value}): {total_system_value:.4f}")

    for miner_uid_hex, miner_info in miners_info.items():
        log_prefix = f"Miner {miner_uid_hex}"
        logger.debug(f"{log_prefix}: Preparing update...")
        score_new = final_scores.get(miner_uid_hex, 0.0) # P_adj
        trust_score_old = getattr(miner_info, 'trust_score', 0.0) # Trust score đầu vào (trước update)

        # --- 1. Lấy Thông tin Cần Thiết từ Datum Cũ (Chủ yếu là Reward) ---
        pending_rewards_old = 0
        # Các trường này ưu tiên lấy từ Info nếu đã load đúng, nếu không thì lấy từ Datum cũ
        old_perf_history = list(getattr(miner_info, 'performance_history', []))
        final_wallet_addr_hash = getattr(miner_info, 'wallet_addr_hash', None)
        perf_hash_old_bytes = getattr(miner_info, 'performance_history_hash', None)
        registration_slot_old = getattr(miner_info, 'registration_slot', 0) # Lấy từ info nếu có

        input_utxo = current_utxo_map.get(miner_uid_hex)
        if input_utxo and input_utxo.output.datum:
            try:
                # Decode datum cũ chủ yếu để lấy reward cũ
                old_datum = MinerDatum.from_cbor(input_utxo.output.datum.cbor)
                pending_rewards_old = getattr(old_datum, 'accumulated_rewards', 0)
                logger.debug(f"{log_prefix}: Old accumulated_rewards from datum: {pending_rewards_old}")

                # Chỉ lấy từ datum cũ nếu chưa có trong info
                if not final_wallet_addr_hash: final_wallet_addr_hash = getattr(old_datum, 'wallet_addr_hash', None)
                if not perf_hash_old_bytes: perf_hash_old_bytes = getattr(old_datum, 'performance_history_hash', None)
                if registration_slot_old == 0: registration_slot_old = getattr(old_datum, 'registration_slot', 0)
                # Logic phức tạp hơn nếu cần decode history từ hash cũ
                # if not old_perf_history and perf_hash_old_bytes: ...

            except Exception as e:
                logger.warning(f"{log_prefix}: Could not decode old MinerDatum: {e}. Using defaults (rewards=0).")
        else:
             logger.warning(f"{log_prefix}: Old UTXO/Datum not found. Assuming 0 old rewards.")
        # ----------------------------------------------------------

        # --- 2. Tính Trust Score Mới ---
        time_since_eval = 1 # Giả định được đánh giá mỗi chu kỳ nếu active
        # Chỉ cập nhật trust dựa trên điểm mới nếu miner đang active
        score_for_trust_update = score_new if getattr(miner_info, 'status', STATUS_ACTIVE) == STATUS_ACTIVE else 0.0
        new_trust_score_float = update_trust_score(
            trust_score_old=trust_score_old, time_since_last_eval=time_since_eval,
            score_new=score_for_trust_update, # Dùng P_adj hoặc 0
            # Lấy các tham số từ settings
            delta_trust=settings.CONSENSUS_PARAM_DELTA_TRUST, alpha_base=settings.CONSENSUS_PARAM_ALPHA_BASE,
            k_alpha=settings.CONSENSUS_PARAM_K_ALPHA, update_sigmoid_L=settings.CONSENSUS_PARAM_UPDATE_SIG_L,
            update_sigmoid_k=settings.CONSENSUS_PARAM_UPDATE_SIG_K, update_sigmoid_x0=settings.CONSENSUS_PARAM_UPDATE_SIG_X0
        )
        logger.debug(f"{log_prefix}: Trust update: {trust_score_old:.4f} -> {new_trust_score_float:.4f}")
        # -----------------------------

        # --- 3. Tính Incentive (Dùng trust CŨ) ---
        incentive_float = 0.0
        if getattr(miner_info, 'status', STATUS_ACTIVE) == STATUS_ACTIVE: # Chỉ miner active mới nhận thưởng
            incentive_float = calculate_miner_incentive(
                trust_score=trust_score_old, # <<<--- Dùng trust cũ
                miner_weight=getattr(miner_info, 'weight', 0.0),
                miner_performance_scores=[score_new], # Dùng P_adj
                total_system_value=total_system_value,
                # Lấy các tham số từ settings
                incentive_sigmoid_L=settings.CONSENSUS_PARAM_INCENTIVE_SIG_L,
                incentive_sigmoid_k=settings.CONSENSUS_PARAM_INCENTIVE_SIG_K,
                incentive_sigmoid_x0=settings.CONSENSUS_PARAM_INCENTIVE_SIG_X0
            )
        logger.debug(f"{log_prefix}: Incentive calculated: {incentive_float:.6f}")
        # -------------------------------------

        # --- 4. Cập nhật Accumulated Rewards ---
        accumulated_rewards_new = pending_rewards_old + int(incentive_float * divisor)
        logger.debug(f"{log_prefix}: AccumulatedRewards update: {pending_rewards_old} -> {accumulated_rewards_new}")
        # -------------------------------------

        # --- 5. Cập nhật Performance History & Hash ---
        # Giả định miner_info.performance_history chứa list float đã load đúng
        updated_history = old_perf_history # Bắt đầu từ history cũ
        updated_history.append(score_new) # Thêm P_adj mới nhất
        max_len = settings.CONSENSUS_MAX_PERFORMANCE_HISTORY_LEN
        updated_history = updated_history[-max_len:] # Giữ độ dài tối đa

        perf_history_hash_new: Optional[bytes] = None
        if updated_history:
            try:
                 perf_history_hash_new = hash_data(updated_history) # hash_data cần xử lý list float
                 logger.debug(f"{log_prefix}: New performance history hash created.")
            except Exception as hash_e:
                 logger.error(f"{log_prefix}: Failed to hash performance history: {hash_e}")
                 perf_history_hash_new = perf_hash_old_bytes # Giữ hash cũ nếu lỗi
        # -----------------------------------------

        # --- 6. Lấy các giá trị tĩnh khác từ MinerInfo ---
        api_endpoint_str = getattr(miner_info, 'api_endpoint', None)
        api_endpoint_bytes = api_endpoint_str.encode('utf-8') if api_endpoint_str else None
        current_status = getattr(miner_info, 'status', STATUS_ACTIVE) # Status hiện tại
        registration_slot = registration_slot_old # Giữ slot đăng ký gốc
        subnet_uid = getattr(miner_info, 'subnet_uid', 0)
        stake = int(getattr(miner_info, 'stake', 0))
        # ---------------------------------------------

        # --- 7. Tạo MinerDatum mới ---
        try:
            # Đảm bảo uid là bytes
            uid_bytes = bytes.fromhex(miner_uid_hex)

            # Đảm bảo wallet_addr_hash là bytes hoặc None
            final_wallet_addr_hash_bytes = final_wallet_addr_hash if isinstance(final_wallet_addr_hash, bytes) else None

            new_datum = MinerDatum(
                uid=uid_bytes,
                subnet_uid=subnet_uid,
                stake=stake,
                scaled_last_performance=int(score_new * divisor), # Perf mới (P_adj)
                scaled_trust_score=int(new_trust_score_float * divisor), # <<< Trust MỚI
                accumulated_rewards=accumulated_rewards_new, # <<< Reward MỚI
                last_update_slot=current_cycle,
                performance_history_hash=perf_history_hash_new, # <<< Hash MỚI
                wallet_addr_hash=final_wallet_addr_hash_bytes, # Hash từ Info/Datum cũ (bytes)
                status=current_status,
                registration_slot=registration_slot,
                api_endpoint=api_endpoint_bytes,
            )
            miner_updates[miner_uid_hex] = new_datum
            logger.debug(f"{log_prefix}: Successfully prepared new MinerDatum.")

        except ValueError as hex_err:
             logger.error(f"{log_prefix}: Invalid UID format, cannot convert from hex: {hex_err}")
        except Exception as e:
            logger.error(f"{log_prefix}: Failed to create MinerDatum: {e}", exc_info=True)

    logger.info(f"Prepared {len(miner_updates)} miner datums for update.")
    return miner_updates


async def prepare_validator_updates_logic( # <<<--- Chuyển thành async vì cần lấy datum cũ
    current_cycle: int,
    self_validator_info: ValidatorInfo, # <<<--- Nhận thông tin validator hiện tại
    calculated_states: Dict[str, Any], # Trạng thái validator dự kiến đã tính (bao gồm cả reward)
    settings: Any,
    context: BlockFrostChainContext # <<<--- Thêm context
) -> Dict[str, ValidatorDatum]:
    """Chuẩn bị dữ liệu ValidatorDatum mới cho chính validator này."""
    logger.info(f"Preparing self validator state update for cycle {current_cycle}...")
    validator_updates: Dict[str, ValidatorDatum] = {}
    self_uid_hex = self_validator_info.uid # Lấy UID hex từ info object

    if self_uid_hex in calculated_states:
        state = calculated_states[self_uid_hex]
        old_datum: Optional[ValidatorDatum] = None
        pending_rewards_old = 0
        wallet_addr_hash_old = b'default_wallet_hash_placeholder' # Giá trị mặc định
        perf_history_hash_old = None
        registration_slot_old = 0
        subnet_uid_old = 0
        stake_old = 0
        status_old = STATUS_ACTIVE
        version_old = 0
        api_endpoint_old = None


        # TODO: Triển khai logic lấy Datum cũ của chính mình bằng context và self_uid_hex
        try:
            # Giả sử có hàm get_validator_datum hoặc cách tìm UTXO + decode datum
            # validator_utxo = await metagraph_data.find_validator_utxo(context, self_uid_hex)
            # if validator_utxo and validator_utxo.output.datum:
            #     old_datum = ValidatorDatum.from_cbor(validator_utxo.output.datum.cbor)
            #     pending_rewards_old = getattr(old_datum, 'accumulated_rewards', 0)
            #     wallet_addr_hash_old = getattr(old_datum, 'wallet_addr_hash', wallet_addr_hash_old)
            #     perf_history_hash_old = getattr(old_datum, 'performance_history_hash', None)
            #     registration_slot_old = getattr(old_datum, 'registration_slot', 0)
            #     subnet_uid_old = getattr(old_datum, 'subnet_uid', 0)
            #     stake_old = getattr(old_datum, 'stake', 0) # Lấy stake từ datum cũ hay từ info mới nhất? -> Nên từ info
            #     status_old = getattr(old_datum, 'status', STATUS_ACTIVE)
            #     version_old = getattr(old_datum, 'version', 0)
            #     api_endpoint_old = getattr(old_datum, 'api_endpoint', None)

            logger.debug(f"Successfully fetched old datum for self ({self_uid_hex}). Old rewards: {pending_rewards_old}")
            # Lấy các giá trị không đổi từ self_validator_info (đã được load mới nhất)
            stake_current = int(self_validator_info.stake)
            subnet_uid_current = self_validator_info.subnet_uid
            registration_slot_current = getattr(self_validator_info, 'registration_slot', registration_slot_old) # Giữ slot gốc
            validator_address_str = self_validator_info.address
            wallet_addr_hash_bytes = validator_address_str.encode('utf-8') # Encode address thành bytes
            api_endpoint_current = self_validator_info.api_endpoint
            status_current = getattr(self_validator_info, 'status', status_old) # Lấy status hiện tại (có thể đã bị phạt)
            version_current = max(1, getattr(self_validator_info, 'version', version_old)) # Lấy version hiện tại

        except Exception as e:
            logger.warning(f"Could not fetch or parse old datum for self ({self_uid_hex}): {e}. Using defaults/current info.")
            # Sử dụng giá trị từ self_validator_info nếu không lấy được datum cũ
            stake_current = int(self_validator_info.stake)
            subnet_uid_current = self_validator_info.subnet_uid
            registration_slot_current = getattr(self_validator_info, 'registration_slot', 0)
            wallet_addr_hash_current = hash_data(self_validator_info.address) # Hash lại nếu ko có datum cũ
            api_endpoint_current = self_validator_info.api_endpoint
            status_current = getattr(self_validator_info, 'status', STATUS_ACTIVE)
            version_current = getattr(self_validator_info, 'version', 1)


        # TODO: Hash lịch sử hiệu suất mới nếu cần lưu
        # new_perf_history = ...
        # perf_history_hash = hash_data(new_perf_history) if new_perf_history else perf_history_hash_old
        perf_history_hash = perf_history_hash_old # Tạm thời giữ hash cũ

        # Tính phần thưởng mới tích lũy
        calculated_reward = state.get("reward", 0.0)
        accumulated_rewards_new = pending_rewards_old + int(calculated_reward * settings.METAGRAPH_DATUM_INT_DIVISOR)

        # Tạo ValidatorDatum mới
        try:
            new_datum = ValidatorDatum(
                uid=bytes.fromhex(self_uid_hex),
                subnet_uid=subnet_uid_current,
                stake=stake_current,
                scaled_last_performance=int(state.get("E_v", 0.0) * settings.METAGRAPH_DATUM_INT_DIVISOR),
                scaled_trust_score=int(state.get("trust", 0.0) * settings.METAGRAPH_DATUM_INT_DIVISOR),
                accumulated_rewards=accumulated_rewards_new,
                last_update_slot=current_cycle, # Ghi lại chu kỳ hiện tại
                performance_history_hash=perf_history_hash,
                wallet_addr_hash=wallet_addr_hash_bytes,
                status=status_current,
                registration_slot=registration_slot_current,
                api_endpoint=api_endpoint_current.encode('utf-8') if api_endpoint_current else None,
            )
            validator_updates[self_uid_hex] = new_datum
            logger.info(f"Prepared update for self ({self_uid_hex})")
        except Exception as e:
            logger.exception(f"Failed to create ValidatorDatum for self ({self_uid_hex}): {e}")

    return validator_updates


async def commit_updates_logic(
    miner_updates: Dict[str, MinerDatum],           # {uid_hex: MinerDatum mới}
    validator_updates: Dict[str, ValidatorDatum],   # {uid_hex: ValidatorDatum mới (self)}
    penalized_validator_updates: Dict[str, ValidatorDatum], # {uid_hex: ValidatorDatum mới (phạt)}
    current_utxo_map: Dict[str, UTxO], # Map từ uid_hex -> UTxO object ở đầu chu kỳ
    context: BlockFrostChainContext,
    signing_key: PaymentSigningKey, # Đây là ExtendedSigningKey
    stake_signing_key: Optional[StakeSigningKey], # Đây cũng là ExtendedSigningKey
    settings: Any, # Đối tượng settings đầy đủ
    script_hash: ScriptHash,
    script_bytes: PlutusV3Script,
    network: Network
):
    """
    Commit các cập nhật MinerDatum và ValidatorDatum lên blockchain.
    Sử dụng current_utxo_map để lấy input UTXO.
    Thực hiện mỗi update một giao dịch riêng lẻ.
    """
    logger.info(f"Starting blockchain commit process (1 Tx per Update)...")
    log_details = f"Updates - Miners: {len(miner_updates)}, SelfValidators: {len(validator_updates)}, PenalizedValidators: {len(penalized_validator_updates)}"
    logger.info(log_details)

    # --- Lấy thông tin của Owner (Validator đang chạy node) ---
    try:
        owner_payment_vkey: PaymentVerificationKey = signing_key.to_verification_key()
        owner_payment_key_hash: VerificationKeyHash = owner_payment_vkey.hash()
        owner_stake_key_hash: Optional[VerificationKeyHash] = None
        if stake_signing_key:
            owner_stake_key_hash = stake_signing_key.to_verification_key().hash()

        owner_address = Address(
            payment_part=owner_payment_key_hash,
            staking_part=owner_stake_key_hash,
            network=network
        )
        logger.info(f"Commit Owner Address: {owner_address}")
    except Exception as e:
        logger.exception(f"Failed to derive owner address or keys: {e}. Aborting commit.")
        return {"status": "failed", "reason": "Owner key/address derivation failed."}
    
    contract_address = Address(payment_part=script_hash, network=network)
    logger.debug(f"Contract Address: {contract_address}")

    default_redeemer = Redeemer(0) # Tag 0

    submitted_tx_ids: Dict[str, str] = {} # {uid_hex_type: tx_id}
    failed_updates: Dict[str, str] = {}   # {uid_hex: error_message}
    skipped_updates: Dict[str, str] = {}  # {uid_hex: reason}

    # Hợp nhất các cập nhật Validator
    all_validator_updates = validator_updates.copy()
    all_validator_updates.update(penalized_validator_updates)
    logger.info(f"Total validator updates to commit: {len(all_validator_updates)}")

    # --- Gom tất cả updates vào một list để xử lý chung ---
    # List các tuple: (uid_hex, new_datum, datum_type_str)
    all_updates: List[Tuple[str, Union[MinerDatum, ValidatorDatum], str]] = []
    for uid_hex, datum in miner_updates.items():
        all_updates.append((uid_hex, datum, "Miner"))
    for uid_hex, datum in all_validator_updates.items():
        all_updates.append((uid_hex, datum, "Validator"))

    logger.info(f"Total updates to attempt committing: {len(all_updates)}")

    # --- Xử lý tuần tự từng update (để dễ quản lý lỗi và delay) ---
    commit_count = 0
    for uid_hex, new_datum, datum_type in all_updates:
        commit_count += 1
        log_prefix = f"Commit #{commit_count}/{len(all_updates)} ({datum_type} {uid_hex})"
        logger.debug(f"{log_prefix}: Processing...")

        # 1. Lấy Input UTXO từ map
        input_utxo = current_utxo_map.get(uid_hex)
        if not input_utxo:
            error_msg = "Input UTxO not found in initial map"
            logger.error(f"{log_prefix}: Skipped - {error_msg}")
            skipped_updates[uid_hex] = error_msg
            continue

        logger.debug(f"{log_prefix}: Found Input UTxO: {input_utxo.input}")

        # 2. Xây dựng Giao dịch
        try:
            builder = TransactionBuilder(context=context)

            # a. Thêm Input Script UTXO (UTXO cũ chứa datum)
            builder.add_script_input(
                utxo=input_utxo,
                script=script_bytes,
                redeemer=default_redeemer
            )
            logger.debug(f"{log_prefix}: Added script input: {input_utxo.input}")

            # b. Thêm Output mới (trả về contract với datum mới)
            #    Giữ nguyên giá trị (coin + multi-asset) của input UTXO
            output_value: Value = input_utxo.output.amount
            builder.add_output(
                TransactionOutput(
                    address=contract_address,
                    amount=output_value, # <<<--- Giữ nguyên giá trị đầy đủ
                    datum=new_datum
                )
            )
            logger.debug(f"{log_prefix}: Added script output with new datum (Amount: {output_value.coin} Lovelace)")

            # c. Thêm Input từ ví Owner để trả phí và làm collateral
            #    TransactionBuilder sẽ tự động chọn UTXO từ địa chỉ này
            builder.add_input_address(owner_address)
            logger.debug(f"{log_prefix}: Added owner address input: {owner_address}")

            # d. Chỉ định người ký cần thiết (là hash của payment key của owner)
            builder.required_signers = [owner_payment_key_hash]
            logger.debug(f"{log_prefix}: Set required signer: {owner_payment_key_hash.to_primitive().hex()}")

            # e. Build và Ký Giao dịch
            #    build_and_sign sẽ tự động tính phí, cân bằng giao dịch,
            #    tạo output trả về tiền thừa (change) cho owner_address.
            logger.debug(f"{log_prefix}: Building and signing transaction...")
            # Chỉ cần payment key để ký vì required_signers chỉ có payment key hash
            # Nếu script yêu cầu stake key, cần thêm stake_signing_key vào list
            signing_keys_list = [signing_key]
            # if stake_signing_key and owner_stake_key_hash in builder.required_signers:
            #    signing_keys_list.append(stake_signing_key)

            signed_tx = builder.build_and_sign(
                signing_keys=signing_keys_list,
                change_address=owner_address,
            )
            logger.debug(f"{log_prefix}: Transaction built and signed. Fee: {signed_tx.transaction_body.fee}")

        except Exception as build_e:
            logger.exception(f"{log_prefix}: Failed during transaction build/sign phase: {build_e}")
            failed_updates[uid_hex] = f"Build/Sign Error: {str(build_e)}"
            continue # Chuyển sang update tiếp theo

        # 3. Submit Giao dịch
        try:
            logger.info(f"{log_prefix}: Submitting transaction to the blockchain...")
            tx_id: TransactionId = context.submit_tx(signed_tx) # submit_tx trả về TransactionId
            tx_id_str = str(tx_id)
            logger.info(f"{log_prefix}: Successfully submitted update! TxID: {tx_id_str}")
            submitted_tx_ids[f"{datum_type.lower()}_{uid_hex}"] = tx_id_str

            # Delay nhỏ
            commit_delay = getattr(settings, 'CONSENSUS_COMMIT_DELAY_SECONDS', 1.5)
            logger.debug(f"Waiting {commit_delay}s before next commit...")
            await asyncio.sleep(commit_delay)

        except ApiError as e:
             logger.error(f"{log_prefix}: Blockfrost API Error on submit: Status={e.status_code}, Message={e.message}", exc_info=False)
             failed_updates[uid_hex] = f"Blockfrost API Error ({e.status_code}): {e.message}"
        except Exception as e:
            logger.exception(f"{log_prefix}: Generic error during transaction submission: {e}")
            failed_updates[uid_hex] = f"Submit Error: {str(e)}"


    # --- 3. Tổng kết ---
    total_submitted = len(submitted_tx_ids)
    total_failed = len(failed_updates)
    total_skipped = len(skipped_updates)
    logger.info(f"Commit process finished. Submitted: {total_submitted}, Failed: {total_failed}, Skipped (No Input UTXO): {total_skipped}")
    if failed_updates:
        logger.warning(f"Failed updates details: {failed_updates}")
    if skipped_updates:
        logger.warning(f"Skipped updates details: {skipped_updates}")

    # Trả về kết quả (ví dụ)
    return {
        "status": "completed" if not failed_updates else "completed_with_errors",
        "submitted_count": total_submitted,
        "failed_count": total_failed,
        "skipped_count": total_skipped,
        "submitted_txs": submitted_tx_ids,
        "failures": failed_updates,
        "skips": skipped_updates
    }