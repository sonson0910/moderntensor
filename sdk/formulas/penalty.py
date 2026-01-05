# sdk/formulas/penalty.py
import math


# --- Hàm phục hồi hiệu suất (Giữ nguyên logic) ---
def calculate_performance_adjustment(
    performance_score_new: float,  # E_validator_new
    performance_score_base: float = 0.9,  # Giá trị mẫu, cần xác định
    recovery_rate: float = 0.1,  # Giá trị mẫu, gamma
) -> float:
    """
    Điều chỉnh (phục hồi) hiệu suất của validator theo thời gian.

    Args:
        performance_score_new: Điểm hiệu suất hiện tại (E_validator_new).
        performance_score_base: Điểm hiệu suất cơ bản (mục tiêu phục hồi). (Cần xác định)
        recovery_rate: Tỷ lệ phục hồi (gamma). (Cần xác định)

    Returns:
        Điểm hiệu suất sau khi điều chỉnh phục hồi.
    """
    adjustment = performance_score_new + recovery_rate * (
        performance_score_base - performance_score_new
    )
    # Đảm bảo không vượt quá base hoặc dưới 0
    return max(0.0, min(performance_score_base, adjustment))


# --- Hàm tính mức độ nghiêm trọng (Hàm trừu tượng, cần logic cụ thể) ---
def calculate_fraud_severity_value(
    detected_behavior: dict,  # Input chứa thông tin về hành vi gian lận đã phát hiện
    # Các tham số ngưỡng ví dụ (Cần xác định giá trị tối ưu)
    # threshold_dev1: float = 0.3,
    # threshold_dev2: float = 0.5,
    # n1_cycles: int = 5
) -> float:
    """
    Xác định giá trị fraud_severity (0 đến 1) dựa trên hành vi gian lận được phát hiện.
    Triển khai logic phân loại theo bậc (Multi-Tier Slashing).

    Args:
        detected_behavior: Dictionary chứa thông tin chi tiết về hành vi.
                           Keys: 'type', 'details', 'deviation', 'proof_valid', etc.

    Returns:
        Giá trị fraud_severity (0.0 - 1.0).
    """
    behavior_type = detected_behavior.get("type", "Unknown")
    severity = 0.0

    # Cấp 3: Nghiêm trọng (Malicious) - Cắt 50% - 100% Stake
    if behavior_type == "Sybil_Attack":
        return 1.0
    elif behavior_type == "Fake_Proof":  # zkML proof invalid
        return 1.0
    elif behavior_type == "Malicious_Code":
        return 1.0

    # Cấp 2: Trung bình (Equivocation/Integrity) - Cắt 1% - 10% Stake
    elif behavior_type == "Double_Signing":
        return 0.1
    elif behavior_type == "Commit_Reveal_Mismatch": # Hash commit không khớp với reveal
        return 0.2
    elif behavior_type == "Plagiarism": # Copy kết quả (nếu phát hiện được)
        return 0.3

    # Cấp 1: Nhẹ (Performance/Availability) - Giảm Trust Score (Severity thấp)
    elif behavior_type == "Offline":
        return 0.01
    elif behavior_type == "Low_Performance":
        # Tính severity dựa trên độ lệch (deviation)
        deviation = detected_behavior.get("deviation", 0.0)
        if deviation > 0.5:
            return 0.05
        else:
            return 0.0

    return severity
        return 0.8  # Bậc 3
    elif detected_behavior.get("type") == "Invalid_Data":
        return 0.3  # Bậc 2
    elif detected_behavior.get("type") == "Sustained_Deviation":
        # Kiểm tra thêm chi tiết độ lệch và thời gian trong detected_behavior
        return 0.1  # Bậc 1
    else:
        return 0.0  # Không có gian lận


# --- Hàm tính lượng stake bị cắt (Cập nhật để nhận fraud_severity đã tính) ---
def calculate_slash_amount(
    stake: float,
    fraud_severity: float,  # Giá trị đã được xác định từ hàm trên
    max_slash_rate: float = 0.2,  # Giá trị mẫu, nên do DAO quản trị
) -> float:
    """
    Tính số tiền bị cắt (slash) dựa trên stake và mức độ gian lận đã xác định.

    Args:
        stake: Số tiền stake của miner/validator.
        fraud_severity: Mức độ nghiêm trọng của gian lận (0 đến 1) đã được xác định.
        max_slash_rate: Tỷ lệ cắt tối đa. (Nên do DAO quản trị)

    Returns:
        Số tiền bị cắt.
    """
    if stake <= 0:
        return 0.0
    # Đảm bảo fraud_severity trong khoảng [0, 1]
    clamped_severity = max(0.0, min(1.0, fraud_severity))

    slash_amount = stake * min(max_slash_rate, clamped_severity)
    return max(0.0, slash_amount)  # Đảm bảo không âm
