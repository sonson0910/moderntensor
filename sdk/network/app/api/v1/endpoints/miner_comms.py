# File mới: sdk/network/app/api/v1/endpoints/miner_comms.py

import logging
from typing import Annotated
from fastapi import APIRouter, Depends, HTTPException, status

# Import các model và dependencies
from sdk.network.server import ResultModel # Model kết quả từ miner
from sdk.core.datatypes import MinerResult # Datatype nội bộ
from sdk.consensus.node import ValidatorNode
from sdk.network.app.dependencies import get_validator_node
import time

logger = logging.getLogger(__name__)
router = APIRouter()

@router.post("/miner/submit_result",
             summary="Miner gửi kết quả Task",
             description="Endpoint để Miner gửi đối tượng ResultModel sau khi hoàn thành task.",
             status_code=status.HTTP_202_ACCEPTED)
async def submit_miner_result(
    result_payload: ResultModel, # Nhận payload từ miner
    node: Annotated[ValidatorNode, Depends(get_validator_node)]
):
    """
    Nhận kết quả từ Miner, chuyển đổi thành MinerResult và thêm vào node.
    """
    logger.info(f"API: Received result submission for task {result_payload.result_id} from miner {result_payload.miner_id}")

    # --- Chuyển đổi ResultModel (API) sang MinerResult (Core) ---
    # Cần ánh xạ các trường. Giả sử result_id chứa task_id
    # Cần kiểm tra lại ResultModel xem có đủ thông tin không
    # Giả sử result_payload.result_id thực chất là task_id
    # Giả sử result_payload.description chứa dữ liệu kết quả chính
    try:
        internal_result = MinerResult(
            task_id=result_payload.result_id, # Hoặc lấy từ field khác nếu có
            miner_uid=result_payload.miner_id,
            # --- result_data cần chứa tất cả thông tin miner trả về ---
            result_data={
                "description": result_payload.description,
                "processing_time": result_payload.processing_time,
                # Thêm các trường khác từ ResultModel nếu cần cho việc chấm điểm
            },
            # --------------------------------------------------------
            timestamp_received=time.time()
        )
    except Exception as convert_e:
         logger.error(f"API: Failed to convert received payload to MinerResult: {convert_e}")
         raise HTTPException(status_code=status.HTTP_422_UNPROCESSABLE_ENTITY, detail="Invalid result payload format.")
    # ----------------------------------------------------------

    # --- Gọi phương thức của Node để thêm kết quả ---
    # TODO: Thêm xác thực chữ ký của Miner nếu cần
    success = await node.add_miner_result(internal_result)
    # -------------------------------------------

    if success:
        return {"message": f"Result for task {internal_result.task_id} accepted."}
    else:
        # Lý do fail đã được log trong add_miner_result
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail="Result rejected (e.g., unknown task, wrong miner).")