# sdk/formulas/trust_score.py
import math
from typing import Dict  # Added for type hints
from .utils import sigmoid, calculate_alpha_effective  # Import helpers


def update_trust_score(
    trust_score_old: float,
    time_since_last_eval: int,  # Số chu kỳ/đơn vị thời gian
    score_new: float,  # Điểm hiệu suất mới (P_adj hoặc E_val)
    # Tham số (Giá trị mẫu, cần xác định/DAO quản trị)
    delta_trust: float = 0.1,
    alpha_base: float = 0.1,
    k_alpha: float = 1.0,
    update_sigmoid_L: float = 1.0,
    update_sigmoid_k: float = 5.0,
    update_sigmoid_x0: float = 0.5,
) -> float:
    """
    Cập nhật điểm tin cậy với suy giảm, learning rate thay đổi và sigmoid cho điểm mới.

    Args:
        trust_score_old: Điểm tin cậy trước đó.
        time_since_last_eval: Thời gian kể từ lần đánh giá cuối cùng.
        score_new: Điểm hiệu suất mới (ví dụ P_miner_adjusted hoặc E_validator). score_new=0 nếu không được đánh giá.
        delta_trust: Hằng số suy giảm trust score. (Nên do DAO quản trị)
        alpha_base: Learning rate cơ bản. (Cần xác định giá trị tối ưu)
        k_alpha: Hệ số điều chỉnh learning rate. (Cần xác định giá trị tối ưu)
        update_sigmoid_L: Tham số L cho hàm sigmoid f_update_sig. (Cần xác định giá trị tối ưu)
        update_sigmoid_k: Tham số k cho hàm sigmoid f_update_sig. (Cần xác định giá trị tối ưu)
        update_sigmoid_x0: Tham số x0 cho hàm sigmoid f_update_sig. (Cần xác định giá trị tối ưu)

    Returns:
        Điểm tin cậy đã cập nhật.
    """
    # 1. Tính suy giảm
    decayed_score = trust_score_old * math.exp(-delta_trust * time_since_last_eval)

    # 2. Tính learning rate hiệu dụng
    alpha_eff = calculate_alpha_effective(trust_score_old, alpha_base, k_alpha)

    # 3. Ánh xạ điểm mới qua sigmoid
    mapped_score_new = sigmoid(
        score_new, L=update_sigmoid_L, k=update_sigmoid_k, y0=update_sigmoid_x0
    )
    # Nếu score_new = 0 (không được đánh giá), mapped_score_new cũng sẽ gần 0 (tùy tham số sigmoid)
    # và phần cập nhật gần như bằng 0. Đặc biệt nếu score_new = 0, không nên cộng thêm gì.
    update_term = alpha_eff * mapped_score_new if score_new > 0 else 0

    # 4. Tính điểm mới
    updated_score = decayed_score + update_term

    return max(0.0, min(1.0, updated_score))  # Giới hạn trong khoảng [0, 1]


def calculate_selection_probability(
    trust_score: float,
    time_since_last_selection: int,  # Số chu kỳ/đơn vị thời gian
    # Tham số (Giá trị mẫu, cần xác định/DAO quản trị)
    beta: float = 0.2,
    max_time_bonus_effect: int = 10,  # Giới hạn số chu kỳ bonus có tác dụng
) -> float:
    """
    Tính xác suất chọn miner/validator dựa trên điểm tin cậy và thời gian không được chọn (có giới hạn).

    Args:
        trust_score: Điểm tin cậy hiện tại.
        time_since_last_selection: Thời gian kể từ lần chọn cuối cùng.
        beta: Hệ số thưởng công bằng. (Nên do DAO quản trị)
        max_time_bonus_effect: Số chu kỳ tối đa bonus thời gian có tác dụng. (Cần xác định giá trị tối ưu)

    Returns:
        Giá trị tỉ lệ với xác suất chọn (có thể cần chuẩn hóa thêm).
    """
    # Giới hạn thời gian bonus
    effective_time = min(time_since_last_selection, max_time_bonus_effect)

    # Tính bonus factor
    fairness_bonus = 1 + beta * effective_time

    # Tính xác suất (chưa chuẩn hóa)
    probability_factor = trust_score * fairness_bonus
    return max(0.0, probability_factor)


def calculate_consensus_score(
    miner_scores: Dict[str, float],  # {validator_uid: score_for_miner}
    validator_stakes: Dict[str, float],  # {validator_uid: stake_amount}
) -> float:
    """
    Tính điểm đồng thuận (Global Consensus Score) cho một Miner dựa trên đánh giá
    của các Validator, có trọng số theo Stake. (Cơ chế tương tự Yuma Consensus).

    Args:
        miner_scores: Dict chứa điểm số mà các validator chấm cho miner này.
        validator_stakes: Dict chứa lượng stake của các validator tương ứng.

    Returns:
        Điểm đồng thuận (0.0 - 1.0).
    """
    total_weighted_score = 0.0
    total_stake = 0.0

    for val_uid, score in miner_scores.items():
        stake = validator_stakes.get(val_uid, 0.0)
        if stake > 0:
            total_weighted_score += score * stake
            total_stake += stake

    if total_stake == 0:
        return 0.0

    return total_weighted_score / total_stake


def calculate_validator_consensus_weight(
    validator_scores: Dict[str, float],  # {miner_uid: score}
    consensus_scores: Dict[str, float],  # {miner_uid: global_consensus_score}
    sigma: float = 0.5,  # Độ lệch chuẩn cho phép (Gaussian kernel width)
) -> float:
    """
    Tính trọng số đồng thuận cho Validator. Validator nào chấm điểm càng gần
    với điểm đồng thuận chung thì trọng số càng cao.

    Công thức: W_v = Sum(Gaussian(score_v_m - consensus_m)) / Num_Miners

    Args:
        validator_scores: Điểm số validator chấm cho các miner.
        consensus_scores: Điểm đồng thuận đã tính được của các miner.
        sigma: Tham số độ rộng của hàm Gaussian (độ khoan dung sai số).

    Returns:
        Trọng số đồng thuận (0.0 - 1.0).
    """
    if not validator_scores:
        return 0.0

    total_similarity = 0.0
    count = 0

    for miner_uid, val_score in validator_scores.items():
        if miner_uid in consensus_scores:
            consensus_score = consensus_scores[miner_uid]
            # Tính độ tương đồng bằng hàm Gaussian
            diff = val_score - consensus_score
            similarity = math.exp(-(diff**2) / (2 * sigma**2))
            total_similarity += similarity
            count += 1

    if count == 0:
        return 0.0

    return total_similarity / count
