# sdk/formulas/validator_weight.py
import math
import logging # Thêm logging

# Giả định epsilon là một hằng số nhỏ để tránh chia cho 0
EPSILON = 1e-9

def calculate_validator_weight(
    stake_v: float,                  # Stake của validator v
    total_stake: float,              # Tổng stake của tất cả validators
    e_validator: float,              # Hiệu suất của validator v (E_v)
    e_avg: float,                    # Hiệu suất trung bình của validators (E_avg)
    time_participated: float,        # Thời gian tham gia (đơn vị cần thống nhất)
    # Tham số (Giá trị mẫu, cần xác định/DAO quản trị)
    lambda_balance: float = 0.5,     # Hệ số cân bằng stake/performance
    stake_log_base: float = math.e,  # Cơ số log cho stake (e hoặc 10)
    time_log_base: float = 10        # Cơ số log cho thời gian
) -> float:
    """
    Tính trọng số của validator (W_validator) dựa trên stake (log), hiệu suất tương đối và thời gian tham gia (log10).

    Args:
        stake_v: Số tiền stake của validator v.
        total_stake: Tổng số tiền stake của tất cả validators đang hoạt động.
        e_validator: Điểm hiệu suất của validator v (E_v).
        e_avg: Điểm hiệu suất trung bình của các validators liên quan. (Cần logic tính toán riêng)
        time_participated: Thời gian validator đã tham gia (ví dụ: số epoch, số ngày). (Đơn vị cần thống nhất)
        lambda_balance: Hệ số cân bằng stake/performance (lambda). (Quan trọng, nên do DAO quản trị)
        stake_log_base: Cơ số của hàm log áp dụng cho stake (math.e cho ln, 10 cho log10). (Cần xác định)
        time_log_base: Cơ số của hàm log áp dụng cho thời gian (thường là 10).

    Returns:
        Trọng số của validator (W_validator).
    """
    # 1. Tính thành phần Stake (đã qua log và chuẩn hóa)
    # Sử dụng log(1 + stake) để tránh log(0) và đảm bảo giá trị không âm
    log_stake_v = math.log(1 + stake_v, stake_log_base) if stake_v >= 0 else 0

    # Cần tính tổng log(1 + stake) của tất cả validators (giả sử có hàm làm việc này)
    # total_log_stake = calculate_total_log_stake(active_validators_stakes, stake_log_base)
    # *** Giả định total_log_stake đã được tính và truyền vào, hoặc tính ở nơi khác ***
    # *** Để ví dụ, ta giả định nó tỷ lệ với total_stake (đây là giả định đơn giản hóa) ***
    # *** Logic chuẩn hóa thực tế cần được thiết kế cẩn thận ***
    if total_stake <= 0: # Tránh chia cho 0 nếu chưa có ai stake
         stake_component_normalized = 0
    else:
         # Ước lượng đơn giản, cần thay thế bằng tính toán tổng log stake thực tế
         # Ví dụ: nếu có N validators, total_log_stake có thể ~ N * log(1 + total_stake/N)
         # Hoặc đơn giản là chuẩn hóa theo tỷ lệ stake gốc nếu hàm log làm khó chuẩn hóa:
         stake_component_normalized = stake_v / total_stake # Tạm dùng tỷ lệ gốc

    # 2. Tính thành phần Hiệu suất và Thời gian
    # Chuẩn hóa hiệu suất theo trung bình
    performance_ratio = e_validator / max(e_avg, EPSILON) if e_avg > 0 else (1 if e_validator > 0 else 0)

    # Tính bonus thời gian (dùng log cơ số time_log_base)
    # Đảm bảo time_participated >= 0
    safe_time = max(0, time_participated)
    # Sử dụng log(1 + time) để tránh log(0)
    time_bonus = math.log(1 + safe_time, time_log_base) if time_log_base > 1 else 0

    performance_time_component = performance_ratio * (1 + time_bonus)

    # 3. Kết hợp các thành phần với lambda
    weight = (lambda_balance * stake_component_normalized +
              (1 - lambda_balance) * performance_time_component)

    return max(0.0, weight) # Đảm bảo trọng số không âm