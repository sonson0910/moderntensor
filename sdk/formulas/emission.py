# moderntensor/sdk/formulas/emission.py

import math
from typing import Union

# --- Constants ---

TOKEN_TICKER = "MIT"  # Tên token của bạn (có thể lấy từ config sau này)
TOKEN_DECIMALS = 6  # Số chữ số thập phân (phổ biến trên Cardano là 6)
_SATOSHI_MULTIPLIER = 10**TOKEN_DECIMALS

# Tổng cung tối đa: 21 triệu MIT, tính bằng đơn vị nhỏ nhất
TOTAL_SUPPLY: int = 21_000_000 * _SATOSHI_MULTIPLIER

# Tỷ lệ phát hành ban đầu: Giả định tương tự Bittensor (1 TAO/block ~ 12s)
# Quy đổi ra tỷ lệ hàng ngày: (24 * 60 * 60 / 12) * 1 = 7200 MIT/ngày
# Giá trị này có thể cần điều chỉnh dựa trên thiết kế cụ thể của Moderntensor
INITIAL_TOKENS_PER_DAY: int = 7200 * _SATOSHI_MULTIPLIER

# --- Helper Functions ---


def _calculate_num_halvings(current_supply: int, total_supply: int) -> int:
    """
    Tính số lần halving đã xảy ra dựa trên lượng cung lưu hành hiện tại
    theo công thức của Bittensor.

    Halving xảy ra khi tỷ lệ cung lưu hành / tổng cung đạt 1/2, 3/4, 7/8,...
    (tương đương 1 - 1/(2^k)). Tìm k lớn nhất thỏa mãn.

    Args:
        current_supply: Tổng lượng token đã phát hành (tính bằng đơn vị nhỏ nhất).
        total_supply: Tổng cung tối đa (tính bằng đơn vị nhỏ nhất).

    Returns:
        Số lần halving đã xảy ra.
    """
    if total_supply == 0:
        return 0  # Tránh chia cho 0
    if current_supply <= 0:
        return 0  # Chưa có cung thì chưa halving

    num_halvings = 0
    # Ngưỡng halving đầu tiên là T/2
    supply_threshold = total_supply / 2

    # Lặp để tìm số lần halving
    # Thêm giới hạn 256 để tránh vòng lặp vô hạn nếu có lỗi logic
    while current_supply >= supply_threshold and num_halvings < 256:
        num_halvings += 1
        # Tính ngưỡng halving tiếp theo: T * (1 - 1 / (2^(k+1)))
        remaining_supply_fraction = 1 / (2 ** (num_halvings + 1))
        # Dùng floor để xử lý sai số float và đảm bảo so sánh số nguyên
        supply_threshold = math.floor(total_supply * (1 - remaining_supply_fraction))

        # Nếu ngưỡng tiếp theo nhỏ hơn hoặc bằng ngưỡng hiện tại do sai số float/integer, dừng lại
        if supply_threshold <= total_supply * (1 - 1 / (2**num_halvings)):
            break

    return num_halvings


# --- Main Emission Calculation Functions ---


def calculate_current_emission_per_day(current_circulating_supply: int) -> int:
    """
    Tính toán tỷ lệ phát hành mục tiêu mỗi ngày dựa trên số lần halving đã xảy ra.

    Args:
        current_circulating_supply: Tổng số token đã được phát hành (đơn vị nhỏ nhất).

    Returns:
        Số lượng token mục tiêu (đơn vị nhỏ nhất) nên được phát hành mỗi ngày
        ở giai đoạn halving hiện tại. Trả về 0 nếu đã đạt tổng cung.
    """
    if current_circulating_supply >= TOTAL_SUPPLY:
        return 0

    num_halvings = _calculate_num_halvings(current_circulating_supply, TOTAL_SUPPLY)
    # Tỷ lệ hiện tại = Tỷ lệ ban đầu / (2 ^ số lần halving)
    current_daily_rate = INITIAL_TOKENS_PER_DAY / (2**num_halvings)

    return math.floor(current_daily_rate)  # Trả về số nguyên


def calculate_emission_for_epoch(
    current_circulating_supply: int,
    epoch_duration_days: float = 1.0,  # Mặc định 1 ngày, cần điều chỉnh theo epoch thực tế
) -> int:
    """
    Tính toán tổng lượng token phát hành cho một chu kỳ (epoch) với độ dài cho trước,
    đồng thời đảm bảo không vượt quá tổng cung tối đa.

    Args:
        current_circulating_supply: Tổng số token đã phát hành *trước* epoch này (đơn vị nhỏ nhất).
        epoch_duration_days: Độ dài của epoch tính bằng ngày (có thể là số lẻ).

    Returns:
        Tổng số token (đơn vị nhỏ nhất) sẽ được phát hành trong epoch này.
    """
    if current_circulating_supply >= TOTAL_SUPPLY:
        return 0

    # Lấy tỷ lệ phát hành hàng ngày dựa trên lượng cung *trước* epoch này
    current_daily_rate = calculate_current_emission_per_day(current_circulating_supply)

    # Tính lượng phát hành tiềm năng cho epoch này
    potential_emission = math.floor(current_daily_rate * epoch_duration_days)

    # Giới hạn lượng phát hành để không vượt quá tổng cung
    remaining_supply_to_emit = TOTAL_SUPPLY - current_circulating_supply
    actual_emission = max(0, min(potential_emission, remaining_supply_to_emit))

    # Đảm bảo kết quả cuối cùng là số nguyên
    return int(actual_emission)


# --- Example Usage (for testing/demonstration) ---
if __name__ == "__main__":
    # Ví dụ: Tính toán cho epoch đầu tiên (giả sử epoch = 5 ngày)
    circ_supply_start = 0
    epoch_days = 5.0  # Cardano epoch duration is roughly 5 days
    first_epoch_emission = calculate_emission_for_epoch(circ_supply_start, epoch_days)
    print(f"--- Example 1: Start ---")
    print(
        f"Initial Circulating Supply: {circ_supply_start / _SATOSHI_MULTIPLIER:.{TOKEN_DECIMALS}f} {TOKEN_TICKER}"
    )
    print(f"Epoch Duration: {epoch_days} days")
    print(
        f"Emission for first {epoch_days}-day epoch: {first_epoch_emission / _SATOSHI_MULTIPLIER:.{TOKEN_DECIMALS}f} {TOKEN_TICKER} ({first_epoch_emission} units)"
    )
    print(
        f"Expected daily rate: {INITIAL_TOKENS_PER_DAY / _SATOSHI_MULTIPLIER:.{TOKEN_DECIMALS}f} {TOKEN_TICKER}"
    )

    # Ví dụ: Tính toán khi đã qua 2 lần halving (ví dụ: 16 triệu MIT đã phát hành)
    circ_supply_later = 16_000_000 * _SATOSHI_MULTIPLIER
    num_h = _calculate_num_halvings(circ_supply_later, TOTAL_SUPPLY)
    later_epoch_emission = calculate_emission_for_epoch(circ_supply_later, epoch_days)
    expected_daily_rate_later = INITIAL_TOKENS_PER_DAY / (2**num_h)
    print(f"\n--- Example 2: After {num_h} halvings ---")
    print(
        f"Circulating Supply: {circ_supply_later / _SATOSHI_MULTIPLIER:.{TOKEN_DECIMALS}f} {TOKEN_TICKER}"
    )
    print(f"Number of halvings occurred: {num_h}")
    print(
        f"Expected daily rate: {expected_daily_rate_later / _SATOSHI_MULTIPLIER:.{TOKEN_DECIMALS}f} {TOKEN_TICKER}"
    )
    print(
        f"Emission for {epoch_days}-day epoch at this stage: {later_epoch_emission / _SATOSHI_MULTIPLIER:.{TOKEN_DECIMALS}f} {TOKEN_TICKER} ({later_epoch_emission} units)"
    )

    # Ví dụ: Gần đạt tổng cung
    circ_supply_near_end = TOTAL_SUPPLY - (15 * _SATOSHI_MULTIPLIER)  # Còn 15 MIT nữa
    num_h_near_end = _calculate_num_halvings(circ_supply_near_end, TOTAL_SUPPLY)
    near_end_emission = calculate_emission_for_epoch(circ_supply_near_end, epoch_days)
    remaining = TOTAL_SUPPLY - circ_supply_near_end
    print(f"\n--- Example 3: Near total supply ---")
    print(
        f"Circulating Supply: {circ_supply_near_end / _SATOSHI_MULTIPLIER:.{TOKEN_DECIMALS}f} {TOKEN_TICKER}"
    )
    print(f"Number of halvings occurred: {num_h_near_end}")
    print(
        f"Remaining supply: {remaining / _SATOSHI_MULTIPLIER:.{TOKEN_DECIMALS}f} {TOKEN_TICKER} ({remaining} units)"
    )
    print(
        f"Emission calculated for {epoch_days}-day epoch: {near_end_emission / _SATOSHI_MULTIPLIER:.{TOKEN_DECIMALS}f} {TOKEN_TICKER} ({near_end_emission} units)"
    )
    # Lượng phát hành thực tế phải bằng phần còn lại
    assert (
        near_end_emission == remaining
    ), f"Expected {remaining} but got {near_end_emission}"
