# sdk/moderntensor_formulas/resource_allocation.py

def calculate_subnet_resource(
    subnet_contribution: float, # Tổng W*P của subnet
    total_contribution: float, # Tổng W*P toàn hệ thống
    total_resources: float
) -> float:
    """
    Tính tài nguyên phân bổ cho subnet dựa trên đóng góp tương đối.

    Args:
        subnet_contribution: Tổng đóng góp (ví dụ: Sum(W_i * P_i)) của subnet.
        total_contribution: Tổng đóng góp của tất cả subnets.
        total_resources: Tổng tài nguyên có sẵn.

    Returns:
        Số tài nguyên phân bổ cho subnet.
    """
    if total_contribution == 0:
        return 0.0
    allocation = (subnet_contribution / total_contribution) * total_resources
    return max(0.0, allocation)