# sdk/network/app/main.py
from fastapi import FastAPI
import Optional
import asyncio
import logging
import time # Thêm time

# Import các thành phần cần thiết
from .api.v1.routes import api_router
from .dependencies import set_validator_node_instance # Hàm để inject instance
from sdk.consensus.node import ValidatorNode
from sdk.core.datatypes import ValidatorInfo
from sdk.config.settings import settings # Import settings

logging.basicConfig(level=(settings.log_level.upper() if settings else "INFO"),
                    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

app = FastAPI(
    title="Moderntensor Network API",
    description="API endpoints for Moderntensor network communication, including P2P consensus.",
    version="0.1.0"
)

main_validator_node_instance: Optional[ValidatorNode] = None
main_loop_task: Optional[asyncio.Task] = None

@app.on_event("startup")
async def startup_event():
    """Khởi tạo Validator Node và inject vào dependency."""
    global main_validator_node_instance, main_loop_task
    logger.info("FastAPI application starting up...")
    if ValidatorInfo and ValidatorNode and settings:
        try:
            # --- Load thông tin validator từ settings hoặc nguồn khác ---
            validator_uid = settings.validator_uid or "V_DEFAULT" # Giả sử có trong settings
            validator_address = settings.validator_address or "addr_default..."
            api_endpoint = settings.validator_api_endpoint or f"http://127.0.0.1:{settings.api_port or 8000}"
            # cardano_ctx = await initialize_cardano_context(settings)
            # node_config_dict = settings.model_dump() # Có thể truyền cả dict settings

            my_validator_info = ValidatorInfo(
                uid=validator_uid, address=validator_address, api_endpoint=api_endpoint
            )

            # Khởi tạo Node (truyền settings vào đây nếu node không import global)
            main_validator_node_instance = ValidatorNode(
                validator_info=my_validator_info,
                cardano_context=None # cardano_ctx
                # config=node_config_dict # Hoặc để node tự import settings
            )

            # Inject instance
            set_validator_node_instance(main_validator_node_instance)
            logger.info(f"ValidatorNode instance '{validator_uid}' initialized and injected.")

            # Khởi chạy vòng lặp chính trong background task
            logger.info("Starting main consensus loop as background task...")
            main_loop_task = asyncio.create_task(run_main_node_loop(main_validator_node_instance))

        except Exception as e:
            logger.exception(f"Failed to initialize ValidatorNode: {e}")
    else:
        logger.error("SDK components or settings not available. Cannot initialize node.")

@app.on_event("shutdown")
async def shutdown_event():
    """Dọn dẹp tài nguyên."""
    logger.info("FastAPI application shutting down...")
    if main_loop_task and not main_loop_task.done():
        logger.info("Cancelling main node loop task...")
        main_loop_task.cancel()
        try:
            await main_loop_task # Chờ task kết thúc sau khi cancel
        except asyncio.CancelledError:
            logger.info("Main node loop task cancelled successfully.")
    if main_validator_node_instance and hasattr(main_validator_node_instance, 'http_client'):
        await main_validator_node_instance.http_client.aclose()
        logger.info("HTTP client closed.")

async def run_main_node_loop(node: ValidatorNode):
    """Hàm chạy vòng lặp đồng thuận chính trong background."""
    # ... (Logic vòng lặp while True như trong node.py) ...
    if not node or not settings: return
    try:
        # Chờ một chút để FastAPI sẵn sàng nhận request nếu cần
        await asyncio.sleep(5)
        while True:
            cycle_start_time = time.time()
            await node.run_cycle()
            cycle_duration = time.time() - cycle_start_time
            cycle_interval_seconds = settings.consensus_metagraph_update_interval_minutes * 60
            min_wait = settings.consensus_cycle_min_wait_seconds
            wait_time = max(min_wait, cycle_interval_seconds - cycle_duration)
            logger.info(f"Cycle duration: {cycle_duration:.1f}s. Waiting {wait_time:.1f}s for next cycle...")
            await asyncio.sleep(wait_time)
    except asyncio.CancelledError:
        logger.info("Main node loop cancelled.")
    except Exception as e:
        logger.exception(f"Exception in main node loop: {e}")
    finally:
        logger.info("Main node loop finished.")


# --- Include Routers ---
app.include_router(api_router)

# --- Điểm chạy chính (uvicorn) ---
# ...
