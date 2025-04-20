# sdk/consensus/state.py
"""
Contains logic for consensus calculations, validator penalty checks,
and preparing/committing state updates to the blockchain.
"""
import logging
import math
import hashlib  # Đảm bảo đã import
import asyncio
from typing import List, Dict, Any, Tuple, Optional, Set, Union
import numpy as np  # Cần cài đặt numpy: pip install numpy
from collections import defaultdict

from sdk.config.settings import settings
from sdk.core.datatypes import MinerInfo, ValidatorInfo, ValidatorScore, TaskAssignment
from sdk.formulas import (
    calculate_adjusted_miner_performance,
    calculate_validator_performance,
    update_trust_score,
    calculate_fraud_severity_value,  # Cần logic cụ thể
    calculate_slash_amount,
    calculate_miner_incentive,
    calculate_validator_incentive,
    # Import các công thức khác nếu cần
)

# Import tokenomics functions
from sdk.formulas.tokenomics import (
    get_current_epoch,
    calculate_epoch_issuance,
    distribute_issuance,
)

from sdk.metagraph.update_metagraph import update_datum
from sdk.metagraph.metagraph_data import get_all_validator_data
from sdk.metagraph.hash.hash_datum import hash_data  # Cần hàm hash
from pycardano import (
    BlockFrostChainContext,
    PaymentSigningKey,
    StakeSigningKey,
    TransactionId,
    Network,
    ScriptHash,
    UTxO,
    Address,
    PlutusV3Script,
    Redeemer,
    VerificationKeyHash,
    PaymentVerificationKey,
    TransactionBuilder,
    Value,
    TransactionOutput,
    ExtendedSigningKey,
    ExtendedVerificationKey,
    RawPlutusData,
)
from sdk.metagraph.metagraph_datum import (
    MinerDatum,
    ValidatorDatum,
    STATUS_ACTIVE,
    STATUS_JAILED,
    STATUS_INACTIVE,
)
from blockfrost import ApiError

logger = logging.getLogger(__name__)

EPSILON = 1e-9  # <<< ĐỊNH NGHĨA EPSILON TRỰC TIẾP

# Default empty bytes for hash placeholders
EMPTY_HASH_BYTES = b""

# --- Logic Đồng thuận và Tính toán Trạng thái Validator ---


def calculate_historical_consistency(
    scores: List[float], max_stddev_penalty: float = 2.0
) -> float:
    """
    Calculates a quality score based on the standard deviation of historical scores.
    A lower standard deviation (more stable performance) results in a higher score.

    Args:
        scores (List[float]): A list of historical performance scores.
        max_stddev_penalty (float): The maximum standard deviation allowed before
                                  the quality score drops to 0. Defaults to 2.0.

    Returns:
        float: A quality score between 0.0 and 1.0.
    """
    if not scores or len(scores) < 2:
        return 0.5  # Return average value if insufficient data

    stddev = float(np.std(scores))

    # Chuẩn hóa điểm: 1.0 khi stddev=0, giảm dần về 0 khi stddev tăng
    # Ví dụ: Giảm tuyến tính, về 0 khi stddev >= max_stddev_penalty
    # Cần đảm bảo max_stddev_penalty > 0
    if max_stddev_penalty <= 0:
        max_stddev_penalty = 0.5  # Giá trị an toàn

    normalized_penalty = min(1.0, stddev / max_stddev_penalty)
    consistency_score = 1.0 - normalized_penalty

    # Đảm bảo kết quả cuối cùng trong khoảng [0, 1]
    return max(0.0, min(1.0, consistency_score))


# --- Placeholder Function: Tìm UTXO theo UID ---
# Cần thay thế bằng logic thực tế, có thể cần sửa đổi get_all_miner/validator_data
async def find_utxo_by_uid(
    context: BlockFrostChainContext,
    script_hash: ScriptHash,
    network: Network,
    uid_bytes: bytes,
    datum_class: type,  # MinerDatum hoặc ValidatorDatum
) -> Optional[UTxO]:
    """
    (Placeholder/Mock) Finds a UTxO at the script address containing a Datum with a matching UID.

    Note:
        This is a placeholder and likely inefficient. Real implementation should
        optimize UTXO fetching, potentially by querying for specific datums if the
        backend supports it, or by using an off-chain index.

    Args:
        context (BlockFrostChainContext): The Cardano chain context.
        script_hash (ScriptHash): The hash of the Plutus script.
        network (Network): The Cardano network.
        uid_bytes (bytes): The UID (as bytes) to search for within the datum.
        datum_class (type): The expected Datum class (e.g., MinerDatum, ValidatorDatum).

    Returns:
        Optional[UTxO]: The found UTxO, or None if not found or an error occurs.
    """
    logger.debug(
        f"Searching for UTxO with UID {uid_bytes.hex()} of type {datum_class.__name__}..."
    )
    contract_address = Address(payment_part=script_hash, network=network)
    try:
        # TODO: Tối ưu hóa: chỉ fetch UTxO liên quan nếu có thể
        # Tạm thời fetch hết và lọc
        utxos = context.utxos(str(contract_address))
        for utxo in utxos:
            if utxo.output.datum:
                try:
                    decoded_datum = datum_class.from_cbor(utxo.output.datum.cbor)  # type: ignore
                    if (
                        hasattr(decoded_datum, "uid")
                        and getattr(decoded_datum, "uid") == uid_bytes
                    ):
                        logger.debug(f"Found matching UTxO: {utxo.input}")
                        return utxo
                except Exception:
                    # logger.warning(f"Failed to decode datum for {utxo.input} as {datum_class.__name__}")
                    continue  # Bỏ qua datum không decode được hoặc sai loại
    except Exception as e:
        logger.error(
            f"Failed to fetch UTxOs for {contract_address} while searching for UID {uid_bytes.hex()}: {e}"
        )

    logger.warning(
        f"UTxO for UID {uid_bytes.hex()} of type {datum_class.__name__} not found."
    )
    return None


# -----------------------------------------


# --- Hàm tính Severity tinh chỉnh hơn ---
def _calculate_fraud_severity(reason: str, tolerance: float) -> float:
    """
    Calculates a fraud severity score based on the deviation reason string.

    Parses mismatch details from the reason string and compares the difference
    against the tolerance to determine severity.

    Args:
        reason (str): String describing the deviation (e.g., "Trust mismatch (... Diff: 0.1)").
        tolerance (float): The acceptable tolerance for floating-point comparisons.

    Returns:
        float: A severity score (e.g., 0.05 for non-commit, 0.1-0.7 for mismatches).
    """
    severity = 0.0
    max_deviation_factor = 0.0
    if "Did not commit" in reason:
        return 0.05
    parts = reason.split(";")
    for part in parts:
        part = part.strip()
        if "mismatch" in part:
            try:
                diff_str = part.split("Diff:")[-1].strip().rstrip(")")
                diff_float = float(diff_str)
                if tolerance > 1e-9:
                    deviation_factor = diff_float / tolerance
                    max_deviation_factor = max(max_deviation_factor, deviation_factor)
            except Exception:
                pass
    severe_threshold_factor = getattr(
        settings, "CONSENSUS_SEVERITY_SEVERE_FACTOR", 10.0
    )
    moderate_threshold_factor = getattr(
        settings, "CONSENSUS_SEVERITY_MODERATE_FACTOR", 3.0
    )
    if max_deviation_factor >= severe_threshold_factor:
        severity = 0.7
    elif max_deviation_factor >= moderate_threshold_factor:
        severity = 0.3
    elif max_deviation_factor > 1.0:
        severity = 0.1
    return severity


# -----------------------------------------


def run_consensus_logic(
    current_cycle: int,
    tasks_sent: Dict[str, TaskAssignment],
    received_scores: Dict[
        str, Dict[str, ValidatorScore]
    ],  # {task_id: {validator_uid_hex: ValidatorScore}}
    validators_info: Dict[
        str, ValidatorInfo
    ],  # {validator_uid_hex: ValidatorInfo} - State at the start of the cycle
    miners_info: Dict[str, MinerInfo],  # <<< THÊM miners_info VÀO ĐÂY
    settings: Any,
    consensus_possible: bool,
    self_validator_uid: str,  # <<< Giữ lại nếu logic cần biết validator nào đang chạy
) -> Tuple[
    Dict[str, float], Dict[str, Any], Dict[str, Any], int
]:  # <<< THAY ĐỔI RETURN TYPE
    """
    Performs the core consensus calculations for a cycle.

    Calculates final miner adjusted scores (P_adj). Calculates expected next validator
    states (E_v, trust, reward including incentive + issuance). Calculates expected
    next miner states (P_adj, trust, reward from incentive).

    If `consensus_possible` is False, it skips most calculations and only applies trust decay.

    Args:
        current_cycle (int): The current cycle number.
        tasks_sent (Dict[str, TaskAssignment]): Tasks sent out during this cycle.
        received_scores (Dict[str, Dict[str, ValidatorScore]]): Scores received from peer validators.
        validators_info (Dict[str, ValidatorInfo]): Validator info at the start of the cycle.
        miners_info (Dict[str, MinerInfo]): Miner info at the start of the cycle.
        settings (Any): Application settings object.
        consensus_possible (bool): Whether enough P2P scores were received for full consensus.
        self_validator_uid (str): UID of the validator running this logic.

    Returns:
        Tuple[Dict[str, float], Dict[str, Any], Dict[str, Any], int]: A tuple containing:
            - Final miner adjusted scores ({miner_uid_hex: P_adj}).
            - Calculated next validator states ({validator_uid_hex: {state_dict}}).
            - Calculated next miner states ({miner_uid_hex: {state_dict}}).
            - Calculated issuance for the current epoch (int, raw units).
    """
    logger.info(f":brain: Running consensus calculations for cycle {current_cycle}...")
    final_miner_scores: Dict[str, float] = {}  # {miner_uid_hex: P_adj}
    validator_deviations: Dict[str, List[float]] = defaultdict(list)
    calculated_validator_states: Dict[str, Any] = {}  # {validator_uid_hex: {state}}
    calculated_miner_states: Dict[str, Any] = {}  # <<< THÊM MINER STATES
    total_validator_contribution: float = 0.0
    issuance_this_epoch = 0  # Khởi tạo giá trị issuance

    # <<< DI CHUYỂN TÍNH TOÁN LÊN ĐÂY >>>
    # Gom tất cả validator đã gửi điểm cho bất kỳ task nào
    unique_validators_overall = set()
    if received_scores:
        for validator_scores_dict in received_scores.values():
            unique_validators_overall.update(validator_scores_dict.keys())
    # Tính số validator tối thiểu cần thiết
    min_validators_needed = settings.CONSENSUS_MIN_VALIDATORS_FOR_CONSENSUS
    # ------------------------------------

    if not consensus_possible:
        logger.warning(
            f"[Consensus:{current_cycle}] Consensus not possible (received from {len(unique_validators_overall)}/{min_validators_needed} validators). Skipping score aggregation. Applying decay to non-participating validators."
        )
        # Apply decay to validators who *didn't* submit scores for any relevant task
        for validator_uid, validator_info in validators_info.items():
            if validator_uid not in unique_validators_overall:
                # Chỉ decay nếu validator đang active
                if getattr(validator_info, "status", STATUS_ACTIVE) == STATUS_ACTIVE:
                    old_trust = getattr(validator_info, "trust_score", 1.0)
                    # <<< ÁP DỤNG DECAY TIÊU CHUẨN >>>
                    delta_trust = settings.CONSENSUS_PARAM_DELTA_TRUST
                    # Giả định thời gian từ lần đánh giá cuối là 1 chu kỳ
                    new_trust = old_trust * math.exp(-delta_trust * 1)
                    new_trust = max(
                        0.0, min(1.0, new_trust)
                    )  # Đảm bảo trong khoảng [0, 1]
                    # ----------------------------------
                    # Cập nhật trực tiếp vào dict (hoặc tạo bản copy nếu cần)
                    calculated_validator_states[validator_uid] = {
                        "trust_score": new_trust,
                        "weight": getattr(validator_info, "weight", 0.0),
                        "stake": getattr(validator_info, "stake", 0.0),
                        "last_performance": getattr(
                            validator_info, "last_performance", 0.0
                        ),
                        "performance_history": getattr(
                            validator_info, "performance_history", []
                        ),
                        "status": getattr(validator_info, "status", STATUS_ACTIVE),
                        "registration_slot": getattr(
                            validator_info, "registration_slot", 0
                        ),
                        "wallet_addr_hash": getattr(
                            validator_info, "wallet_addr_hash", None
                        ),
                        "api_endpoint": getattr(validator_info, "api_endpoint", None),
                        "performance_history_hash": getattr(
                            validator_info, "performance_history_hash", None
                        ),
                    }
                    logger.debug(
                        f"Applied standard trust decay (delta={delta_trust}) to non-scoring validator {validator_uid}. Trust: {old_trust:.4f} -> {new_trust:.4f}"
                    )
        # final_miner_scores sẽ là {} như khởi tạo
        # calculated_miner_states sẽ được tính ở vòng lặp sau dựa trên final_miner_scores rỗng

    # --- 1. Tính điểm đồng thuận Miner (P_miner_adjusted) và độ lệch ---
    scores_by_miner: Dict[str, List[Tuple[float, float]]] = defaultdict(
        list
    )  # {miner_uid_hex: [(score, validator_trust)]}
    tasks_processed_by_miner: Dict[str, Set[str]] = defaultdict(
        set
    )  # {miner_uid_hex: {task_id1, task_id2,...}}
    validator_scores_by_task: Dict[str, Dict[str, float]] = defaultdict(
        dict
    )  # {task_id: {validator_uid: score}}

    # Gom điểm theo miner VÀ theo task
    for task_id, validator_scores_dict in received_scores.items():
        first_score = next(iter(validator_scores_dict.values()), None)
        if not first_score:
            continue
        miner_uid_hex = first_score.miner_uid
        tasks_processed_by_miner[miner_uid_hex].add(
            task_id
        )  # Lưu tất cả task miner đã làm

        for validator_uid_hex, score_entry in validator_scores_dict.items():
            validator = validators_info.get(validator_uid_hex)
            if (
                validator
                and getattr(validator, "status", STATUS_ACTIVE) == STATUS_ACTIVE
            ):  # Chỉ tính điểm từ validator active
                # Lưu (điểm, trust của validator chấm điểm) cho miner
                scores_by_miner[miner_uid_hex].append(
                    (score_entry.score, validator.trust_score)
                )
                # Lưu điểm của validator cho task này
                validator_scores_by_task[task_id][validator_uid_hex] = score_entry.score
            # else: logger.warning(...) # Bỏ qua validator không tồn tại hoặc inactive

    # Tính P_adj và độ lệch
    for miner_uid_hex, scores_trusts in scores_by_miner.items():
        if not scores_trusts:
            continue
        scores = [s for s, t in scores_trusts]
        trusts = [t for s, t in scores_trusts]

        p_adj = calculate_adjusted_miner_performance(scores, trusts)
        final_miner_scores[miner_uid_hex] = p_adj
        logger.info(
            f"  :chart_with_upwards_trend: Consensus score (P_adj) for Miner [cyan]{miner_uid_hex}[/cyan]: [yellow]{p_adj:.4f}[/yellow]"
        )

        # Tính độ lệch cho từng validator đã chấm điểm miner này, trên từng task
        related_task_ids = tasks_processed_by_miner.get(miner_uid_hex, set())
        for task_id in related_task_ids:
            scores_for_this_task = validator_scores_by_task.get(task_id, {})
            for validator_uid_hex, score in scores_for_this_task.items():
                # Độ lệch = |điểm validator chấm - điểm đồng thuận của miner|
                deviation = abs(score - p_adj)
                validator_deviations[validator_uid_hex].append(deviation)
                logger.debug(
                    f"  Deviation for V:{validator_uid_hex} on M:{miner_uid_hex} (Task:{task_id}): {deviation:.4f}"
                )

    # --- 2. Tính E_validator, Trust mới dự kiến, và Đóng góp cho thưởng ---
    temp_validator_contributions: Dict[str, float] = {}

    # Cải thiện cách tính E_avg: Trung bình trọng số theo stake của các validator ACTIVE
    active_validators_info = {
        uid: v
        for uid, v in validators_info.items()
        if getattr(v, "status", STATUS_ACTIVE) == STATUS_ACTIVE
    }
    total_active_stake = sum(v.stake for v in active_validators_info.values())
    e_avg_weighted = 0.0
    if total_active_stake > EPSILON:
        # Tính E_v trung bình dựa trên trạng thái *đầu chu kỳ* (last_performance từ ValidatorInfo)
        valid_e_validators_for_avg = [
            (v.stake, getattr(v, "last_performance", 0.0))
            for v in active_validators_info.values()
        ]
        if valid_e_validators_for_avg:
            e_avg_weighted = (
                sum(stake * perf for stake, perf in valid_e_validators_for_avg)
                / total_active_stake
            )
    else:
        e_avg_weighted = 0.5  # Default nếu không có ai active hoặc stake=0

    logger.info(
        f"  Weighted E_avg (based on start-of-cycle active validator stake): {e_avg_weighted:.4f}"
    )

    # Tính toán cho từng validator (kể cả inactive/jailed để có trạng thái dự kiến nếu họ quay lại)
    for validator_uid_hex, validator_info in validators_info.items():
        deviations = validator_deviations.get(validator_uid_hex, [])
        avg_dev = sum(deviations) / len(deviations) if deviations else 0.0

        # Nếu validator không chấm điểm nào thì avg_dev = 0.
        # Cân nhắc: Có nên phạt validator không tham gia chấm điểm không? (Hiện tại thì không)
        logger.debug(
            f"  Validator {validator_uid_hex}: Average deviation = {avg_dev:.4f} ({len(deviations)} scores evaluated)"
        )

        # Metric Quality Placeholder
        # Giả định validator_info.performance_history chứa list điểm số float
        historical_scores = getattr(validator_info, "performance_history", [])
        # Cần lấy tham số max_stddev_penalty từ settings hoặc đặt mặc định
        max_penalty_for_consistency = getattr(
            settings, "CONSENSUS_METRIC_MAX_STDDEV", 0.2
        )  # Ví dụ: ngưỡng 0.2
        metric_quality = calculate_historical_consistency(
            historical_scores, max_penalty_for_consistency
        )
        logger.debug(
            f"  Validator {validator_uid_hex}: Historical Consistency Metric = {metric_quality:.3f} (based on {len(historical_scores)} scores)"
        )

        # Kiểm tra xem UID của validator này có trong danh sách điểm miner cuối cùng không
        q_task_val = 0.0  # Mặc định là 0
        if (
            validator_uid_hex in final_miner_scores
        ):  # final_miner_scores đã được tính ở phần 1 của hàm
            q_task_val = final_miner_scores[validator_uid_hex]
            logger.debug(
                f"  Validator {validator_uid_hex} also acted as miner. Using P_adj={q_task_val:.4f} as Q_task_validator."
            )

        # Tính E_validator mới
        new_e_validator = calculate_validator_performance(
            q_task_validator=q_task_val,
            metric_validator_quality=metric_quality,
            deviation=avg_dev,  # Độ lệch trung bình của validator này
            theta1=settings.CONSENSUS_PARAM_THETA1,
            theta2=settings.CONSENSUS_PARAM_THETA2,
            theta3=settings.CONSENSUS_PARAM_THETA3,
            # Tham số Penalty Term lấy từ settings
            penalty_threshold_dev=settings.CONSENSUS_PARAM_PENALTY_THRESHOLD_DEV,
            penalty_k_penalty=settings.CONSENSUS_PARAM_PENALTY_K_PENALTY,
            penalty_p_penalty=settings.CONSENSUS_PARAM_PENALTY_P_PENALTY,
        )
        logger.info(
            f"  :chart_with_downwards_trend: Calculated performance (E_val) for Validator [cyan]{validator_uid_hex}[/cyan]: [yellow]{new_e_validator:.4f}[/yellow]"
        )

        # Tính Trust Score mới dự kiến
        # Nếu validator không hoạt động (inactive/jailed), chỉ áp dụng suy giảm
        time_since_val_eval = 1  # Mặc định là 1 chu kỳ
        score_for_trust_update = 0.0
        if getattr(validator_info, "status", STATUS_ACTIVE) == STATUS_ACTIVE:
            # Chỉ cập nhật trust dựa trên E_v mới nếu validator đang active
            score_for_trust_update = new_e_validator
        else:
            # Nếu không active, trust chỉ bị suy giảm (score_new = 0)
            # Có thể cần logic tính time_since phức tạp hơn nếu validator bị inactive/jailed lâu
            logger.debug(
                f"Validator {validator_uid_hex} is not active. Applying only trust decay."
            )

        new_val_trust_score = update_trust_score(
            validator_info.trust_score,  # Trust score đầu chu kỳ
            time_since_val_eval,
            score_for_trust_update,  # Dùng E_val mới tính nếu active, nếu không thì dùng 0
            delta_trust=settings.CONSENSUS_PARAM_DELTA_TRUST,
            alpha_base=settings.CONSENSUS_PARAM_ALPHA_BASE,
            k_alpha=settings.CONSENSUS_PARAM_K_ALPHA,
            update_sigmoid_L=settings.CONSENSUS_PARAM_UPDATE_SIG_L,
            update_sigmoid_k=settings.CONSENSUS_PARAM_UPDATE_SIG_K,
            update_sigmoid_x0=settings.CONSENSUS_PARAM_UPDATE_SIG_X0,
        )
        logger.info(
            f"  :sparkles: Calculated next Trust for Validator [cyan]{validator_uid_hex}[/cyan]: [yellow]{new_val_trust_score:.4f}[/yellow]"
        )

        # Tính đóng góp W*E cho việc tính thưởng (dùng weight đầu chu kỳ và E_v mới)
        current_weight = getattr(validator_info, "weight", 0.0)
        # Chỉ validator active mới đóng góp vào việc chia thưởng
        contribution = 0.0
        if getattr(validator_info, "status", STATUS_ACTIVE) == STATUS_ACTIVE:
            contribution = current_weight * new_e_validator
            temp_validator_contributions[validator_uid_hex] = contribution
            total_validator_contribution += contribution

        # Lưu trạng thái dự kiến (bao gồm cả E_v, trust cho validator inactive/jailed)
        calculated_validator_states[validator_uid_hex] = {
            "E_v": new_e_validator,
            "trust": new_val_trust_score,  # Trust dự kiến cuối chu kỳ
            "weight": current_weight,  # Weight đầu chu kỳ
            "contribution": contribution,  # Đóng góp W*E (chỉ > 0 nếu active)
            "last_update_cycle": current_cycle,
            # Lưu thêm trạng thái đầu vào để tiện debug/kiểm tra
            "avg_deviation": avg_dev,
            "metric_quality": metric_quality,
            "start_trust": validator_info.trust_score,
            "start_status": getattr(validator_info, "status", STATUS_ACTIVE),
        }

    # --- 3. Tính phần thưởng CƠ BẢN dự kiến cho từng validator (chỉ những ai active) ---
    logger.info(
        f":moneybag: Total validator contribution (Sum W*E from Active): [yellow]{total_validator_contribution:.4f}[/yellow]"
    )
    if total_validator_contribution > EPSILON:
        for validator_uid_hex, state in calculated_validator_states.items():
            # Chỉ tính thưởng cho validator active
            if state.get("start_status") == STATUS_ACTIVE:
                trust_for_reward = state["start_trust"]  # Dùng trust đầu chu kỳ
                # TÍNH REWARD GỐC Ở ĐÂY (NẾU CÓ)
                # Ví dụ: reward_from_fees = ... (logic tính phí)
                # state["reward"] = reward_from_fees
                # Tạm thời giả định reward ban đầu = 0 nếu chưa có logic khác
                if "reward" not in state:
                    state["reward"] = 0.0

                # Tính incentive (ví dụ, nếu có)
                reward_from_incentive = calculate_validator_incentive(
                    trust_score=trust_for_reward,
                    validator_weight=state["weight"],
                    validator_performance=state["E_v"],
                    total_validator_value=total_validator_contribution,
                    incentive_sigmoid_L=settings.CONSENSUS_PARAM_INCENTIVE_SIG_L,
                    incentive_sigmoid_k=settings.CONSENSUS_PARAM_INCENTIVE_SIG_K,
                    incentive_sigmoid_x0=settings.CONSENSUS_PARAM_INCENTIVE_SIG_X0,
                )
                state["reward"] += reward_from_incentive  # Cộng incentive vào reward
                logger.info(
                    f"  :dollar: Validator [cyan]{validator_uid_hex}[/cyan]: Calculated Base Reward (Incentive/Fees) = [green]{state['reward']:.6f}[/green]"
                )
            else:
                state["reward"] = 0.0  # Không có thưởng nếu không active
    else:
        logger.warning(
            ":warning: Total active validator contribution is zero. No base validator rewards calculated."
        )
        for state in calculated_validator_states.values():
            state["reward"] = 0.0

    # --- 4. TÍNH TOÁN VÀ PHÂN PHỐI TOKEN ISSUANCE MỚI (CHO VALIDATOR) ---
    logger.info(
        f":rocket: Calculating and distributing new token issuance for cycle {current_cycle}..."
    )
    # Ước tính slot bắt đầu của chu kỳ hiện tại để xác định epoch
    # Lưu ý: Đây là ước tính, có thể không chính xác 100% nếu độ dài slot thay đổi
    estimated_cycle_start_slot = (
        current_cycle - 1
    ) * settings.CONSENSUS_CYCLE_SLOT_LENGTH
    current_epoch = get_current_epoch(estimated_cycle_start_slot)
    logger.info(f"  Current estimated epoch for issuance: {current_epoch}")

    issuance_this_epoch = calculate_epoch_issuance(current_epoch)
    logger.info(
        f"  :money_mouth_face: Epoch {current_epoch}: Calculated new issuance: {issuance_this_epoch} raw units ({(issuance_this_epoch / (10**settings.TOKEN_DECIMALS)):.6f} MOD)"
    )

    if issuance_this_epoch > 0:
        issuance_distribution = distribute_issuance(
            issuance_this_epoch=issuance_this_epoch,
            calculated_states=calculated_validator_states,
            status_active_val=STATUS_ACTIVE,  # Đảm bảo dùng đúng hằng số STATUS_ACTIVE
        )

        # Cộng phần token mới được phát hành vào phần thưởng ('reward') của mỗi node
        for uid_hex, state in calculated_validator_states.items():
            issuance_share = issuance_distribution.get(
                uid_hex, 0
            )  # Lấy phần share (đơn vị nhỏ nhất)
            if issuance_share > 0:
                # Chuyển đổi share (int nhỏ nhất) thành giá trị float có thể cộng vào reward
                # Giả định 'reward' lưu giá trị đã scale (chia cho divisor)
                reward_from_issuance_scaled = (
                    issuance_share / settings.METAGRAPH_DATUM_INT_DIVISOR
                )

                # Cộng vào reward hiện có
                state["reward"] = state.get("reward", 0.0) + reward_from_issuance_scaled
                logger.info(
                    f"    :sparkles: UID [cyan]{uid_hex}[/cyan]: Issuance share = [blue]{issuance_share}[/blue] raw units -> Added [blue]{reward_from_issuance_scaled:.6f}[/blue] to total reward. New Total Reward: {state['reward']:.6f}"
                )
    else:
        logger.info("  No new tokens issued in this epoch.")
    # --- KẾT THÚC TÍCH HỢP TOKENOMICS ---

    # --- 5. TÍNH TOÁN TRẠNG THÁI VÀ INCENTIVE CHO MINER ---
    logger.info(f":gear: Calculating final states and incentives for miners...")
    # Ước tính total_system_value cho incentive (tổng W*P_adj của miner active)
    # Lưu ý: Giá trị này đã được tính trong prepare_miner_updates_logic trước đây
    #      Chúng ta cần tính nó ở đây để dùng cho calculate_miner_incentive.
    total_weighted_perf = sum(
        getattr(minfo, "weight", 0) * final_miner_scores.get(uid, 0)
        for uid, minfo in miners_info.items()
        if getattr(minfo, "status", STATUS_ACTIVE) == STATUS_ACTIVE
    )
    min_total_value = 1.0  # Hoặc settings.MIN_TOTAL_INCENTIVE_VALUE?
    total_system_value_for_miner_incentive = max(min_total_value, total_weighted_perf)
    logger.debug(
        f"Using total_system_value for miner incentive (Sum W*P_adj, min={min_total_value}): {total_system_value_for_miner_incentive:.4f}"
    )

    for miner_uid_hex, miner_info in miners_info.items():
        p_adj = final_miner_scores.get(miner_uid_hex, 0.0)
        trust_score_old = getattr(miner_info, "trust_score", 0.0)

        # Tính Trust Score mới cho miner
        time_since_miner_eval = 1  # Giả định
        score_for_trust_update = (
            p_adj
            if getattr(miner_info, "status", STATUS_ACTIVE) == STATUS_ACTIVE
            else 0.0
        )
        new_miner_trust_score = update_trust_score(
            trust_score_old,
            time_since_miner_eval,
            score_for_trust_update,
            # Các tham số trust có thể khác cho miner?
            delta_trust=settings.CONSENSUS_PARAM_DELTA_TRUST,
            alpha_base=settings.CONSENSUS_PARAM_ALPHA_BASE,
            k_alpha=settings.CONSENSUS_PARAM_K_ALPHA,
            update_sigmoid_L=settings.CONSENSUS_PARAM_UPDATE_SIG_L,
            update_sigmoid_k=settings.CONSENSUS_PARAM_UPDATE_SIG_K,
            update_sigmoid_x0=settings.CONSENSUS_PARAM_UPDATE_SIG_X0,
        )

        # Tính Incentive cho miner (dùng trust CŨ)
        miner_incentive_float = 0.0
        if getattr(miner_info, "status", STATUS_ACTIVE) == STATUS_ACTIVE:
            miner_incentive_float = calculate_miner_incentive(
                trust_score=trust_score_old,
                miner_weight=getattr(miner_info, "weight", 0.0),
                miner_performance_scores=[p_adj],
                total_system_value=total_system_value_for_miner_incentive,
                incentive_sigmoid_L=settings.CONSENSUS_PARAM_INCENTIVE_SIG_L,
                incentive_sigmoid_k=settings.CONSENSUS_PARAM_INCENTIVE_SIG_K,
                incentive_sigmoid_x0=settings.CONSENSUS_PARAM_INCENTIVE_SIG_X0,
            )
            logger.info(
                f"  :receipt: Miner [cyan]{miner_uid_hex}[/cyan]: Incentive = [green]{miner_incentive_float:.6f}[/green]"
            )

        # Lưu trạng thái dự kiến cho miner
        calculated_miner_states[miner_uid_hex] = {
            "P_adj": p_adj,
            "trust": new_miner_trust_score,
            "reward": miner_incentive_float,  # <<< REWARD CỦA MINER (HIỆN CHỈ LÀ INCENTIVE)
            "last_update_cycle": current_cycle,
            "start_trust": trust_score_old,
            "start_status": getattr(miner_info, "status", STATUS_ACTIVE),
            "notes": "Calculated in run_consensus_logic",
        }
    # --- KẾT THÚC TÍNH TOÁN CHO MINER ---

    logger.info(
        ":brain: Finished consensus calculations and state estimation for validators and miners."
    )
    return (
        final_miner_scores,
        calculated_validator_states,
        calculated_miner_states,
        issuance_this_epoch,
    )


# --- Logic Kiểm tra và Phạt Validator ---


async def verify_and_penalize_logic(
    current_cycle: int,
    previous_calculated_states: Dict[str, Any],  # Expected state from cycle N-1
    validators_info: Dict[
        str, ValidatorInfo
    ],  # Current state (start of cycle N), MODIFIED IN-PLACE
    context: BlockFrostChainContext,
    settings: Any,
    script_hash: ScriptHash,
    network: Network,
) -> None:
    """
    Verifies previous cycle's ValidatorDatum against on-chain data.
    Applies trust/status penalties IN-PLACE to the `validators_info` dictionary
    for validators found to have deviated.

    Args:
        current_cycle (int): The current cycle number (N).
        previous_calculated_states (Dict[str, Any]): Expected validator states calculated at the end of cycle N-1.
        validators_info (Dict[str, ValidatorInfo]): Dictionary of current validator info (start of cycle N).
                                                   This dictionary WILL BE MODIFIED IN-PLACE with penalties.
        context (BlockFrostChainContext): Cardano chain context.
        settings (Any): Application settings.
        script_hash (ScriptHash): The validator script hash.
        network (Network): The Cardano network.

    Returns:
        None: Modifies `validators_info` directly.

    Raises:
        ApiError: If fetching on-chain data from BlockFrost fails.
        Exception: For other unexpected errors during data fetching or comparison.
    """
    logger.info(
        f":mag: Verifying previous cycle ({current_cycle - 1}) validator updates..."
    )
    previous_cycle = current_cycle - 1
    if previous_cycle < 0:
        return

    # penalized_validator_datums: Dict[str, ValidatorDatum] = {}
    tolerance = settings.CONSENSUS_DATUM_COMPARISON_TOLERANCE
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    logger.debug(f"Verification tolerance (float): {tolerance}")

    try:
        # 1. Get on-chain data from the PREVIOUS cycle
        all_validator_results: List[Tuple[UTxO, Dict]] = await get_all_validator_data(
            context, script_hash, network
        )
        on_chain_states_decoded: Dict[str, Dict] = {}
        utxo_map_on_chain: Dict[str, UTxO] = {}
        for utxo_obj, datum_dict in all_validator_results:
            uid_hex = datum_dict.get("uid")
            last_update = datum_dict.get("last_update_slot")
            # Chỉ quan tâm đến datum được cập nhật ở chu kỳ trước đó
            if uid_hex and last_update == previous_cycle:
                on_chain_states_decoded[uid_hex] = datum_dict
                utxo_map_on_chain[uid_hex] = utxo_obj
        logger.info(
            f":inbox_tray: Found [cyan]{len(on_chain_states_decoded)}[/cyan] validator datums updated in cycle [yellow]{previous_cycle}[/yellow] for verification."
        )

        # 2. Compare with expected states
        expected_states = previous_calculated_states
        if not expected_states:
            logger.warning(
                ":warning: No expected validator states found from previous cycle."
            )
            return
        suspicious_validators: Dict[str, str] = {}
        for uid_hex, expected in expected_states.items():
            actual_decoded = on_chain_states_decoded.get(uid_hex)
            reason_parts = []
            if not actual_decoded:
                if expected.get("start_status") == STATUS_ACTIVE:
                    reason_parts.append(
                        f"Did not commit updates in cycle {previous_cycle}"
                    )
                if reason_parts:
                    suspicious_validators[uid_hex] = "; ".join(reason_parts)
                continue
            # Compare Trust Score (float)
            expected_trust_float = expected.get("trust", -1.0)
            actual_trust_float = actual_decoded.get("trust_score", -999.0)
            if actual_trust_float == -999.0:
                reason_parts.append("Trust score missing in on-chain data")
            else:
                diff_trust_float = abs(actual_trust_float - expected_trust_float)
                if diff_trust_float > tolerance:
                    reason_parts.append(
                        f"Trust mismatch (Expected: {expected_trust_float:.5f}, Actual: {actual_trust_float:.5f}, Diff: {diff_trust_float:.5f})"
                    )
            # Compare Performance Score (float)
            expected_perf_float = expected.get("E_v", -1.0)
            actual_perf_float = actual_decoded.get("last_performance", -999.0)
            if actual_perf_float == -999.0:
                reason_parts.append("Performance score missing in on-chain data")
            else:
                diff_perf_float = abs(actual_perf_float - expected_perf_float)
                if diff_perf_float > tolerance:
                    reason_parts.append(
                        f"Performance mismatch (Expected: {expected_perf_float:.5f}, Actual: {actual_perf_float:.5f}, Diff: {diff_perf_float:.5f})"
                    )

            if reason_parts:
                suspicious_validators[uid_hex] = "; ".join(reason_parts)
                logger.warning(
                    f":exclamation: Deviation detected for Validator [cyan]{uid_hex}[/cyan]: [yellow]{suspicious_validators[uid_hex]}[/yellow]"
                )

        # 3. Consensus on Fraud (Mocked)
        confirmed_deviators = suspicious_validators  # Mock

        # 4. Apply Penalties
        for uid_hex, reason in confirmed_deviators.items():
            validator_info = validators_info.get(uid_hex)
            if not validator_info:
                logger.warning(
                    f"Info for penalized validator {uid_hex} not found in current state."
                )
                continue

            logger.warning(
                f":hammer: Applying penalty IN MEMORY to Validator [cyan]{uid_hex}[/cyan] for: [yellow]{reason}[/yellow]"
            )

            # a. Determine Severity
            fraud_severity = _calculate_fraud_severity(reason, tolerance)

            # b. Tính lượng slash tiềm năng
            slash_amount = calculate_slash_amount(
                validator_info.stake,
                fraud_severity,
                settings.CONSENSUS_PARAM_MAX_SLASH_RATE,
            )
            if slash_amount > 0:
                logger.warning(
                    f":money_with_wings: Potential slash for [cyan]{uid_hex}[/cyan]: [red]{slash_amount / 1e6:.6f}[/red] ADA (Severity: {fraud_severity:.2f}). Needs trigger mechanism."
                )
                # TODO: Trigger Slashing Mechanism (Future/DAO)

            # c. Penalize Trust Score
            penalty_eta = settings.CONSENSUS_PARAM_PENALTY_ETA
            original_trust = validator_info.trust_score
            new_trust_score = max(
                0.0, original_trust * (1 - penalty_eta * fraud_severity)
            )
            if abs(new_trust_score - original_trust) > 1e-9:
                logger.warning(
                    f":arrow_down: Updating IN-MEMORY Trust Score for [cyan]{uid_hex}[/cyan]: [yellow]{original_trust:.4f}[/yellow] -> [red]{new_trust_score:.4f}[/red]"
                )
                validator_info.trust_score = new_trust_score
            else:
                logger.info(
                    f"In-memory trust score for {uid_hex} remains {original_trust:.4f}."
                )

            # d. Penalize Status
            new_status = validator_info.status
            jailed_threshold = getattr(
                settings, "CONSENSUS_JAILED_SEVERITY_THRESHOLD", 0.2
            )
            if (
                fraud_severity >= jailed_threshold
                and validator_info.status == STATUS_ACTIVE
            ):
                new_status = STATUS_JAILED
                logger.warning(
                    f":lock: Updating IN-MEMORY Status for [cyan]{uid_hex}[/cyan] to [bold red]JAILED[/bold red]."
                )
                validator_info.status = new_status

    except Exception as e:
        logger.exception(f"Error during validator verification/penalization: {e}")

    return


# --- Logic Chuẩn bị và Commit Cập nhật ---


async def prepare_miner_updates_logic(
    current_cycle: int,
    miners_info: Dict[str, MinerInfo],
    final_scores: Dict[str, float],
    settings: Any,
    current_utxo_map: Dict[str, UTxO],
    calculated_miner_states: Dict[str, Any],
) -> Dict[str, MinerDatum]:
    """
    Prepares new MinerDatum objects for committing to the blockchain.

    Calculates the next state for each miner based on consensus results:
    - Calculates new trust score using P_adj.
    - Retrieves pre-calculated reward (incentive) from calculated_miner_states.
    - Updates accumulated rewards.
    - Updates performance history and calculates its new hash.
    - Constructs the final MinerDatum object.

    Args:
        current_cycle (int): The current cycle number.
        miners_info (Dict[str, MinerInfo]): Miner info at the start of the cycle (or after penalties).
        final_scores (Dict[str, float]): Final adjusted performance scores (P_adj) for miners.
        settings (Any): Application settings.
        current_utxo_map (Dict[str, UTxO]): Map of UIDs to their UTxOs (needed for old rewards).
        calculated_miner_states (Dict[str, Any]): Pre-calculated miner states from consensus logic.

    Returns:
        Dict[str, MinerDatum]: A dictionary mapping miner UIDs (hex) to their new MinerDatum objects.

    Raises:
        ValueError: If datum CBOR decoding fails for existing datums.
        Exception: For errors during history hashing or MinerDatum creation.
    """
    logger.info(f":package: Preparing miner state updates for cycle {current_cycle}...")
    miner_updates: Dict[str, MinerDatum] = {}
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR

    for miner_uid_hex, miner_info in miners_info.items():
        log_prefix = f"Miner {miner_uid_hex}"
        logger.debug(f"{log_prefix}: Preparing update...")
        score_new = final_scores.get(miner_uid_hex, 0.0)  # P_adj
        trust_score_old = getattr(miner_info, "trust_score", 0.0)

        # Lấy trạng thái đã tính toán cho miner này
        calculated_state = calculated_miner_states.get(miner_uid_hex, {})
        if not calculated_state:
            logger.warning(f"{log_prefix}: No calculated state found. Skipping update.")
            continue

        # --- 1. Lấy Thông tin Cần Thiết từ Datum Cũ (Reward cũ) và Info ---
        pending_rewards_old = 0
        old_perf_history = list(getattr(miner_info, "performance_history", []))
        final_wallet_addr_hash = getattr(miner_info, "wallet_addr_hash", None)
        perf_hash_old_bytes = getattr(miner_info, "performance_history_hash", None)
        registration_slot_old = getattr(miner_info, "registration_slot", 0)

        input_utxo = current_utxo_map.get(miner_uid_hex)
        datum_cbor: Optional[bytes] = None
        if input_utxo and input_utxo.output.datum:
            raw_datum = input_utxo.output.datum
            # --- Stricter check: Only access .cbor if type is RawPlutusData ---
            if isinstance(raw_datum, RawPlutusData):
                datum_cbor = raw_datum.cbor  # type: ignore[attr-defined]
            else:
                # Log if datum is present but not the expected RawPlutusData type
                logger.warning(
                    f"{log_prefix}: Datum found for UTxO, but it's not RawPlutusData. Type: {type(raw_datum)}. Skipping datum decode."
                )
            # --------------------------------------------------------------------

        if datum_cbor:  # Only proceed if we successfully got CBOR bytes
            try:
                # Decode datum cũ chủ yếu để lấy reward cũ
                old_datum = MinerDatum.from_cbor(datum_cbor)
                pending_rewards_old = getattr(old_datum, "accumulated_rewards", 0)
                logger.debug(
                    f"{log_prefix}: Old accumulated_rewards from datum: {pending_rewards_old}"
                )
                # ... (rest of logic to potentially update fields from old_datum if missing in info)
                if not final_wallet_addr_hash:
                    final_wallet_addr_hash = getattr(
                        old_datum, "wallet_addr_hash", None
                    )
                if not perf_hash_old_bytes:
                    perf_hash_old_bytes = getattr(
                        old_datum, "performance_history_hash", None
                    )
                if registration_slot_old == 0:
                    registration_slot_old = getattr(old_datum, "registration_slot", 0)
            except Exception as e:
                logger.warning(
                    f"{log_prefix}: Could not decode old MinerDatum from CBOR: {e}. Using defaults (rewards=0)."
                )
        else:
            logger.warning(
                f"{log_prefix}: Old UTxO/Datum/CBOR not found. Assuming 0 old rewards."
            )
        # ----------------------------------------------------------

        # --- 2. Lấy Trust Score Mới (đã tính) ---
        new_trust_score_float = calculated_state.get(
            "trust", trust_score_old
        )  # Lấy từ state đã tính
        logger.debug(
            f"{log_prefix}: Using pre-calculated Trust: {new_trust_score_float:.4f}"
        )

        # --- 3. Lấy Incentive (Reward) (đã tính) ---
        calculated_reward = calculated_state.get("reward", 0.0)  # Lấy từ state đã tính
        logger.debug(
            f"{log_prefix}: Using pre-calculated Reward: {calculated_reward:.6f}"
        )

        # --- 4. Cập nhật Accumulated Rewards ---
        accumulated_rewards_new = pending_rewards_old + int(calculated_reward * divisor)
        logger.debug(
            f"{log_prefix}: AccumulatedRewards update: {pending_rewards_old} -> {accumulated_rewards_new}"
        )

        # --- 5. Cập nhật Performance History & Hash ---
        updated_history = old_perf_history
        updated_history.append(score_new)
        max_len = settings.CONSENSUS_MAX_PERFORMANCE_HISTORY_LEN
        updated_history = updated_history[-max_len:]

        perf_history_hash_new_bytes: bytes = EMPTY_HASH_BYTES  # Default to empty bytes
        if updated_history:
            try:
                hashed = hash_data(updated_history)
                if isinstance(hashed, bytes):
                    perf_history_hash_new_bytes = hashed
                    logger.debug(f"{log_prefix}: New performance history hash created.")
                else:
                    logger.error(
                        f"{log_prefix}: hash_data did not return bytes for history. Using old hash."
                    )
                    perf_history_hash_new_bytes = (
                        perf_hash_old_bytes
                        if isinstance(perf_hash_old_bytes, bytes)
                        else EMPTY_HASH_BYTES
                    )
            except Exception as hash_e:
                logger.error(
                    f"{log_prefix}: Failed to hash performance history: {hash_e}"
                )
                perf_history_hash_new_bytes = (
                    perf_hash_old_bytes
                    if isinstance(perf_hash_old_bytes, bytes)
                    else EMPTY_HASH_BYTES
                )
        # -----------------------------------------

        # --- 6. Lấy các giá trị tĩnh khác từ MinerInfo ---
        api_endpoint_str = getattr(miner_info, "api_endpoint", None)
        api_endpoint_bytes = (
            api_endpoint_str.encode("utf-8")
            if api_endpoint_str
            else b""  # Default to empty bytes
        )
        current_status = getattr(miner_info, "status", STATUS_ACTIVE)  # Status hiện tại
        registration_slot = registration_slot_old  # Giữ slot đăng ký gốc
        subnet_uid = getattr(miner_info, "subnet_uid", 0)
        stake = int(getattr(miner_info, "stake", 0))
        # ---------------------------------------------

        # --- 7. Tạo MinerDatum mới ---
        try:
            uid_bytes = bytes.fromhex(miner_uid_hex)

            # Ensure wallet_addr_hash is bytes, default to empty bytes if None
            final_wallet_addr_hash_bytes = (
                final_wallet_addr_hash
                if isinstance(final_wallet_addr_hash, bytes)
                else EMPTY_HASH_BYTES
            )

            new_datum = MinerDatum(
                uid=uid_bytes,
                subnet_uid=subnet_uid,
                stake=stake,
                scaled_last_performance=int(score_new * divisor),
                scaled_trust_score=int(new_trust_score_float * divisor),
                accumulated_rewards=accumulated_rewards_new,
                last_update_slot=current_cycle,
                performance_history_hash=perf_history_hash_new_bytes,  # Pass bytes
                wallet_addr_hash=final_wallet_addr_hash_bytes,  # Pass bytes
                status=current_status,
                registration_slot=registration_slot,
                api_endpoint=api_endpoint_bytes,  # Pass bytes
            )
            miner_updates[miner_uid_hex] = new_datum
            logger.debug(f"{log_prefix}: Successfully prepared new MinerDatum.")

        except ValueError as hex_err:
            logger.error(
                f"{log_prefix}: Invalid UID format, cannot convert from hex: {hex_err}"
            )
        except Exception as e:
            logger.error(
                f"{log_prefix}: Failed to create MinerDatum: {e}", exc_info=True
            )

    logger.info(
        f":package: Prepared [cyan]{len(miner_updates)}[/cyan] miner datums for update."
    )
    return miner_updates


async def prepare_validator_updates_logic(
    current_cycle: int,
    self_validator_info: ValidatorInfo,
    calculated_states: Dict[str, Any],
    settings: Any,
    context: Optional[
        BlockFrostChainContext
    ],  # Context might be needed for future extensions
) -> Dict[str, ValidatorDatum]:
    """
    Prepares the ValidatorDatum update specifically for the validator running this node.

    Uses the pre-calculated state for this validator (from `run_consensus_logic`)
    and combines it with potentially updated trust/status (from `verify_and_penalize_logic`,
    reflected in `self_validator_info`) to create the final Datum.

    Args:
        current_cycle (int): The current cycle number.
        self_validator_info (ValidatorInfo): The current info for this validator
                                           (potentially updated with penalties).
        calculated_states (Dict[str, Any]): The dictionary of calculated next states
                                           from `run_consensus_logic`.
        settings (Any): Application settings.
        context (Optional[BlockFrostChainContext]): Cardano context (may be unused currently).

    Returns:
        Dict[str, ValidatorDatum]: A dictionary containing the single update for this validator,
                                   mapping its UID (hex) to its new ValidatorDatum.
                                   Returns an empty dict if the update cannot be prepared.
    Raises:
        Exception: If errors occur during Datum creation.
    """
    logger.info(
        f":pencil2: Preparing self validator state update for cycle {current_cycle}..."
    )
    validator_updates: Dict[str, ValidatorDatum] = {}
    self_uid_hex = self_validator_info.uid
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR

    if self_uid_hex not in calculated_states:
        logger.warning(
            f":warning: Calculated state for self validator [cyan]{self_uid_hex}[/cyan] not found."
        )
        return {}

    state = calculated_states[self_uid_hex]

    # Vì trust/status có thể đã bị thay đổi bởi verify_and_penalize_logic
    current_trust_float = self_validator_info.trust_score
    current_status = self_validator_info.status

    # Lấy thông tin tĩnh/ít thay đổi từ self_validator_info
    stake_current = int(self_validator_info.stake)
    subnet_uid_current = getattr(self_validator_info, "subnet_uid", 0)
    registration_slot_current = getattr(self_validator_info, "registration_slot", 0)
    validator_address_str = self_validator_info.address
    api_endpoint_current = self_validator_info.api_endpoint
    status_current = getattr(self_validator_info, "status", STATUS_ACTIVE)

    # Hash địa chỉ thay vì dùng bytes gốc
    wallet_addr_hash_bytes = getattr(self_validator_info, "wallet_addr_hash", None)
    if not isinstance(wallet_addr_hash_bytes, bytes):
        wallet_addr_hash_bytes = hashlib.sha256(
            validator_address_str.encode("utf-8")
        ).digest()

    # Mã hóa endpoint string thành bytes UTF-8, không hash nữa
    api_endpoint_bytes_utf8 = b""  # Default là bytes rỗng
    if api_endpoint_current and isinstance(api_endpoint_current, str):
        try:
            api_endpoint_bytes_utf8 = api_endpoint_current.encode("utf-8")
            logger.debug(
                f"Self-update: Encoding endpoint '{api_endpoint_current}' to bytes."
            )
        except Exception as enc_err:
            logger.error(
                f"Self-update: Failed to encode endpoint '{api_endpoint_current}': {enc_err}. Using empty bytes."
            )
            api_endpoint_bytes_utf8 = b""
    elif api_endpoint_current:  # Nếu không phải string hợp lệ
        logger.warning(
            f"Self-update: api_endpoint is not a valid string ('{type(api_endpoint_current)}'). Using empty bytes."
        )
        api_endpoint_bytes_utf8 = b""

    # Lấy performance history hash, default to empty bytes
    perf_history_hash_bytes = getattr(
        self_validator_info, "performance_history_hash", None
    )
    if not isinstance(perf_history_hash_bytes, bytes):
        perf_history_hash_bytes = EMPTY_HASH_BYTES

    # Lấy accumulated_rewards cũ
    pending_rewards_old = int(state.get("accumulated_rewards_old", 0))

    # Tính phần thưởng mới tích lũy
    calculated_reward = state.get("reward", 0.0)
    accumulated_rewards_new = pending_rewards_old + int(calculated_reward * divisor)

    # Tạo ValidatorDatum mới
    try:
        new_perf_float = state.get("E_v", 0.0)
        new_trust_float = state.get("trust", 0.0)

        new_datum = ValidatorDatum(
            uid=bytes.fromhex(self_uid_hex),
            subnet_uid=subnet_uid_current,
            stake=stake_current,
            scaled_last_performance=int(new_perf_float * divisor),
            scaled_trust_score=int(current_trust_float * divisor),
            accumulated_rewards=accumulated_rewards_new,
            last_update_slot=current_cycle,
            performance_history_hash=perf_history_hash_bytes,  # Pass bytes
            wallet_addr_hash=wallet_addr_hash_bytes,  # Pass bytes
            status=current_status,
            registration_slot=registration_slot_current,
            api_endpoint=api_endpoint_bytes_utf8,  # <<< GÁN BYTES UTF-8 VÀO ĐÂY
        )
        validator_updates[self_uid_hex] = new_datum
        logger.info(
            f":white_check_mark: Prepared update for self ([cyan]{self_uid_hex}[/cyan])"
        )
    except Exception as e:
        logger.exception(
            f":x: Failed to create ValidatorDatum for self ([cyan]{self_uid_hex}[/cyan]): {e}"
        )

    return validator_updates


async def commit_updates_logic(
    validator_updates: Dict[str, ValidatorDatum],  # Should contain only the self-update
    current_utxo_map: Dict[
        str, UTxO
    ],  # Map from uid_hex -> UTxO object at start of cycle
    context: BlockFrostChainContext,
    signing_key: ExtendedSigningKey,  # Owner's (node runner's) payment signing key
    stake_signing_key: Optional[ExtendedSigningKey],  # Owner's stake key, if any
    settings: Any,  # Full settings object
    script_hash: ScriptHash,
    script_bytes: PlutusV3Script,
    network: Network,
) -> Dict[str, Any]:
    """
    Builds and submits the transaction to commit the self-validator update.

    Takes the prepared ValidatorDatum update for the node itself, finds its
    corresponding input UTxO from the `current_utxo_map`, constructs a
    transaction spending that UTxO and creating a new one at the script address
    with the new Datum, signs it with the owner's keys, and submits it.

    Args:
        validator_updates (Dict[str, ValidatorDatum]): Dictionary containing the single
                                                     self-update datum, keyed by UID hex.
        current_utxo_map (Dict[str, UTxO]): Map of UIDs to their UTxOs at the start of the cycle.
        context (BlockFrostChainContext): Cardano chain context.
        signing_key (ExtendedSigningKey): Payment key of the node operator (owner).
        stake_signing_key (Optional[ExtendedSigningKey]): Stake key of the node operator.
        settings (Any): Application settings.
        script_hash (ScriptHash): Hash of the validator Plutus script.
        script_bytes (PlutusV3Script): The compiled Plutus script.
        network (Network): The Cardano network.

    Returns:
        Dict[str, Any]: A dictionary summarizing the commit attempt, including:
                        'status': 'completed', 'completed_with_errors', 'completed_with_skips', 'failed'.
                        'submitted_count': Number of transactions submitted (0 or 1).
                        'failed_count': Number of failures (0 or 1).
                        'skipped_count': Number skipped due to missing input UTxO (0 or 1).
                        'submitted_txs': Dict mapping internal ID to submitted TxID string.
                        'failures': Dict mapping UID to error message for failures.
                        'skips': Dict mapping UID to reason for skipping.

    Raises:
        ApiError: If Blockfrost submission fails.
        Exception: For unexpected errors during transaction building or signing.
    """
    logger.info(
        f":link: Starting blockchain commit process (SELF Validator Update Only)..."
    )

    # Kiểm tra xem có bản cập nhật nào cần commit không (chỉ là self-update)
    if not validator_updates:
        logger.info(
            ":information_source: No self validator update prepared for this cycle."
        )
        return {
            "status": "completed_no_updates",
            "submitted_count": 0,
            "failed_count": 0,
            "skipped_count": 0,
        }

    # --- Lấy thông tin Owner (Validator đang chạy node) ---
    try:
        owner_payment_vkey = signing_key.to_verification_key()
        owner_payment_key_hash: VerificationKeyHash = owner_payment_vkey.hash()  # type: ignore
        owner_stake_key_hash: Optional[VerificationKeyHash] = None
        if stake_signing_key:
            owner_stake_vkey = stake_signing_key.to_verification_key()
            owner_stake_key_hash = owner_stake_vkey.hash()  # type: ignore

        owner_address = Address(
            payment_part=owner_payment_key_hash,
            staking_part=owner_stake_key_hash,
            network=network,
        )
        logger.info(f"Commit Owner Address: {owner_address}")
    except Exception as e:
        logger.exception(
            f":stop_sign: [bold red]Failed to derive owner address/keys:[/bold red] {e}. Aborting commit."
        )
        return {"status": "failed", "reason": "Owner key/address derivation failed."}

    # Địa chỉ contract và Redeemer mặc định
    contract_address = Address(payment_part=script_hash, network=network)
    # Giả định Tag 0 là cho validator update (cần khớp với script)
    default_redeemer = Redeemer(0)
    logger.debug(f"Contract Address: {contract_address}")
    logger.debug(f"Using Redeemer for Validator Update: {default_redeemer}")

    submitted_tx_ids: Dict[str, str] = {}
    failed_updates: Dict[str, str] = {}
    skipped_updates: Dict[str, str] = {}

    # --- Chỉ xử lý bản cập nhật duy nhất (self-update) ---
    # Lấy UID và Datum mới từ dict (chỉ nên có 1 key là UID của validator này)
    if len(validator_updates) > 1:
        logger.warning(
            f"Expected only one self-update in validator_updates, but found {len(validator_updates)}. Processing the first one found..."
        )

    # Lấy UID và Datum từ phần tử đầu tiên (hoặc duy nhất)
    try:
        self_uid_hex, new_datum = next(iter(validator_updates.items()))
    except StopIteration:
        logger.error(
            "validator_updates dictionary is unexpectedly empty after initial check."
        )
        return {"status": "failed", "reason": "validator_updates became empty."}

    log_prefix = f"CommitSelf (Validator {self_uid_hex})"
    logger.debug(f"{log_prefix}: Processing...")

    # 1. Lấy Input UTxO từ map cho chính mình
    input_utxo = current_utxo_map.get(self_uid_hex)
    if not input_utxo:
        error_msg = "Input UTxO not found in initial map for self-update"
        logger.error(f"{log_prefix}: Skipped - {error_msg}")
        skipped_updates[self_uid_hex] = error_msg
        # Trả về kết quả ngay vì đây là bước quan trọng duy nhất
        return {
            "status": "completed_with_skips",
            "submitted_count": 0,
            "failed_count": 0,
            "skipped_count": 1,
            "submitted_txs": {},
            "failures": {},
            "skips": skipped_updates,
        }

    logger.debug(f"{log_prefix}: Found Input UTxO for self: {input_utxo.input}")

    # 2. Xây dựng Giao dịch cho self-update
    try:
        builder = TransactionBuilder(context=context)

        # a. Thêm Input Script UTxO (UTXO cũ của validator)
        builder.add_script_input(
            utxo=input_utxo,
            script=script_bytes,
            redeemer=default_redeemer,  # Redeemer cho validator update
        )
        logger.debug(f"{log_prefix}: Added script input: {input_utxo.input}")

        # b. Thêm Output mới (trả về contract với datum mới)
        #    Giữ nguyên giá trị (coin + multi-asset) của input UTxO
        output_value: Value = input_utxo.output.amount
        builder.add_output(
            TransactionOutput(
                address=contract_address,
                amount=output_value,
                datum=new_datum,  # Datum validator mới của chính mình
            )
        )
        logger.debug(
            f"{log_prefix}: Added script output with new datum (Amount: {output_value.coin} Lovelace)"
        )

        # c. Thêm Input từ ví Owner để trả phí và làm collateral
        builder.add_input_address(owner_address)
        logger.debug(f"{log_prefix}: Added owner address input: {owner_address}")

        # d. Chỉ định người ký cần thiết (là hash của payment key của owner)
        builder.required_signers = [owner_payment_key_hash]
        logger.debug(
            f"{log_prefix}: Set required signer: {owner_payment_key_hash.to_primitive().hex()}"
        )

        # e. Build và Ký Giao dịch
        logger.debug(f"{log_prefix}: Building and signing transaction...")
        # Chỉ cần payment key của owner (trừ khi script yêu cầu stake key)
        signing_keys_list: List = [signing_key]
        # if stake_signing_key and owner_stake_key_hash in builder.required_signers:
        #    signing_keys_list.append(stake_signing_key)

        signed_tx = builder.build_and_sign(
            signing_keys=signing_keys_list,  # type: ignore
            change_address=owner_address,
        )
        logger.debug(
            f"{log_prefix}: Transaction built and signed. Fee: {signed_tx.transaction_body.fee}"
        )

    except Exception as build_e:
        logger.exception(
            f":hammer: {log_prefix}: Failed during transaction build/sign phase: {build_e}"
        )
        failed_updates[self_uid_hex] = f"Build/Sign Error: {str(build_e)}"
        # Trả về kết quả lỗi
        return {
            "status": "completed_with_errors",
            "submitted_count": 0,
            "failed_count": 1,
            "skipped_count": 0,
            "submitted_txs": {},
            "failures": failed_updates,
            "skips": {},
        }

    # 3. Submit Giao dịch self-update
    tx_id_str: Optional[str] = None
    try:
        logger.info(
            f":arrow_up: {log_prefix}: Submitting self-update transaction to the blockchain..."
        )
        # submit_tx của BlockFrostContext trả về TransactionId
        tx_id: TransactionId = context.submit_tx(signed_tx)  # type: ignore
        tx_id_str = str(tx_id)
        logger.info(
            f":white_check_mark: {log_prefix}: Successfully submitted self-update! TxID: [yellow]{tx_id_str}[/yellow]"
        )
        submitted_tx_ids[f"validator_{self_uid_hex}"] = tx_id_str

    except ApiError as e:
        error_msg = f"Blockfrost API Error ({e.status_code}): {e.message}"
        logger.error(
            f":x: {log_prefix}: Blockfrost API Error on submit: Status=[red]{e.status_code}[/red], Message=[yellow]{e.message}[/yellow]",
            exc_info=False,
        )
        failed_updates[self_uid_hex] = error_msg
    except Exception as e:
        error_msg = f"Submit Error: {str(e)}"
        logger.exception(
            f":rotating_light: {log_prefix}: Generic error during transaction submission: {e}"
        )
        failed_updates[self_uid_hex] = error_msg

    # --- Tổng kết ---
    total_submitted = len(submitted_tx_ids)
    total_failed = len(failed_updates)
    total_skipped = len(skipped_updates)  # Sẽ là 0 hoặc 1 ở đây
    logger.info(
        f":checkered_flag: Validator Self-Commit process finished. Submitted: [green]{total_submitted}[/green], Failed: [red]{total_failed}[/red], Skipped: [yellow]{total_skipped}[/yellow]"
    )
    if failed_updates:
        logger.warning(f":warning: Failed self-update details: {failed_updates}")
    if skipped_updates:
        logger.warning(f":warning: Skipped self-update details: {skipped_updates}")

    return {
        "status": (
            "completed"
            if total_submitted == 1
            else (
                "completed_with_errors" if total_failed == 1 else "completed_with_skips"
            )
        ),
        "submitted_count": total_submitted,
        "failed_count": total_failed,
        "skipped_count": total_skipped,
        "submitted_txs": submitted_tx_ids,
        "failures": failed_updates,
        "skips": skipped_updates,
    }
