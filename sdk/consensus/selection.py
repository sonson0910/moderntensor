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
    from sdk.formulas import calculate_selection_probability
except ImportError as e:
    raise ImportError(f"Error importing dependencies in selection.py: {e}")

logger = logging.getLogger(__name__)

def select_miners_logic(
    miners_info: Dict[str, MinerInfo],
    current_cycle: int,
    num_to_select: int,
    beta: float,
    max_time_bonus: int
) -> List[MinerInfo]:
    """
    Logic chọn miners dựa trên trust score và thời gian chờ.

    Args:
        miners_info: Dictionary chứa thông tin các miner hiện có.
        current_cycle: Chu kỳ hiện tại.
        num_to_select: Số lượng miner cần chọn.
        beta: Hệ số bonus công bằng (từ settings).
        max_time_bonus: Giới hạn bonus thời gian chờ (từ settings).

    Returns:
        Danh sách các MinerInfo đã được chọn.
    """
    if not miners_info:
        logger.warning("No miners available in the current metagraph state for selection.")
        return []

    # Chỉ xem xét các miner đang hoạt động
    active_miners = [m for m in miners_info.values() if getattr(m, 'status', STATUS_ACTIVE) == STATUS_ACTIVE]
    if not active_miners:
         logger.warning("No active miners found for selection.")
         return []

    logger.debug(f"Found {len(active_miners)} active miners to consider for selection.")

    miner_probabilities = []
    total_prob_factor = 0.0

    for miner in active_miners:
        time_since = max(0, current_cycle - miner.last_selected_time)
        prob_factor = calculate_selection_probability(
            trust_score=miner.trust_score,
            time_since_last_selection=time_since,
            beta=beta,
            max_time_bonus_effect=max_time_bonus
        )
        prob_factor = max(0.0, prob_factor) # Đảm bảo không âm
        miner_probabilities.append((miner, prob_factor))
        total_prob_factor += prob_factor
        logger.debug(f"Miner {miner.uid}: Trust={miner.trust_score:.3f}, TimeSince={time_since}, ProbFactor={prob_factor:.4f}")

    if total_prob_factor <= 1e-9:
        logger.warning("Total probability factor is zero or negligible. Selecting randomly among active miners.")
        k = min(num_to_select, len(active_miners))
        selected_miners = random.sample(active_miners, k)
        logger.info(f"Randomly selected {len(selected_miners)} miners: {[m.uid for m in selected_miners]}")
        return selected_miners

    try:
        probabilities = [p / total_prob_factor for _, p in miner_probabilities]
    except ZeroDivisionError:
         logger.error("Division by zero during probability normalization. Selecting randomly.")
         k = min(num_to_select, len(active_miners))
         selected_miners = random.sample(active_miners, k)
         logger.info(f"Randomly selected {len(selected_miners)} miners due to normalization error.")
         return selected_miners

    miners_population = [m for m, _ in miner_probabilities]

    selected_miners_dict: Dict[str, MinerInfo] = {}
    attempts = 0
    max_attempts = num_to_select * 5
    target_count = min(num_to_select, len(miners_population))

    while len(selected_miners_dict) < target_count and attempts < max_attempts:
         try:
             # Chọn có trọng số, không thay thế (nếu dùng random.sample với weights - cần numpy hoặc logic phức tạp hơn)
             # random.choices cho phép chọn lại, nên dùng dict để lọc trùng
             chosen_miner = random.choices(population=miners_population, weights=probabilities, k=1)[0]
             selected_miners_dict[chosen_miner.uid] = chosen_miner
         except IndexError:
              logger.warning("Random choices returned empty list unexpectedly.")
              break
         except Exception as e:
              logger.exception(f"Error during random weighted choice: {e}")
              break
         attempts += 1

    selected_miners = list(selected_miners_dict.values())

    if len(selected_miners) < target_count:
         logger.warning(f"Could only select {len(selected_miners)} unique miners out of {target_count} requested after {max_attempts} attempts.")

    logger.info(f"Selected {len(selected_miners)} unique miners: {[m.uid for m in selected_miners]}")
    return selected_miners
