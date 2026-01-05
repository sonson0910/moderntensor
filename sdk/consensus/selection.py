# sdk/consensus/selection.py
"""
Logic chọn lựa miners cho chu trình đồng thuận.
"""
import random
import logging
from typing import List, Dict

# Import các thành phần cần thiết
try:
    from sdk.config.settings import settings
    from sdk.core.datatypes import MinerInfo
    from sdk.metagraph.metagraph_datum import STATUS_ACTIVE

    # from sdk.formulas.trust_score import calculate_selection_probability # Deprecated logic
except ImportError as e:
    raise ImportError(f"Error importing dependencies in selection.py: {e}")

logger = logging.getLogger(__name__)


def select_miners_logic(
    miners_info: Dict[str, MinerInfo],
    current_cycle: int,
    num_to_select: int,
    beta: float,
    max_time_bonus: int,
) -> List[MinerInfo]:
    """
    Logic chọn miners sử dụng chiến lược lai (Hybrid Strategy): Exploitation + Exploration.

    Mô phỏng cơ chế của Bittensor/Yuma Consensus:
    1. Exploitation (Top-Tier): Chọn một tỷ lệ lớn (ví dụ 70%) các miner có Trust Score cao nhất.
       Đảm bảo chất lượng dịch vụ tốt nhất cho mạng lưới.
    2. Exploration (Random/Weighted): Chọn ngẫu nhiên từ các miner còn lại (bao gồm cả miner mới/trust thấp).
       Đảm bảo tính công bằng và cơ hội cho các nhân tố mới (tránh "Rich get richer" tuyệt đối).

    Args:
        miners_info: Dictionary chứa thông tin các miner hiện có ({uid: MinerInfo}).
        current_cycle: Chu kỳ hiện tại.
        num_to_select: Tổng số lượng miner cần chọn.
        beta: (Không còn dùng trực tiếp trong logic mới, giữ lại để tương thích API).
        max_time_bonus: (Không còn dùng trực tiếp trong logic mới).

    Returns:
        Danh sách các MinerInfo đã được chọn.
    """
    if not miners_info:
        logger.warning(
            "No miners available in the current metagraph state for selection."
        )
        return []

    # 1. Lọc Active Miners
    active_miners = [
        m
        for m in miners_info.values()
        if getattr(m, "status", STATUS_ACTIVE) == STATUS_ACTIVE
    ]

    if not active_miners:
        logger.warning("No active miners found for selection.")
        return []

    # Nếu số lượng active ít hơn số cần chọn, chọn tất cả
    if len(active_miners) <= num_to_select:
        logger.info(
            f"Not enough active miners ({len(active_miners)}) to satisfy request ({num_to_select}). Selecting all."
        )
        return active_miners

    # 2. Phân bổ số lượng (70% Top-Tier, 30% Random)
    # Tỷ lệ này có thể đưa vào settings trong tương lai
    top_tier_ratio = 0.7
    num_top = int(num_to_select * top_tier_ratio)
    num_random = num_to_select - num_top

    # 3. Sắp xếp theo Trust Score giảm dần
    # Sử dụng uid làm key phụ để đảm bảo thứ tự ổn định (stable sort)
    sorted_miners = sorted(
        active_miners, key=lambda m: (m.trust_score, m.uid), reverse=True
    )

    # 4. Chọn Top-Tier (Exploitation)
    selected_miners = sorted_miners[:num_top]
    remaining_miners = sorted_miners[num_top:]

    logger.debug(
        f"Selected {len(selected_miners)} Top-Tier miners (Trust > {selected_miners[-1].trust_score if selected_miners else 0})."
    )

    # 5. Chọn Random (Exploration) từ phần còn lại
    # Phần này giúp miner mới (trust=0) hoặc miner cũ (trust thấp) có cơ hội được đánh giá lại
    if remaining_miners and num_random > 0:
        # Đảm bảo không chọn quá số lượng còn lại
        k = min(num_random, len(remaining_miners))
        random_selection = random.sample(remaining_miners, k)
        selected_miners.extend(random_selection)
        logger.debug(f"Selected {len(random_selection)} Random miners for exploration.")

    logger.info(
        f"Selected {len(selected_miners)} unique miners (Top: {num_top}, Rnd: {num_random}). UIDs: {[m.uid for m in selected_miners]}"
    )

    return selected_miners
