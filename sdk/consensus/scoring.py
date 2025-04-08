# sdk/consensus/scoring.py
"""
Logic chấm điểm kết quả từ miners.
Chứa hàm cơ sở cần được kế thừa và triển khai bởi các validator/subnet cụ thể.
"""
import logging
from typing import List, Dict, Any
from collections import defaultdict
# Giả định các kiểu dữ liệu này đã được import hoặc định nghĩa đúng
try:
    from sdk.core.datatypes import MinerResult, TaskAssignment, ValidatorScore
except ImportError as e:
    raise ImportError(f"Error importing dependencies in scoring.py: {e}")

logger = logging.getLogger(__name__)

# --- 1. Đánh dấu hàm này cần override ---
def _calculate_score_from_result(task_data: Any, result_data: Any) -> float:
    """
    (Trừu tượng/Cần Override) Tính điểm P_miner,v từ dữ liệu task và kết quả.

    Lớp Validator kế thừa cho từng Subnet PHẢI override phương thức này
    với logic chấm điểm phù hợp cho loại nhiệm vụ AI của họ.

    Args:
        task_data: Dữ liệu của nhiệm vụ đã giao (từ TaskAssignment.task_data).
        result_data: Dữ liệu kết quả mà miner trả về (từ MinerResult.result_data).

    Returns:
        Điểm số (từ 0.0 đến 1.0).

    Raises:
        NotImplementedError: Nếu phương thức này không được override bởi lớp con.
    """
    # Hoặc trả về điểm mặc định và log warning:
    logger.warning(
        f"'_calculate_score_from_result' is using the base implementation. "
        f"This should be overridden by specific subnet/validator logic. "
        f"Task data type: {type(task_data)}, Result data type: {type(result_data)}. "
        f"Returning default score 0.0"
    )
    return 0.0
    # Hoặc raise lỗi để bắt buộc override:
    # raise NotImplementedError("Scoring logic must be implemented by validator subclass/subnet.")
# ---------------------------------------

def score_results_logic(
    results_received: Dict[str, List[MinerResult]],
    tasks_sent: Dict[str, TaskAssignment],
    validator_uid: str,
) -> Dict[str, List[ValidatorScore]]:
    """
    Chấm điểm kết quả nhận được từ miners bằng cách gọi hàm chấm điểm
    _calculate_score_from_result (có thể đã được override).
    """
    logger.info(f"[V:{validator_uid}] Scoring {len(results_received)} received tasks using defined logic...")
    validator_scores: Dict[str, List[ValidatorScore]] = defaultdict(list)

    for task_id, results in results_received.items():
        assignment = tasks_sent.get(task_id)
        if not assignment:
            logger.warning(f"Scoring skipped: Task assignment not found for task_id {task_id}.")
            continue

        # Chỉ chấm điểm kết quả đầu tiên hợp lệ từ đúng miner? Hay chấm tất cả?
        # Tạm thời chấm kết quả đầu tiên từ đúng miner
        valid_result_found = False
        for result in results:
            if result.miner_uid == assignment.miner_uid:
                # Gọi hàm chấm điểm (có thể là hàm đã override)
                try:
                    score = _calculate_score_from_result(assignment.task_data, result.result_data)
                    # Đảm bảo điểm nằm trong khoảng [0, 1]
                    score = max(0.0, min(1.0, score))
                    valid_result_found = True # Đánh dấu đã tìm thấy kết quả hợp lệ
                except NotImplementedError:
                    logger.error(f"Scoring logic not implemented for task {task_id}! Assigning score 0.")
                    score = 0.0
                    valid_result_found = True # Vẫn coi như đã xử lý
                except Exception as e:
                    logger.exception(f"Error calculating score for task {task_id}, miner {result.miner_uid}: {e}. Assigning score 0.")
                    score = 0.0
                    # Có nên coi đây là kết quả hợp lệ để dừng không? Tạm thời không.
                    continue # Thử kết quả tiếp theo nếu có lỗi

                logger.info(f"  Scored Miner {result.miner_uid} for task {task_id}: {score:.4f}")

                val_score = ValidatorScore(
                    task_id=task_id,
                    miner_uid=result.miner_uid,
                    validator_uid=validator_uid,
                    score=score
                )
                validator_scores[task_id].append(val_score)
                break # Chỉ chấm điểm kết quả hợp lệ đầu tiên từ đúng miner

        if not valid_result_found:
             logger.warning(f"No valid result found from expected miner {assignment.miner_uid} for task {task_id}. No score generated.")
             # Có thể tạo điểm 0 cho miner nếu không có kết quả hợp lệ?
             # val_score = ValidatorScore(...)
             # validator_scores[task_id].append(val_score)

    logger.info(f"Finished scoring. Generated scores for {len(validator_scores)} tasks.")
    return dict(validator_scores)