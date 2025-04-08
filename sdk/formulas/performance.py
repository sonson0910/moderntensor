# sdk/formulas/performance.py
import math
from typing import List

# --- Hàm tính Q_task hoặc P_miner cơ bản (Giữ nguyên logic) ---
def calculate_task_completion_rate(
    success_tasks: List[int],
    total_tasks: List[int],
    current_time: int,
    decay_constant: float = 0.5 # Giá trị mẫu, nên do DAO quản trị
) -> float:
    """
    Tính tỷ lệ hoàn thành nhiệm vụ với yếu tố suy giảm theo thời gian (Q_task hoặc P_miner).

    Args:
        success_tasks: Danh sách nhiệm vụ hoàn thành thành công theo thời gian.
        total_tasks: Danh sách tổng số nhiệm vụ theo thời gian.
        current_time: Thời điểm hiện tại (T).
        decay_constant: Hằng số suy giảm (delta). (Nên do DAO quản trị)

    Returns:
        Tỷ lệ hoàn thành nhiệm vụ.
    """
    numerator = sum(success * math.exp(-decay_constant * (current_time - t))
                    for t, success in enumerate(success_tasks))
    denominator = sum(total * math.exp(-decay_constant * (current_time - t))
                      for t, total in enumerate(total_tasks))

    # Tránh chia cho 0
    if denominator == 0:
        return 0.0

    # Giới hạn giá trị trả về trong khoảng [0, 1] phòng trường hợp lỗi dữ liệu
    rate = numerator / denominator
    return max(0.0, min(1.0, rate))


# --- Hàm tính P_miner_adjusted (Giữ nguyên logic) ---
def calculate_adjusted_miner_performance(
    performance_scores_by_validators: List[float], # List các P_miner,v
    trust_scores_of_validators: List[float]      # List các trust_score_v tương ứng
) -> float:
    """
    Tính hiệu suất điều chỉnh của miner dựa trên điểm tin cậy của validators.

    Args:
        performance_scores_by_validators: Điểm hiệu suất do validators đưa ra (P_miner,v).
        trust_scores_of_validators: Điểm tin cậy của các validators tương ứng.

    Returns:
        Điểm hiệu suất điều chỉnh (P_miner_adjusted).
    """
    if not trust_scores_of_validators or not performance_scores_by_validators or len(performance_scores_by_validators) != len(trust_scores_of_validators):
        return 0.0 # Hoặc raise lỗi tùy thiết kế

    numerator = sum(trust * perf for trust, perf in zip(trust_scores_of_validators, performance_scores_by_validators))
    total_trust = sum(trust_scores_of_validators)

    if total_trust == 0:
        return 0.0 # Tránh chia cho 0

    adjusted_performance = numerator / total_trust
    return max(0.0, min(1.0, adjusted_performance)) # Đảm bảo kết quả trong [0, 1]


# --- Hàm tính Penalty Term (Hàm mới cho E_validator) ---
def calculate_penalty_term(
    deviation: float, # Độ lệch chuẩn hóa |Eval - Avg| / sigma
    threshold_dev: float = 0.1, # Giá trị mẫu, cần xác định
    k_penalty: float = 5.0,    # Giá trị mẫu, cần xác định
    p_penalty: float = 1.0     # Giá trị mẫu, cần xác định (1 hoặc 2)
) -> float:
    """
    Tính thành phần phạt dựa trên độ lệch đánh giá so với trung bình.

    Args:
        deviation: Độ lệch chuẩn hóa của đánh giá.
        threshold_dev: Ngưỡng độ lệch bắt đầu phạt. (Cần xác định giá trị tối ưu)
        k_penalty: Hệ số kiểm soát mức độ phạt (k'). (Cần xác định giá trị tối ưu)
        p_penalty: Bậc của độ lệch trong công thức phạt (p). (Cần xác định giá trị tối ưu)

    Returns:
        Giá trị thành phần phạt (từ 0 đến 1, 1 là không phạt).
    """
    penalty_factor = k_penalty * (max(0, deviation - threshold_dev) ** p_penalty)
    penalty_term = 1 / (1 + penalty_factor)
    return penalty_term # Giá trị trong khoảng (0, 1]


# --- Hàm tính E_validator (Cập nhật) ---
def calculate_validator_performance(
    q_task_validator: float,          # Tỷ lệ hoàn thành task của validator (nếu có)
    metric_validator_quality: float, # Chỉ số chất lượng đánh giá (cần được tính từ bên ngoài)
    deviation: float,                # Độ lệch đánh giá |Eval-Avg|/sigma
    # Tham số trọng số (Giá trị mẫu, nên do DAO quản trị)
    theta1: float = 0.3,
    theta2: float = 0.4,
    theta3: float = 0.3,
    # Tham số cho Penalty Term (Giá trị mẫu, cần xác định)
    penalty_threshold_dev: float = 0.1,
    penalty_k_penalty: float = 5.0,
    penalty_p_penalty: float = 1.0
) -> float:
    """
    Tính điểm hiệu suất tổng hợp của Validator (E_validator).

    Args:
        q_task_validator: Tỷ lệ hoàn thành task của validator (0 nếu không áp dụng).
        metric_validator_quality: Chỉ số chất lượng đánh giá đã tính toán. (Cần logic tính toán riêng)
        deviation: Độ lệch chuẩn hóa của đánh giá validator so với trung bình.
        theta1, theta2, theta3: Trọng số cho các thành phần (tổng = 1). (Nên do DAO quản trị)
        penalty_threshold_dev: Ngưỡng phạt độ lệch. (Cần xác định giá trị tối ưu)
        penalty_k_penalty: Hệ số phạt độ lệch. (Cần xác định giá trị tối ưu)
        penalty_p_penalty: Bậc phạt độ lệch. (Cần xác định giá trị tối ưu)

    Returns:
        Điểm hiệu suất Validator (E_validator).
    """
    # Đảm bảo tổng theta = 1 (có thể chuẩn hóa lại nếu cần)
    if not math.isclose(theta1 + theta2 + theta3, 1.0):
        print("Warning: Tổng theta trong calculate_validator_performance không bằng 1.")
        # Có thể chuẩn hóa: total = theta1+theta2+theta3; theta1/=total; ...

    penalty_term_value = calculate_penalty_term(
        deviation,
        threshold_dev=penalty_threshold_dev,
        k_penalty=penalty_k_penalty,
        p_penalty=penalty_p_penalty
    )

    e_validator = (theta1 * q_task_validator +
                   theta2 * metric_validator_quality +
                   theta3 * penalty_term_value)

    return max(0.0, min(1.0, e_validator)) # Đảm bảo kết quả trong [0, 1]