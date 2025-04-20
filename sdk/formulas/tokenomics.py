"""
Contains formulas and logic related to the project's tokenomics,
including issuance calculation based on epochs and halving.
"""

import math
import logging
from sdk.config.settings import settings
from typing import Dict, Any

logger = logging.getLogger(__name__)

# Import các hằng số cần thiết (hoặc định nghĩa lại EPSILON)
# Giả sử các hằng số này có thể truy cập được hoặc được truyền vào
# Nếu không, cần import từ đúng module, ví dụ: from sdk.metagraph.metagraph_datum import STATUS_ACTIVE
# Hoặc định nghĩa lại:
STATUS_ACTIVE = 1  # Giả định giá trị của STATUS_ACTIVE
EPSILON = 1e-9  # Giá trị rất nhỏ để tránh chia cho 0


def get_current_epoch(current_slot: int) -> int:
    """Calculates the current tokenomics epoch based on the slot."""
    epoch_length = settings.TOKEN_EPOCH_LENGTH_SLOTS
    if epoch_length <= 0:
        logger.error("TOKEN_EPOCH_LENGTH_SLOTS must be positive. Returning epoch 0.")
        return 0  # Avoid division by zero and invalid state
    return current_slot // epoch_length


def calculate_epoch_issuance(epoch: int) -> int:
    """
    Calculates the number of new tokens (in the smallest unit, like Lovelace)
    to be issued in a given epoch, applying the halving mechanism.

    Args:
        epoch (int): The epoch number for which to calculate issuance.

    Returns:
        int: The amount of new tokens to issue in this epoch (smallest unit).
    """
    initial_issuance = settings.TOKEN_INITIAL_ISSUANCE_PER_EPOCH
    halving_interval = settings.TOKEN_HALVING_INTERVAL_EPOCHS

    if halving_interval <= 0:
        logger.warning(
            "TOKEN_HALVING_INTERVAL_EPOCHS is not positive. Halving disabled."
        )
        return initial_issuance  # No halving

    if epoch < 0:
        logger.warning(f"Received negative epoch ({epoch}). Returning 0 issuance.")
        return 0

    # Calculate the number of halvings that have occurred up to this epoch
    # Note: Epochs start from 0. Halving happens *at* the interval.
    # E.g., if interval is 10, halving occurs at start of epoch 10, 20, 30...
    num_halvings = epoch // halving_interval

    # Limit the number of halvings to prevent issuance becoming excessively small or zero due to float precision
    # 64 halvings reduce issuance by 2^64, which is a massive reduction.
    max_halvings = 64
    num_halvings = min(num_halvings, max_halvings)

    # Apply halving
    # Use integer division or careful float handling if needed, but starting with float is fine
    current_issuance_float = initial_issuance / (2**num_halvings)

    # Convert to integer (smallest unit). Using floor to be conservative.
    current_issuance_int = math.floor(current_issuance_float)

    # TODO: Implement logic to check against TOKEN_TOTAL_SUPPLY.
    # This requires knowing the total tokens already issued, which needs
    # to be tracked either off-chain (e.g., in validator state) or ideally
    # read from an on-chain source (e.g., total supply in the minting script state).
    # For now, this function only calculates the theoretical issuance for the epoch.

    logger.debug(
        f"Epoch {epoch}: Num Halvings = {num_halvings}, Calculated Issuance = {current_issuance_int}"
    )
    return current_issuance_int


# --- HÀM MỚI: Phân phối Issuance ---
def distribute_issuance(
    issuance_this_epoch: int,  # Tổng số token mới (đơn vị nhỏ nhất)
    calculated_states: Dict[str, Any],  # Kết quả từ run_consensus_logic
    # Giả định calculated_states[uid] chứa ít nhất: 'contribution', 'start_status'
    status_active_val: int = STATUS_ACTIVE,  # Có thể truyền vào nếu giá trị thay đổi
) -> Dict[str, int]:  # Trả về dict {uid_hex: issuance_share (đơn vị nhỏ nhất)}
    """
    Distributes the epoch's total issuance proportionally based on calculated contributions
    of active validators/miners.

    Args:
        issuance_this_epoch (int): Total new tokens (smallest unit) to distribute.
        calculated_states (Dict[str, Any]): The calculated states from consensus logic,
                                           expected to contain 'contribution' and 'start_status'.
        status_active_val (int): The integer value representing the ACTIVE status.

    Returns:
        Dict[str, int]: A dictionary mapping entity UIDs (hex) to their calculated
                        share of the issuance (in the smallest unit).
    """
    distribution: Dict[str, int] = {}
    if issuance_this_epoch <= 0:
        logger.info("Issuance for epoch is zero or negative, no distribution needed.")
        return distribution

    # 1. Tính tổng contribution của các node đang hoạt động
    total_contribution = 0.0
    active_node_contributions: Dict[str, float] = {}

    for uid_hex, state in calculated_states.items():
        if state.get("start_status") == status_active_val:
            contribution = state.get("contribution", 0.0)
            if contribution > 0:  # Chỉ tính những ai có đóng góp dương
                active_node_contributions[uid_hex] = contribution
                total_contribution += contribution

    logger.debug(f"Total contribution from active nodes: {total_contribution:.4f}")

    # 2. Phân phối issuance nếu có đóng góp
    if total_contribution <= EPSILON:
        logger.warning(
            "Total contribution from active nodes is zero or negligible. Cannot distribute issuance."
        )
        return distribution  # Không có ai đóng góp, không phân phối

    distributed_total = 0  # Theo dõi tổng số đã phân phối (do làm tròn)
    for uid_hex, contribution in active_node_contributions.items():
        share_percentage = contribution / total_contribution
        # Tính phần share dạng float trước
        issuance_share_float = issuance_this_epoch * share_percentage
        # Chuyển thành int (làm tròn xuống để bảo toàn)
        issuance_share_int = math.floor(issuance_share_float)

        if issuance_share_int > 0:
            distribution[uid_hex] = issuance_share_int
            distributed_total += issuance_share_int
            logger.debug(
                f"  UID {uid_hex}: Contribution={contribution:.4f}, Share={share_percentage:.4%}, Issuance={issuance_share_int}"
            )

    # 3. Xử lý phần dư (dust) do làm tròn
    remainder = issuance_this_epoch - distributed_total
    if remainder > 0 and active_node_contributions:
        logger.warning(
            f"Issuance remainder due to floor rounding: {remainder} units. Distributing to highest contributor."
        )
        # Tìm người đóng góp nhiều nhất để nhận phần dư
        # Sắp xếp theo contribution giảm dần
        sorted_contributors = sorted(
            active_node_contributions.items(), key=lambda item: item[1], reverse=True
        )
        if sorted_contributors:
            top_contributor_uid = sorted_contributors[0][0]
            distribution[top_contributor_uid] = (
                distribution.get(top_contributor_uid, 0) + remainder
            )
            logger.info(
                f"Remainder {remainder} units added to top contributor: {top_contributor_uid}"
            )
            distributed_total += remainder  # Cập nhật lại tổng đã phân phối

    logger.info(
        f"Issuance distribution complete. Target: {issuance_this_epoch}, Distributed: {distributed_total}"
    )
    if distributed_total != issuance_this_epoch:
        logger.error(
            f"DISCREPANCY in issuance distribution! Target={issuance_this_epoch}, Distributed={distributed_total}"
        )

    return distribution


# Example Usage (can be removed later):
# if __name__ == "__main__":
#     print(f"Settings: Epoch Length={settings.TOKEN_EPOCH_LENGTH_SLOTS}, Halving Interval={settings.TOKEN_HALVING_INTERVAL_EPOCHS}, Initial Issuance={settings.TOKEN_INITIAL_ISSUANCE_PER_EPOCH}")
#     for test_epoch in [0, 1, settings.TOKEN_HALVING_INTERVAL_EPOCHS - 1, settings.TOKEN_HALVING_INTERVAL_EPOCHS, settings.TOKEN_HALVING_INTERVAL_EPOCHS + 1, 2 * settings.TOKEN_HALVING_INTERVAL_EPOCHS]:
#         issuance = calculate_epoch_issuance(test_epoch)
#         print(f"Issuance for Epoch {test_epoch}: {issuance} ({(issuance / (10**settings.TOKEN_DECIMALS)):.6f} tokens)")
#
#     slot_now = 100_000_000 # Example slot
#     current_ep = get_current_epoch(slot_now)
#     print(f"\nCurrent Slot: {slot_now}, Current Epoch: {current_ep}")
#     print(f"Issuance for Current Epoch: {calculate_epoch_issuance(current_ep)}")
