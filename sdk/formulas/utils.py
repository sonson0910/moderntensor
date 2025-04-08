# sdk/formulas/_utils.py
import math

def sigmoid(y: float, L: float = 1.0, k: float = 10.0, y0: float = 0.5) -> float:
    """
    Hàm Sigmoid chuẩn.
    Args:
        y: Giá trị đầu vào (ví dụ: trust score, performance score).
        L: Giá trị tối đa của hàm. (Cần xác định giá trị tối ưu)
        k: Độ dốc của hàm. (Cần xác định giá trị tối ưu)
        y0: Điểm uốn của hàm. (Cần xác định giá trị tối ưu)
    Returns:
        Giá trị sigmoid tương ứng.
    """
    try:
        return L / (1 + math.exp(-k * (y - y0)))
    except OverflowError:
        # Xử lý trường hợp giá trị exp quá lớn hoặc quá nhỏ
        if -k * (y - y0) > 700:  # Giới hạn trên để tránh overflow exp()
             return L
        else: # Giới hạn dưới
             return 0.0

def calculate_alpha_effective(trust_score_old: float, alpha_base: float = 0.1, k_alpha: float = 1.0) -> float:
    """
    Tính learning rate hiệu dụng, giảm dần khi trust score gần 0 hoặc 1.
    Args:
        trust_score_old: Điểm tin cậy cũ.
        alpha_base: Learning rate cơ bản. (Cần xác định giá trị tối ưu)
        k_alpha: Hệ số điều chỉnh độ nhạy (0=không đổi, 2=bằng 0 ở cực). (Cần xác định giá trị tối ưu)
    Returns:
        Learning rate hiệu dụng.
    """
    # Đảm bảo k_alpha không làm learning rate âm
    k_alpha_adjusted = min(k_alpha, 2.0) # Giới hạn k_alpha tối đa là 2 để alpha không âm
    effective_alpha = alpha_base * (1 - k_alpha_adjusted * abs(trust_score_old - 0.5))
    return max(0, effective_alpha) # Đảm bảo alpha không âm