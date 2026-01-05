import uvicorn
import asyncio
import logging
import os
import sys
import time
from typing import Type, Optional, Dict, Any
from fastapi import FastAPI
from pathlib import Path

# Import các thành phần cần thiết từ SDK
# Lưu ý: Các đường dẫn import này là tương đối trong SDK
from .network.app.api.v1.routes import api_router
from .network.app.dependencies import set_validator_node_instance
from .consensus.node import ValidatorNode  # Import lớp base
from .core.datatypes import ValidatorInfo
from .keymanager.decryption_utils import decode_hotkey_skey
from .service.context import get_chain_context
from .config.settings import (
    settings as sdk_settings,
)  # Dùng settings SDK cho giá trị mặc định
from sdk.compat.pycardano import ExtendedSigningKey, BlockFrostChainContext, Network

logger = logging.getLogger(__name__)


class ValidatorRunner:
    """
    Lớp điều phối việc khởi tạo, cấu hình và chạy một node Validator đầy đủ.

    Bao gồm việc thiết lập API server (FastAPI), khởi tạo instance của lớp
    ValidatorNode cụ thể được cung cấp, và chạy vòng lặp đồng thuận chính
    của node trong một background task.

    Attributes:
        config (Dict[str, Any]): Dictionary cấu hình được truyền vào khi khởi tạo.
        validator_class (Type[ValidatorNode]): Lớp ValidatorNode cụ thể của subnet.
        app (FastAPI): Instance của FastAPI dùng để chạy API server.
        validator_node_instance (Optional[ValidatorNode]): Instance của ValidatorNode
            sau khi được khởi tạo thành công trong sự kiện startup.
        main_loop_task (Optional[asyncio.Task]): Task chạy vòng lặp đồng thuận chính.
    """

    def __init__(self, config: Dict[str, Any]):
        """
        Khởi tạo ValidatorRunner với cấu hình được cung cấp.

        Args:
            config (Dict[str, Any]): Dictionary chứa các cấu hình cần thiết để
                khởi tạo ValidatorNode và chạy API server. Bao gồm các key như:
                - validator_class (Type[ValidatorNode]): Lớp validator cụ thể.
                - host (str): Địa chỉ IP host cho API server.
                - port (int): Cổng cho API server.
                - log_level (str): Cấp độ log (vd: 'info', 'debug').
                - validator_uid (str): UID hex của validator on-chain.
                - validator_address (str): Địa chỉ Cardano của validator.
                - validator_api_endpoint (str): URL công khai của API server.
                - hotkey_base_dir (str): Đường dẫn thư mục gốc chứa coldkeys.
                - coldkey_name (str): Tên thư mục của coldkey.
                - hotkey_name (str): Tên của hotkey cần load.
                - password (str): Mật khẩu để giải mã hotkey.
                - blockfrost_project_id (str): ID dự án Blockfrost.
                - network (Network): Mạng Cardano (MAINNET hoặc TESTNET).
                - ... (Các cấu hình bổ sung mà validator_class yêu cầu).
        """
        self.config = config
        self.validator_class: Type[ValidatorNode] = config.get(
            "validator_class", ValidatorNode
        )
        self.app = FastAPI(
            title=f"Moderntensor Validator API - {self.validator_class.__name__}",
            description=f"API server and consensus node runner for {self.validator_class.__name__}.",
            # version= # Có thể lấy version từ SDK
        )
        self.validator_node_instance: Optional[ValidatorNode] = None
        self.main_loop_task: Optional[asyncio.Task] = None

        self._setup_app()

    def _setup_app(self):
        """
        Thiết lập instance FastAPI (`self.app`).

        Bao gồm việc đăng ký các sự kiện startup và shutdown:
        - Startup: Khởi tạo instance ValidatorNode, load keys, context,
          inject node instance vào dependencies, và bắt đầu vòng lặp đồng thuận.
        - Shutdown: Hủy bỏ task vòng lặp đồng thuận và đóng các tài nguyên
          (như HTTP client của node) nếu cần.

        Đồng thời, include API router từ `sdk.network.app.api.v1.routes`.
        """

        @self.app.on_event("startup")
        async def startup_event():
            logger.info(
                f"Runner: FastAPI application startup for {self.validator_class.__name__}..."
            )
            try:
                # --- Load Config & Components ---
                cfg = self.config
                base_dir = cfg["hotkey_base_dir"]
                coldkey_name = cfg["coldkey_name"]
                hotkey_name = cfg["hotkey_name"]
                password = cfg["password"]
                validator_uid = cfg["validator_uid"]
                validator_address = cfg["validator_address"]
                api_endpoint = cfg["validator_api_endpoint"]
                bf_project_id = cfg["blockfrost_project_id"]
                network = cfg["network"]

                # --- Load Keys ---
                logger.info(f"Runner: Loading keys for hotkey '{hotkey_name}'...")
                payment_esk, stake_esk = decode_hotkey_skey(
                    base_dir, coldkey_name, hotkey_name, password
                )
                if not payment_esk:
                    raise ValueError("Failed to decode payment signing key.")
                logger.info("Runner: Signing keys loaded.")

                # --- Get Context ---
                logger.info(
                    f"Runner: Initializing Cardano context (Network: {network.name})..."
                )
                # Cần đảm bảo get_chain_context sử dụng đúng project_id
                # Có thể cần sửa get_chain_context hoặc truyền project_id vào đây
                cardano_context = get_chain_context(method="blockfrost")
                if not cardano_context:
                    raise RuntimeError("get_chain_context returned None.")
                # cardano_context.network = network # Đảm bảo đúng network
                logger.info("Runner: Cardano context initialized.")

                # --- Create ValidatorInfo ---
                validator_info = ValidatorInfo(
                    uid=validator_uid,
                    address=validator_address,
                    api_endpoint=api_endpoint,
                )

                # --- Instantiate the SPECIFIC Validator Class ---
                logger.info(
                    f"Runner: Instantiating {self.validator_class.__name__} '{validator_uid}'..."
                )
                self.validator_node_instance = self.validator_class(
                    validator_info=validator_info,
                    cardano_context=cardano_context,
                    signing_key=payment_esk,  # type: ignore
                    stake_signing_key=stake_esk,  # type: ignore
                    # Truyền thêm config vào validator nếu lớp đó yêu cầu
                )
                # --- Inject instance for API endpoints ---
                set_validator_node_instance(self.validator_node_instance)
                logger.info("Runner: Validator instance initialized and injected.")

                # --- Start Background Consensus Loop ---
                logger.info(
                    "Runner: Starting main consensus loop as background task..."
                )
                self.main_loop_task = asyncio.create_task(self._run_main_node_loop())
                logger.info("Runner: Consensus loop background task started.")

            except Exception as e:
                logger.exception(f"Runner: FATAL ERROR during validator startup: {e}")
                # Cân nhắc dừng hẳn server nếu khởi tạo lỗi
                # raise SystemExit("Failed to start validator node.") from e

        @self.app.on_event("shutdown")
        async def shutdown_event():
            logger.info(
                f"Runner: FastAPI application shutting down for {self.validator_class.__name__}..."
            )
            if self.main_loop_task and not self.main_loop_task.done():
                logger.info("Runner: Cancelling main node loop task...")
                self.main_loop_task.cancel()
                try:
                    await self.main_loop_task
                except asyncio.CancelledError:
                    logger.info("Runner: Main node loop task cancelled successfully.")
            # Đóng http client của node instance
            if self.validator_node_instance and hasattr(
                self.validator_node_instance, "http_client"
            ):
                if self.validator_node_instance.http_client:
                    try:
                        await self.validator_node_instance.http_client.aclose()
                        logger.info("Runner: Node HTTP client closed.")
                    except Exception as e_close:
                        logger.error(
                            f"Runner: Error closing node HTTP client: {e_close}"
                        )

        # Include API routes từ SDK
        self.app.include_router(api_router)

    async def _run_main_node_loop(
        self,
    ):
        """
        Chạy vòng lặp đồng thuận chính của ValidatorNode (`self.validator_node_instance`) như một background task.

        Vòng lặp này gọi phương thức `run_cycle()` của node một cách liên tục.
        Nó sẽ tiếp tục chạy cho đến khi task bị hủy (thường là khi server shutdown)
        hoặc gặp lỗi không mong muốn.

        Lưu ý: Vòng lặp này hiện tại không có độ trễ giữa các cycle. Cân nhắc
        thêm `asyncio.sleep()` nếu `run_cycle()` trả về quá nhanh và gây tốn CPU.
        """
        node = self.validator_node_instance
        if not node:
            logger.error("Runner: Validator node instance not available for main loop.")
            return

        node_settings = getattr(node, "settings", sdk_settings)

        try:
            await asyncio.sleep(
                5
            )  # Giữ lại khoảng chờ ngắn ban đầu nếu cần API sẵn sàng
            while True:
                await node.run_cycle()
                # Cân nhắc thêm sleep nếu cần:
                # await asyncio.sleep(node_settings.CYCLE_INTERVAL_SECONDS)
        except asyncio.CancelledError:
            logger.info("Runner: Main node loop cancelled.")
        except Exception as e:
            logger.exception(f"Runner: Exception in main node loop: {e}")
        finally:
            logger.info("Runner: Main node loop finished.")

    def run(self):
        """
        Khởi động API server sử dụng Uvicorn.

        Lấy thông tin host, port và log level từ `self.config` và chạy
        `uvicorn.run()` với instance FastAPI (`self.app`) đã được thiết lập.
        Đây là một blocking call, giữ cho tiến trình chạy để phục vụ API.
        """
        host = self.config.get("host", "127.0.0.1")
        port = self.config.get("port", 8001)
        log_level = self.config.get(
            "log_level", "info"
        ).lower()  # Lấy log level từ config

        logger.info(
            f"Runner: Starting Uvicorn server on {host}:{port} with log level {log_level}"
        )
        try:
            # Sử dụng app instance của class này
            uvicorn.run(
                self.app, host=host, port=port, log_level=log_level, log_config=None
            )
        except Exception as e:
            logger.exception(f"Runner: Failed to run Uvicorn: {e}")
