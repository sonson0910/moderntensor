# sdk/formulas/incentive.py
import math
from typing import List
from .utils import sigmoid # Import hàm sigmoid

def calculate_miner_incentive(
    trust_score: float,
    miner_weight: float,
    miner_performance_scores: List[float],
    total_system_value: float,
    # Tham số Sigmoid cho Trust Score -> Incentive (Giá trị mẫu, cần xác định)
    incentive_sigmoid_L: float = 1.0,
    incentive_sigmoid_k: float = 10.0,
    incentive_sigmoid_x0: float = 0.5
) -> float:
    """
    Tính phần thưởng cho miner dựa trên điểm tin cậy (qua sigmoid), trọng số và điểm hiệu suất.

    Args:
        trust_score: Điểm tin cậy của miner.
        miner_weight: Trọng số của miner (W_x).
        miner_performance_scores: Danh sách điểm hiệu suất từ validators (P_xj).
        total_system_value: Tổng giá trị đóng góp (W*P) của tất cả miners trong hệ thống.
        incentive_sigmoid_L: Tham số L cho hàm sigmoid f_sig. (Cần xác định giá trị tối ưu)
        incentive_sigmoid_k: Tham số k cho hàm sigmoid f_sig. (Cần xác định giá trị tối ưu)
        incentive_sigmoid_x0: Tham số x0 cho hàm sigmoid f_sig. (Cần xác định giá trị tối ưu)

    Returns:
        Phần thưởng tính được cho miner.
    """
    if total_system_value == 0:
        return 0.0

    # Áp dụng sigmoid cho trust score
    trust_factor = sigmoid(
        trust_score,
        L=incentive_sigmoid_L,
        k=incentive_sigmoid_k,
        y0=incentive_sigmoid_x0
    )

    # Tính tổng hiệu suất có trọng số của miner này
    # Giả định miner_performance_scores chứa các P_xj
    sum_weighted_performance = miner_weight * sum(miner_performance_scores)

    # Tính incentive
    incentive = trust_factor * (sum_weighted_performance / total_system_value)
    return max(0.0, incentive) # Đảm bảo phần thưởng không âm

def calculate_validator_incentive(
    trust_score: float,
    validator_weight: float,
    validator_performance: float, # E_v
    total_validator_value: float, # Sum(W_u * E_u)
    # Tham số Sigmoid cho Trust Score -> Incentive (Giá trị mẫu, cần xác định)
    incentive_sigmoid_L: float = 1.0,
    incentive_sigmoid_k: float = 10.0,
    incentive_sigmoid_x0: float = 0.5
) -> float:
    """
    Tính phần thưởng cho validator dựa trên điểm tin cậy (qua sigmoid), trọng số và hiệu suất.

    Args:
        trust_score: Điểm tin cậy của validator.
        validator_weight: Trọng số của validator (W_v).
        validator_performance: Điểm hiệu suất của validator (E_v).
        total_validator_value: Tổng giá trị (W*E) của tất cả validators.
        incentive_sigmoid_L: Tham số L cho hàm sigmoid f_sig. (Cần xác định giá trị tối ưu)
        incentive_sigmoid_k: Tham số k cho hàm sigmoid f_sig. (Cần xác định giá trị tối ưu)
        incentive_sigmoid_x0: Tham số x0 cho hàm sigmoid f_sig. (Cần xác định giá trị tối ưu)

    Returns:
        Phần thưởng tính được cho validator.
    """
    if total_validator_value == 0:
        return 0.0

     # Áp dụng sigmoid cho trust score
    trust_factor = sigmoid(
        trust_score,
        L=incentive_sigmoid_L,
        k=incentive_sigmoid_k,
        y0=incentive_sigmoid_x0
    )

    # Tính đóng góp có trọng số của validator này
    weighted_performance = validator_weight * validator_performance

    # Tính incentive
    incentive = trust_factor * (weighted_performance / total_validator_value)
    return max(0.0, incentive) # Đảm bảo phần thưởng không âm