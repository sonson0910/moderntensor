# sdk/network/app/main.py
from fastapi import FastAPI

# from typing import Optional # Thường đã được import gián tiếp
import asyncio
import logging  # Import logging
import time
import os  # Import os nếu dùng getenv

# Import các thành phần cần thiết
from .api.v1.routes import api_router
from .dependencies import set_validator_node_instance
from sdk.keymanager.decryption_utils import decode_hotkey_skey
from sdk.consensus.node import ValidatorNode
from sdk.core.datatypes import ValidatorInfo

# Import settings (sẽ kích hoạt cấu hình logging trong settings.py)
from sdk.config.settings import settings
from sdk.service.context import get_chain_context

from sdk.compat.luxtensor_types import ExtendedSigningKey
from typing import Optional

# Lấy logger đã được cấu hình trong settings.py

logger = logging.getLogger(__name__)

app = FastAPI(
    title="Moderntensor Network API",
    description="API endpoints for Moderntensor network communication, including P2P consensus.",
    version="0.1.0",
)

main_validator_node_instance: Optional[ValidatorNode] = None
main_loop_task: Optional[asyncio.Task] = None


# --- Hàm startup_event ---
@app.on_event("startup")
async def startup_event():
    """Khởi tạo Validator Node và inject vào dependency."""
    global main_validator_node_instance, main_loop_task
    logger.info("FastAPI application starting up...")
    if ValidatorInfo and ValidatorNode and settings:
        try:
            # Lấy thông tin validator từ settings
            validator_uid = settings.VALIDATOR_UID or "V_DEFAULT_API"
            validator_address = settings.VALIDATOR_ADDRESS or "addr_default_api..."
            host = os.getenv("HOST", "127.0.0.1")
            port = settings.API_PORT
            api_endpoint = settings.VALIDATOR_API_ENDPOINT or f"http://{host}:{port}"

            # --- KHỞI TẠO CONTEXT CARDANO ---
            cardano_ctx = get_chain_context(
                "blockfrost"
            )  # Giả sử hàm này hoạt động đúng
            if not cardano_ctx:
                raise RuntimeError(
                    "Node initialization failed: Could not initialize Cardano context."
                )

            # --- LOAD SIGNING KEYS (SỬ DỤNG decode_hotkey_skey) ---
            signing_key: Optional[ExtendedSigningKey] = None
            stake_signing_key: Optional[ExtendedSigningKey] = None
            try:
                logger.info(
                    "Attempting to load signing keys using decode_hotkey_skey..."
                )
                base_dir = settings.HOTKEY_BASE_DIR
                coldkey_name = settings.COLDKEY_NAME
                hotkey_name = settings.HOTKEY_NAME
                password = settings.HOTKEY_PASSWORD  # Lưu ý bảo mật khi lấy password

                # Gọi hàm decode để lấy ExtendedSigningKeys
                payment_esk, stake_esk = decode_hotkey_skey(
                    base_dir=base_dir,
                    coldkey_name=coldkey_name,
                    hotkey_name=hotkey_name,
                    password=password,
                )

                # Gán trực tiếp kết quả vì ValidatorNode.__init__ đã nhận ExtendedSigningKey
                signing_key = payment_esk  # type: ignore
                stake_signing_key = stake_esk  # type: ignore

                if not signing_key:
                    # decode_hotkey_skey nên raise lỗi nếu thất bại, nhưng kiểm tra lại cho chắc
                    raise ValueError(
                        "Failed to load required payment signing key (decode_hotkey_skey returned None)."
                    )

                logger.info(
                    f"Successfully loaded keys for hotkey '{hotkey_name}' under coldkey '{coldkey_name}'."
                )

            except FileNotFoundError as fnf_err:
                logger.exception(
                    f"Failed to load signing keys: Hotkey file or directory not found. Details: {fnf_err}"
                )
                raise RuntimeError(
                    f"Node initialization failed: Hotkey file not found ({fnf_err}). Check HOTKEY_BASE_DIR, COLDKEY_NAME, HOTKEY_NAME settings."
                ) from fnf_err
            except Exception as key_err:
                logger.exception(f"Failed to load/decode signing keys: {key_err}")
                raise RuntimeError(
                    f"Node initialization failed: Could not load/decode keys ({key_err}). Check password or key files."
                ) from key_err
            # --- KẾT THÚC LOAD KEYS ---

            # Tạo ValidatorInfo
            my_validator_info = ValidatorInfo(
                uid=validator_uid,
                address=validator_address,
                api_endpoint=api_endpoint,
                # Thêm các trường khác nếu cần
            )

            # Khởi tạo ValidatorNode với ExtendedSigningKeys
            main_validator_node_instance = ValidatorNode(
                validator_info=my_validator_info,
                cardano_context=cardano_ctx,
                signing_key=signing_key,  # Truyền ExtendedSigningKey
                stake_signing_key=stake_signing_key,  # Truyền ExtendedSigningKey (hoặc None)
            )

            # Inject và chạy loop (giữ nguyên)
            set_validator_node_instance(main_validator_node_instance)
            logger.info(
                f"ValidatorNode instance '{validator_uid}' initialized and injected."
            )
            # logger.info("Starting main consensus loop as background task...")
            # main_loop_task = asyncio.create_task(run_main_node_loop(main_validator_node_instance))

        except Exception as e:
            # Bắt các lỗi khác trong quá trình khởi tạo
            logger.exception(
                f"Failed to initialize ValidatorNode during API startup: {e}"
            )
            # Có thể muốn dừng hẳn ứng dụng FastAPI ở đây nếu node không khởi tạo được
            # raise e # Ném lỗi ra ngoài để FastAPI dừng lại
    else:
        logger.error(
            "SDK components (ValidatorNode/Info) or settings not available. Cannot initialize node."
        )


@app.on_event("shutdown")
async def shutdown_event():
    """Dọn dẹp tài nguyên."""
    logger.info("FastAPI application shutting down...")
    if main_loop_task and not main_loop_task.done():
        logger.info("Cancelling main node loop task...")
        main_loop_task.cancel()
        try:
            await main_loop_task  # Chờ task kết thúc sau khi cancel
        except asyncio.CancelledError:
            logger.info("Main node loop task cancelled successfully.")
    if main_validator_node_instance and hasattr(
        main_validator_node_instance, "http_client"
    ):
        await main_validator_node_instance.http_client.aclose()
        logger.info("HTTP client closed.")


async def run_main_node_loop(node: ValidatorNode):
    """Hàm chạy vòng lặp đồng thuận chính trong background."""
    # ... (Logic vòng lặp while True như trong node.py) ...
    if not node or not settings:
        return
    try:
        # Chờ một chút để FastAPI sẵn sàng nhận request nếu cần
        await asyncio.sleep(5)
        while True:
            cycle_start_time = time.time()
            await node.run_cycle()
            cycle_duration = time.time() - cycle_start_time
            cycle_interval_seconds = (
                settings.CONSENSUS_METAGRAPH_UPDATE_INTERVAL_MINUTES * 60
            )
            min_wait = settings.CONSENSUS_CYCLE_MIN_WAIT_SECONDS
            wait_time = max(min_wait, cycle_interval_seconds - cycle_duration)
            logger.info(
                f"Cycle duration: {cycle_duration:.1f}s. Waiting {wait_time:.1f}s for next cycle..."
            )
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
