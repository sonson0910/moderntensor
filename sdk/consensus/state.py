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
    calculate_fraud_severity_value,
    calculate_slash_amount,
    calculate_miner_incentive,
    calculate_validator_incentive,
    calculate_consensus_score,  # Import mới
    calculate_validator_consensus_weight,  # Import mới
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

from sdk.network.hydra_client import HydraClient  # Import Hydra Client

EPSILON = 1e-9
logger = logging.getLogger(__name__)

# Global Hydra Client instance (lazy init)
hydra_client = None

async def get_hydra_client():
    global hydra_client
    if hydra_client is None:
        # Lấy URL từ settings hoặc dùng default
        hydra_url = getattr(settings, "HYDRA_NODE_URL", "ws://localhost:4001")
        hydra_client = HydraClient(hydra_node_url=hydra_url)
    return hydra_client

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
    (Placeholder/Mock) Finds a UTXO at the script address containing a Datum with a matching UID.

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
    settings: Any,
    consensus_possible: bool,
    self_validator_uid: str,
) -> Tuple[Dict[str, float], Dict[str, Any], Dict[str, float]]:
    """
    Performs the core consensus calculations for a cycle.

    Calculates adjusted miner performance scores (P_adj) based on received validator scores
    and trust. Then, calculates the expected next state (performance E_v, trust score,
    potential reward, contribution) for each validator based on their participation,
    deviations, and historical performance.

    If `consensus_possible` is False, it skips most calculations and only applies trust decay.

    Args:
        current_cycle (int): The current cycle number.
        tasks_sent (Dict[str, TaskAssignment]): Tasks sent out during this cycle.
        received_scores (Dict[str, Dict[str, ValidatorScore]]): Scores received from peer validators.
        validators_info (Dict[str, ValidatorInfo]): Validator info at the start of the cycle.
        settings (Any): Application settings object.
        consensus_possible (bool): Whether enough P2P scores were received for full consensus.
        self_validator_uid (str): UID of the validator running this logic.

    Returns:
        Tuple[Dict[str, float], Dict[str, Any], Dict[str, float]]: A tuple containing:
            - Final miner adjusted scores ({miner_uid_hex: P_adj}).
            - Calculated next validator states ({validator_uid_hex: {state_dict}}).
            - Miner penalties ({miner_uid_hex: penalty_score}).
    """
    logger.info(f":brain: Running consensus calculations for cycle {current_cycle}...")
    final_miner_scores: Dict[str, float] = {}  # {miner_uid_hex: P_adj}
    miner_penalties: Dict[str, float] = defaultdict(float) # {miner_uid_hex: penalty_score}
    validator_deviations: Dict[str, List[float]] = defaultdict(
        list
    )  # {validator_uid_hex: [deviation1, deviation2,...]}
    calculated_validator_states: Dict[str, Any] = {}  # {validator_uid_hex: {state}}
    total_validator_contribution: float = 0.0  # Tổng W*E để tính thưởng validator
    if not consensus_possible:
        logger.warning(
            f":warning: Cycle {current_cycle}: Insufficient P2P scores received. Skipping detailed consensus. Applying only trust decay."
        )
        # Chỉ tính decay cho trust score, không tính P_adj, E_v, reward
        for validator_uid_hex, validator_info in validators_info.items():
            time_since_val_eval = 1  # Giả định 1 chu kỳ
            # Tính trust chỉ với decay (score_new = 0)
            new_val_trust_score = update_trust_score(
                validator_info.trust_score,
                time_since_val_eval,
                0.0,
                delta_trust=settings.CONSENSUS_PARAM_DELTA_TRUST,
                # Các tham số alpha, k_alpha, sigmoid không ảnh hưởng khi score_new=0
                alpha_base=settings.CONSENSUS_PARAM_ALPHA_BASE,
                k_alpha=settings.CONSENSUS_PARAM_K_ALPHA,
                update_sigmoid_L=settings.CONSENSUS_PARAM_UPDATE_SIG_L,
                update_sigmoid_k=settings.CONSENSUS_PARAM_UPDATE_SIG_K,
                update_sigmoid_x0=settings.CONSENSUS_PARAM_UPDATE_SIG_X0,
            )
            # Lưu trạng thái tối thiểu
            calculated_validator_states[validator_uid_hex] = {
                "E_v": getattr(validator_info, "last_performance", 0.0),  # Giữ E_v cũ
                "trust": new_val_trust_score,  # Chỉ có decay
                "reward": 0.0,  # Không có reward
                "weight": validator_info.weight,
                "contribution": 0.0,
                "last_update_cycle": current_cycle,
                "start_trust": validator_info.trust_score,
                "start_status": validator_info.status,
                "notes": "Consensus skipped due to insufficient scores.",
            }
            # final_miner_scores vẫn rỗng

        return (
            final_miner_scores,
            calculated_validator_states,
            miner_penalties,
        )  # Trả về kết quả rỗng/chỉ decay

    # --- 1. Tính điểm đồng thuận Miner (Global Consensus - Yuma Style) ---
    # Chuẩn bị dữ liệu cho tính toán đồng thuận
    miner_scores_map: Dict[str, Dict[str, float]] = defaultdict(dict) # {miner_uid: {validator_uid: score}}
    validator_scores_map: Dict[str, Dict[str, float]] = defaultdict(dict) # {validator_uid: {miner_uid: score}}
    validator_stakes: Dict[str, float] = {
        uid: v.stake for uid, v in validators_info.items()
        if getattr(v, "status", STATUS_ACTIVE) == STATUS_ACTIVE
    }
    total_active_stake = sum(validator_stakes.values())

    # Gom điểm từ received_scores
    for task_id, validator_scores_dict in received_scores.items():
        first_score = next(iter(validator_scores_dict.values()), None)
        if not first_score:
            continue
        miner_uid_hex = first_score.miner_uid

        for validator_uid_hex, score_entry in validator_scores_dict.items():
            # Chỉ nhận điểm từ validator active
            if validator_uid_hex in validator_stakes:
                miner_scores_map[miner_uid_hex][validator_uid_hex] = score_entry.score
                validator_scores_map[validator_uid_hex][miner_uid_hex] = score_entry.score

                # Accumulate penalties
                if getattr(score_entry, "penalty", 0.0) > 0:
                     stake = validator_stakes[validator_uid_hex]
                     miner_penalties[miner_uid_hex] += stake * score_entry.penalty

    # Normalize penalties
    if total_active_stake > 0:
        for miner_uid in miner_penalties:
            miner_penalties[miner_uid] /= total_active_stake
            if miner_penalties[miner_uid] > 0.5: # Threshold > 50% stake flags penalty
                logger.warning(f"Miner {miner_uid} flagged for slashing! Penalty Score: {miner_penalties[miner_uid]:.4f}")

    # Tính điểm đồng thuận cho từng Miner (P_adj)
    for miner_uid_hex, scores_dict in miner_scores_map.items():
        # Sử dụng cơ chế đồng thuận theo trọng số Stake (Yuma-like)
        consensus_score = calculate_consensus_score(scores_dict, validator_stakes)

        # Apply penalty if significant
        if miner_penalties.get(miner_uid_hex, 0.0) > 0.5:
            consensus_score = 0.0 # Force score to 0 if penalized

        final_miner_scores[miner_uid_hex] = consensus_score

        logger.info(
            f"  :chart_with_upwards_trend: Global Consensus Score for Miner [cyan]{miner_uid_hex}[/cyan]: [yellow]{consensus_score:.4f}[/yellow]"
        )

    # Tính hiệu suất đồng thuận cho từng Validator (E_v)
    # Thay vì tính deviation thủ công, ta dùng hàm calculate_validator_consensus_weight
    validator_consensus_weights: Dict[str, float] = {}
    for validator_uid_hex, scores_dict in validator_scores_map.items():
        # So sánh điểm validator chấm với điểm đồng thuận chung
        weight = calculate_validator_consensus_weight(
            scores_dict,
            final_miner_scores,
            sigma=settings.CONSENSUS_PARAM_SIGMA if hasattr(settings, 'CONSENSUS_PARAM_SIGMA') else 0.5
        )
        validator_consensus_weights[validator_uid_hex] = weight
        logger.debug(f"  Validator {validator_uid_hex} Consensus Weight: {weight:.4f}")

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

        # --- Cải tiến: Sử dụng Consensus Weight làm E_validator ---
        # Thay vì tính toán phức tạp với deviation thủ công, ta dùng trọng số đồng thuận
        # đã được tính toán dựa trên độ tương đồng với toàn mạng lưới (Yuma-like)
        new_e_validator = validator_consensus_weights.get(validator_uid_hex, 0.0)

        logger.info(
            f"  :chart_with_downwards_trend: Consensus Performance (E_val) for Validator [cyan]{validator_uid_hex}[/cyan]: [yellow]{new_e_validator:.4f}[/yellow]"
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
            "start_trust": validator_info.trust_score,
            "start_status": getattr(validator_info, "status", STATUS_ACTIVE),
        }

    # --- 3. Tính phần thưởng dự kiến cho từng validator (chỉ những ai active) ---
    logger.info(
        f":moneybag: Total validator contribution (Sum W*E from Active): [yellow]{total_validator_contribution:.4f}[/yellow]"
    )
    if total_validator_contribution > EPSILON:
        for validator_uid_hex, state in calculated_validator_states.items():
            # Chỉ tính thưởng cho validator active
            if state.get("start_status") == STATUS_ACTIVE:
                trust_for_reward = state["start_trust"]  # Dùng trust đầu chu kỳ
                reward = calculate_validator_incentive(
                    trust_score=trust_for_reward,
                    validator_weight=state["weight"],  # Weight đầu chu kỳ
                    validator_performance=state["E_v"],  # E_v mới tính
                    total_validator_value=total_validator_contribution,  # Tổng contribution của những người active
                    incentive_sigmoid_L=settings.CONSENSUS_PARAM_INCENTIVE_SIG_L,
                    incentive_sigmoid_k=settings.CONSENSUS_PARAM_INCENTIVE_SIG_K,
                    incentive_sigmoid_x0=settings.CONSENSUS_PARAM_INCENTIVE_SIG_X0,
                )
                state["reward"] = reward  # Thêm phần thưởng vào trạng thái dự kiến
                logger.info(
                    f"  :dollar: Validator [cyan]{validator_uid_hex}[/cyan]: Calculated Reward = [green]{reward:.6f}[/green]"
                )
            else:
                state["reward"] = 0.0  # Không có thưởng nếu không active
    else:
        logger.warning(
            ":warning: Total active validator contribution is zero. No validator rewards calculated."
        )
        for state in calculated_validator_states.values():
            state["reward"] = 0.0

    logger.info(
        ":brain: Finished consensus calculations and validator state estimation."
    )
    return final_miner_scores, calculated_validator_states


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
) -> Dict[str, str]:
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
        Dict[str, str]: A dictionary of {validator_uid: reason} for penalized validators.

    Raises:
        ApiError: If fetching on-chain data from BlockFrost fails.
        Exception: For other unexpected errors during data fetching or comparison.
    """
    logger.info(
        f":mag: Verifying previous cycle ({current_cycle - 1}) validator updates..."
    )
    previous_cycle = current_cycle - 1
    suspicious_validators: Dict[str, str] = {}
    if previous_cycle < 0:
        return suspicious_validators

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
 suspicious_validators
    return


# --- Logic Chuẩn bị và Commit Cập nhật ---


async def prepare_miner_updates_logic(  # <<<--- async vì cần lấy/decode datum cũ
    current_cycle: int,
    miners_info: Dict[str, MinerInfo],  # Input miner state (start of cycle)
    final_scores: Dict[str, float],  # Adjusted performance scores (P_adj)
    settings: Any,
    # context: BlockFrostChainContext, # Optional if utxo_map has decoded datum
    current_utxo_map: Dict[str, UTxO],  # Map uid_hex -> UTxO object
    miner_penalties: Dict[str, float] = None, # Optional penalties
) -> Dict[str, MinerDatum]:
    """
    Prepares new MinerDatum objects for committing to the blockchain.

    Calculates the next state for each miner based on consensus results:
    - Calculates new trust score using P_adj.
    - Calculates new incentive based on P_adj and old trust score.
    - Updates accumulated rewards.
    - Updates performance history and calculates its new hash.
    - Constructs the final MinerDatum object.

    Args:
        current_cycle (int): The current cycle number.
        miners_info (Dict[str, MinerInfo]): Miner info at the start of the cycle (or after penalties).
        final_scores (Dict[str, float]): Final adjusted performance scores (P_adj) for miners.
        settings (Any): Application settings.
        current_utxo_map (Dict[str, UTxO]): Map of UIDs to their UTxOs (needed for old rewards).
        miner_penalties (Dict[str, float]): Optional map of miner penalties.

    Returns:
        Dict[str, MinerDatum]: A dictionary mapping miner UIDs (hex) to their new MinerDatum objects.

    Raises:
        ValueError: If datum CBOR decoding fails for existing datums.
        Exception: For errors during history hashing or MinerDatum creation.
    """
    logger.info(f":package: Preparing miner state updates for cycle {current_cycle}...")
    miner_updates: Dict[str, MinerDatum] = {}
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    if miner_penalties is None:
        miner_penalties = {}

    # Ước tính total_system_value cho incentive (tổng W*P_adj của miner active)
    total_weighted_perf = sum(
        getattr(minfo, "weight", 0) * final_scores.get(uid, 0)
        for uid, minfo in miners_info.items()
        if getattr(minfo, "status", STATUS_ACTIVE) == STATUS_ACTIVE
    )
    # Đặt giá trị tối thiểu để tránh chia cho 0 và incentive quá lớn khi mạng nhỏ
    min_total_value = 1.0  # Hoặc một giá trị phù hợp khác
    total_system_value = max(min_total_value, total_weighted_perf)
    logger.debug(
        f"Using total_system_value (Sum W*P_adj, min={min_total_value}): {total_system_value:.4f}"
    )

    for miner_uid_hex, miner_info in miners_info.items():
        log_prefix = f"Miner {miner_uid_hex}"
        logger.debug(f"{log_prefix}: Preparing update...")
        score_new = final_scores.get(miner_uid_hex, 0.0)  # P_adj
        trust_score_old = getattr(
            miner_info, "trust_score", 0.0
        )

        # Apply penalty to trust score if needed
        penalty = miner_penalties.get(miner_uid_hex, 0.0)
        if penalty > 0.5:
            logger.warning(f"{log_prefix}: Applying penalty {penalty} to trust score.")
            trust_score_old = 0.0 # Slash trust to 0

        # --- 1. Lấy Thông tin Cần Thiết từ Datum Cũ (Chủ yếu là Reward) ---
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

        # --- 2. Tính Trust Score Mới ---
        time_since_eval = 1  # Giả định được đánh giá mỗi chu kỳ nếu active
        # Chỉ cập nhật trust dựa trên điểm mới nếu miner đang active
        score_for_trust_update = (
            score_new
            if getattr(miner_info, "status", STATUS_ACTIVE) == STATUS_ACTIVE
            else 0.0
        )
        new_trust_score_float = update_trust_score(
            trust_score_old=trust_score_old,
            time_since_last_eval=time_since_eval,
            score_new=score_for_trust_update,  # Dùng P_adj hoặc 0
            # Lấy các tham số từ settings
            delta_trust=settings.CONSENSUS_PARAM_DELTA_TRUST,
            alpha_base=settings.CONSENSUS_PARAM_ALPHA_BASE,
            k_alpha=settings.CONSENSUS_PARAM_K_ALPHA,
            update_sigmoid_L=settings.CONSENSUS_PARAM_UPDATE_SIG_L,
            update_sigmoid_k=settings.CONSENSUS_PARAM_UPDATE_SIG_K,
            update_sigmoid_x0=settings.CONSENSUS_PARAM_UPDATE_SIG_X0,
        )
        logger.debug(
            f"{log_prefix}: Trust update: {trust_score_old:.4f} -> {new_trust_score_float:.4f}"
        )
        # -----------------------------

        # --- 3. Tính Incentive (Dùng trust CŨ) ---
        incentive_float = 0.0
        if (
            getattr(miner_info, "status", STATUS_ACTIVE) == STATUS_ACTIVE
        ):  # Chỉ miner active mới nhận thưởng
            incentive_float = calculate_miner_incentive(
                trust_score=trust_score_old,  # <<<--- Dùng trust cũ
                miner_weight=getattr(miner_info, "weight", 0.0),
                miner_performance_scores=[score_new],  # Dùng P_adj
                total_system_value=total_system_value,
                # Lấy các tham số từ settings
                incentive_sigmoid_L=settings.CONSENSUS_PARAM_INCENTIVE_SIG_L,
                incentive_sigmoid_k=settings.CONSENSUS_PARAM_INCENTIVE_SIG_K,
                incentive_sigmoid_x0=settings.CONSENSUS_PARAM_INCENTIVE_SIG_X0,
            )
        logger.debug(f"{log_prefix}: Incentive calculated: {incentive_float:.6f}")
        # -------------------------------------

        # --- 4. Cập nhật Accumulated Rewards ---
        accumulated_rewards_new = pending_rewards_old + int(incentive_float * divisor)
        logger.debug(
            f"{log_prefix}: AccumulatedRewards update: {pending_rewards_old} -> {accumulated_rewards_new}"
        )
        # -------------------------------------

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
    final_miner_scores: Dict[str, float] = None,  # Added: Scores to hash
    signing_key: Optional[ExtendedSigningKey] = None, # Added: For correct address hashing
) -> Dict[str, ValidatorDatum]:
    """
    Prepares the ValidatorDatum update specifically for the validator running this node.

    Uses the pre-calculated state for this validator (from `run_consensus_logic`)
    and combines it with potentially updated trust/status (from `verify_and_penalize_logic`,
    reflected in `self_validator_info`) to create the final Datum.

    Also hashes the `final_miner_scores` (weights) into `performance_history_hash`
    to commit the consensus weights to the blockchain.

    Args:
        current_cycle (int): The current cycle number.
        self_validator_info (ValidatorInfo): The current info for this validator
                                           (potentially updated with penalties).
        calculated_states (Dict[str, Any]): The dictionary of calculated next states
                                           from `run_consensus_logic`.
        settings (Any): Application settings.
        context (Optional[BlockFrostChainContext]): Cardano context (may be unused currently).
        final_miner_scores (Dict[str, float]): The calculated miner scores (weights) for this cycle.
        signing_key (Optional[ExtendedSigningKey]): The validator's signing key for address hashing.

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

    # Hash địa chỉ
    wallet_addr_hash_bytes = getattr(self_validator_info, "wallet_addr_hash", None)

    # Ưu tiên tính lại hash từ signing key nếu có (để đảm bảo chính xác)
    if signing_key:
        try:
            # Sử dụng Payment Verification Key Hash (28 bytes)
            vkey = signing_key.to_verification_key()
            wallet_addr_hash_bytes = vkey.hash().to_primitive()
        except Exception as e:
            logger.warning(f"Failed to derive wallet hash from signing key: {e}")

    # Fallback nếu chưa có hash
    if not isinstance(wallet_addr_hash_bytes, bytes):
        # Cố gắng hash từ địa chỉ string (kém chính xác hơn nhưng là fallback)
        # Lưu ý: Đây là SHA256 của string, KHÔNG PHẢI key hash chuẩn của Cardano
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

    # --- Calculate Weight Hash (Commitment) ---
    if final_miner_scores:
        try:
            # Sort by UID to ensure deterministic hash
            sorted_scores = sorted(final_miner_scores.items())
            # Serialize to JSON string
            scores_str = json.dumps(sorted_scores, separators=(',', ':'))
            # Hash the string
            perf_history_hash_bytes = hashlib.sha256(scores_str.encode('utf-8')).digest()
            logger.info(f"Calculated weight hash for cycle {current_cycle}: {perf_history_hash_bytes.hex()[:8]}...")
        except Exception as e:
            logger.error(f"Failed to hash miner scores: {e}. Using empty bytes.")
            perf_history_hash_bytes = EMPTY_HASH_BYTES
    else:
        # Fallback: Use existing hash or empty
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
    corresponding input UTXO from the `current_utxo_map`, constructs a
    transaction spending that UTXO and creating a new one at the script address
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

    # 1. Lấy Input UTXO từ map cho chính mình
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
        # --- Cải tiến: Hydra Head Integration ---
        # Kiểm tra xem có dùng Hydra không
        use_hydra = getattr(settings, "USE_HYDRA_HEAD", False)
        hydra_client = None
        if use_hydra:
            try:
                hydra_client = await get_hydra_client()
                if not hydra_client.is_connected:
                    await hydra_client.connect()
                logger.info(f"{log_prefix}: Using Hydra Head for transaction submission.")
            except Exception as e:
                logger.error(f"{log_prefix}: Failed to connect to Hydra: {e}. Falling back to Layer 1.")
                use_hydra = False

        builder = TransactionBuilder(context=context)

        # a. Thêm Input Script UTXO (UTXO cũ của validator)
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

        # f. Submit Transaction (Hydra or L1)
        tx_id_str = str(signed_tx.id)

        if use_hydra and hydra_client:
            try:
                # Submit to Hydra
                # Cần serialize tx sang CBOR hex
                tx_cbor = signed_tx.to_cbor_hex()
                hydra_tx_id = await hydra_client.submit_tx(tx_cbor)
                if hydra_tx_id:
                    logger.info(f"{log_prefix}: :rocket: Successfully submitted update via Hydra! TxID: {hydra_tx_id}")
                    submitted_tx_ids[self_uid_hex] = hydra_tx_id
                else:
                    raise Exception("Hydra submission returned None")
            except Exception as hydra_e:
                logger.warning(f"{log_prefix}: Hydra submission failed ({hydra_e}). Falling back to L1...")
                # Fallback to L1
                context.submit_tx(signed_tx)
                logger.info(f"{log_prefix}: :rocket: Successfully submitted update via L1 (Fallback)! TxID: {tx_id_str}")
                submitted_tx_ids[self_uid_hex] = tx_id_str
        else:
            # Submit to L1 (BlockFrost)
            context.submit_tx(signed_tx)
            logger.info(f"{log_prefix}: :rocket: Successfully submitted update via L1! TxID: {tx_id_str}")
            submitted_tx_ids[self_uid_hex] = tx_id_str

    except Exception as build_e:
        logger.exception(
            f":hammer: {log_prefix}: Failed during transaction build/sign/submit phase: {build_e}"
        )
        failed_updates[self_uid_hex] = f"Error: {str(build_e)}"
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
