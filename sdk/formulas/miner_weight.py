# sdk/formulas/miner_weight.py
import math
from typing import List, Sequence

def calculate_miner_weight(
    performance_history: Sequence[float],
    current_time_step: int,
    # Tham số (Giá trị mẫu, cần xác định/DAO quản trị)
    decay_constant_W: float = 0.5
) -> float:
    """
    Tính trọng số của Miner (W_x) dựa trên lịch sử hiệu suất với suy giảm theo hàm mũ.

    Công thức: W_x = Sum[P_miner,t * exp(-delta_W*(T-t))]

    Args:
        performance_history: Một chuỗi (list, tuple,...) chứa điểm hiệu suất
                             lịch sử của miner (ví dụ: P_miner hoặc P_miner_adjusted).
                             Phần tử đầu tiên là hiệu suất tại t=0, thứ hai tại t=1,...
        current_time_step: Bước thời gian hiện tại (T). Giả định T là chỉ số
                           của bước thời gian *tiếp theo*, tức là T = len(performance_history)
                           nếu lịch sử chứa tất cả các điểm đến T-1.
        decay_constant_W: Hằng số suy giảm trọng số (delta_W).
                          (Nên do DAO quản trị, giá trị mẫu = 0.5)

    Returns:
        Trọng số Miner (W_x) đã tính toán. Giá trị không âm.
    """
    weight = 0.0
    history_len = len(performance_history)

    # Đảm bảo current_time_step hợp lệ (ít nhất bằng độ dài lịch sử)
    # Thông thường T = history_len
    if current_time_step < history_len:
        # Có thể log warning hoặc raise error tùy thiết kế
        # Tạm thời coi T = history_len nếu T < history_len
        effective_T = history_len
    else:
        effective_T = current_time_step

    for t, performance_score in enumerate(performance_history):
        # performance_score là P_miner,t
        time_diff = effective_T - t
        decay_factor = math.exp(-decay_constant_W * time_diff)
        weight += performance_score * decay_factor

    return max(0.0, weight) # Đảm bảo trọng số không âm

