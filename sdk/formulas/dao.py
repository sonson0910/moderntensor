# sdk/formulas/dao.py
import math

def calculate_voting_power(
    stake_p: float,
    time_staked_p: float, # Thời gian đã stake (cùng đơn vị với total_time)
    total_time: float,    # Khoảng thời gian tham chiếu
    lockup_multiplier: float = 1.0, # Hệ số nhân nếu khóa stake
    # Tham số (Giá trị mẫu, cần xác định/DAO quản trị)
    time_bonus_factor_kg: float = 1.0
) -> float:
    """
    Tính quyền biểu quyết trong DAO dựa trên stake, thời gian stake (sqrt) và hệ số khóa stake.

    Args:
        stake_p: Số tiền stake của thành viên p.
        time_staked_p: Thời gian thành viên p đã stake. (Đơn vị cần thống nhất)
        total_time: Khoảng thời gian tham chiếu để chuẩn hóa time_staked_p. (Cần định nghĩa)
        lockup_multiplier: Hệ số nhân thêm nếu p khóa stake (>1 nếu khóa, 1 nếu không). (Cần cơ chế xác định)
        time_bonus_factor_kg: Hệ số điều chỉnh bonus thời gian (k_g). (Cần xác định)

    Returns:
        Quyền biểu quyết của thành viên p.
    """
    if stake_p <= 0:
        return 0.0

    # Tính bonus thời gian (đảm bảo time không âm và total_time dương)
    time_ratio = 0.0
    if total_time > 0:
        safe_time_staked = max(0, time_staked_p)
        time_ratio = safe_time_staked / total_time
        # Áp dụng sqrt để lợi ích giảm dần
        time_bonus = time_bonus_factor_kg * math.sqrt(time_ratio)
    else:
        time_bonus = 0 # Tránh lỗi chia cho 0

    # Tính voting power
    voting_power = stake_p * (1 + time_bonus) * lockup_multiplier

    return max(0.0, voting_power)