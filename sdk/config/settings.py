# sdk/config/settings.py

import logging
import math
import os
from typing import Optional

from pydantic import Field, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict
from pycardano import Network


class Settings(BaseSettings):
    """
    Quản lý cấu hình tập trung cho dự án, load từ biến môi trường hoặc file .env.
    Đã cập nhật để bao gồm các tham số và hằng số cho module đồng thuận.
    """

    model_config = SettingsConfigDict(
        extra="ignore",
        env_file=".env",
        env_file_encoding="utf-8",
        env_prefix='MODERNTENSOR_' # Giữ nguyên tiền tố (hoặc bỏ nếu không muốn)
    )

    # --- Các trường cấu hình gốc của bạn ---
    BLOCKFROST_PROJECT_ID: str = Field(default="preprod06dzhzKlynuTInzvxHDH5cXbdHo524DE", alias="BLOCKFROST_PROJECT_ID")
    HOTKEY_BASE_DIR: str = Field(default="moderntensor", alias="HOTKEY_BASE_DIR")
    COLDKEY_NAME: str = Field(default="kickoff", alias="COLDKEY_NAME")
    HOTKEY_NAME: str = Field(default="hk1", alias="HOTKEY_NAME")
    HOTKEY_PASSWORD: str = Field(default="sonlearn2003", alias="HOTKEY_PASSWORD")
    TEST_RECEIVER_ADDRESS: str = Field(default="addr_test1qpkxr3kpzex93m646qr7w82d56md2kchtsv9jy39dykn4cmcxuuneyeqhdc4wy7de9mk54fndmckahxwqtwy3qg8pums5vlxhz", alias="TEST_RECEIVER_ADDRESS")
    TEST_POLICY_ID_HEX: str = Field(default="b9107b627e28700da1c5c2077c40b1c7d1fe2e9b23ff20e0e6b8fec1", alias="TEST_POLICY_ID_HEX")
    TEST_CONTRACT_ADDRESS: str = Field(default="addr_test1wqlerxnfzfcgx72zpuepgchl6p5mnjsxm27cwjxqq9wuthch489d5", alias="TEST_CONTRACT_ADDRESS")
    CARDANO_NETWORK: str = Field(default="TESTNET", alias="CARDANO_NETWORK")

    # --- Thêm các trường cấu hình Node (Ví dụ) ---
    API_PORT: int = Field(8000, alias="API_PORT", description="Port cho FastAPI server")
    VALIDATOR_UID: Optional[str] = Field(None, alias="VALIDATOR_UID", description="UID của validator này (nếu chạy node)")
    VALIDATOR_ADDRESS: Optional[str] = Field(None, alias="VALIDATOR_ADDRESS", description="Địa chỉ Cardano của validator này")
    VALIDATOR_API_ENDPOINT: Optional[str] = Field(None, alias="VALIDATOR_API_ENDPOINT", description="Địa chỉ API đầy đủ mà các node khác có thể gọi đến validator này")

    # --- Cấu hình Đồng thuận (Consensus) - Tham số & Hằng số ---

    # --- Cấu hình Đồng thuận (Consensus) - Tham số & Hằng số ---

    # --- Timing ---
    CONSENSUS_METAGRAPH_UPDATE_INTERVAL_MINUTES: int = Field(60, description="Khoảng thời gian (phút) giữa các lần cập nhật metagraph dự kiến.")
    CONSENSUS_SEND_SCORE_OFFSET_MINUTES: int = Field(2, description="Gửi điểm số P2P trước thời điểm cập nhật metagraph bao nhiêu phút.")
    CONSENSUS_CONSENSUS_TIMEOUT_OFFSET_MINUTES: int = Field(1, description="Chờ điểm P2P đến trước thời điểm cập nhật metagraph bao nhiêu phút (phải nhỏ hơn SEND_SCORE_OFFSET).")
    CONSENSUS_COMMIT_OFFSET_SECONDS: int = Field(15, description="Commit lên blockchain trước thời điểm cập nhật metagraph bao nhiêu giây (phải nhỏ hơn CONSENSUS_TIMEOUT_OFFSET).")
    CONSENSUS_COMMIT_DELAY_SECONDS: float = Field(1.5, description="Delay (giây) giữa các lần submit giao dịch commit để tránh rate limit.")
    CONSENSUS_CYCLE_MIN_WAIT_SECONDS: int = Field(10, description="Thời gian chờ tối thiểu (giây) giữa các chu kỳ, nếu chu kỳ hoàn thành quá nhanh.")
    CONSENSUS_NETWORK_TIMEOUT_SECONDS: int = Field(10, description="Timeout (giây) cho các yêu cầu mạng P2P (gửi task, gửi điểm).")

    # --- Limits & Constants ---
    CONSENSUS_MAX_PERFORMANCE_HISTORY_LEN: int = Field(100, description="Độ dài tối đa của lịch sử hiệu suất lưu trữ (ảnh hưởng tính weight).")
    METAGRAPH_DATUM_INT_DIVISOR: float = Field(1_000_000.0, description="Hệ số scale khi lưu float (performance, trust) thành int trong Datum.")
    CONSENSUS_MIN_VALIDATORS_FOR_CONSENSUS: int = Field(3, description="Số validator tối thiểu cần gửi điểm để thực hiện tính toán đồng thuận.")
    # CONSENSUS_REQUIRED_PERCENTAGE: float = Field(0.6, description="Tỷ lệ % validator tối thiểu cần gửi điểm (ngoài số lượng tối thiểu).") # Có thể thêm nếu muốn

    # --- Miner Selection ---
    CONSENSUS_NUM_MINERS_TO_SELECT: int = Field(5, description="Số lượng miner mỗi validator chọn trong một chu kỳ.")
    CONSENSUS_PARAM_BETA: float = Field(0.2, ge=0.0, description="Hệ số bonus công bằng khi chọn miner (beta >= 0). Càng lớn càng ưu tiên miner chờ lâu.")
    CONSENSUS_PARAM_MAX_TIME_BONUS: int = Field(10, ge=0, description="Số chu kỳ tối đa mà bonus thời gian chờ (beta) có tác dụng.")

    # --- Trust Score Update ---
    CONSENSUS_PARAM_DELTA_TRUST: float = Field(0.01, ge=0.0, lt=1.0, description="Hằng số suy giảm trust score mỗi chu kỳ (0 <= delta < 1). Càng lớn suy giảm càng nhanh.")
    CONSENSUS_PARAM_ALPHA_BASE: float = Field(0.1, gt=0.0, le=1.0, description="Learning rate cơ bản khi cập nhật trust score (0 < alpha <= 1).")
    CONSENSUS_PARAM_K_ALPHA: float = Field(1.0, ge=0.0, le=2.0, description="Hệ số điều chỉnh learning rate (0 <= k_alpha <= 2). k=0: alpha không đổi; k=2: alpha=0 tại trust=0 và 1.")
    CONSENSUS_PARAM_UPDATE_SIG_L: float = Field(1.0, gt=0.0, description="Giá trị tối đa (L > 0) của hàm sigmoid f_update_sig (ánh xạ điểm perf/E_v mới khi cập nhật trust).")
    CONSENSUS_PARAM_UPDATE_SIG_K: float = Field(10.0, gt=0.0, description="Độ dốc (k > 0) của hàm sigmoid f_update_sig.")
    CONSENSUS_PARAM_UPDATE_SIG_X0: float = Field(0.5, description="Điểm uốn (x0) của hàm sigmoid f_update_sig.")

    # --- Incentive Calculation ---
    CONSENSUS_PARAM_INCENTIVE_SIG_L: float = Field(1.0, gt=0.0, description="Giá trị tối đa (L > 0) của hàm sigmoid f_sig (ánh xạ trust score khi tính incentive).")
    CONSENSUS_PARAM_INCENTIVE_SIG_K: float = Field(10.0, gt=0.0, description="Độ dốc (k > 0) của hàm sigmoid f_sig.")
    CONSENSUS_PARAM_INCENTIVE_SIG_X0: float = Field(0.5, description="Điểm uốn (x0) của hàm sigmoid f_sig.")

    # --- Fraud Detection & Penalty (Validator) ---
    CONSENSUS_DATUM_COMPARISON_TOLERANCE: float = Field(1e-5, gt=0.0, description="Sai số cho phép (float) khi so sánh datum on-chain và dự kiến.")
    # CONSENSUS_PARAM_FRAUD_N_CYCLES: int = Field(3, ge=1, description="Số chu kỳ liên tiếp sai lệch để gắn cờ gian lận (cần logic theo dõi).") # Chưa dùng đến
    CONSENSUS_PARAM_PENALTY_ETA: float = Field(0.1, ge=0.0, le=1.0, description="Hệ số phạt trust score validator khi gian lận (0 <= eta <= 1). Giá trị = eta * severity.")
    CONSENSUS_JAILED_SEVERITY_THRESHOLD: float = Field(0.2, ge=0.0, le=1.0, description="Ngưỡng severity tối thiểu để chuyển validator sang JAILED (0 <= threshold <= 1).")
    CONSENSUS_PARAM_MAX_SLASH_RATE: float = Field(0.05, ge=0.0, le=1.0, description="Tỷ lệ cắt stake tối đa khi gian lận (0 <= rate <= 1). Ví dụ: 0.05 = 5% stake.")

    # --- Weight Calculation ---
    CONSENSUS_PARAM_DELTA_W: float = Field(0.05, ge=0.0, lt=1.0, description="Hằng số suy giảm trọng số miner W_x theo thời gian (0 <= delta < 1).")
    CONSENSUS_PARAM_LAMBDA_BALANCE: float = Field(0.5, ge=0.0, le=1.0, description="Hệ số cân bằng stake/performance cho trọng số validator W_v (0 <= lambda <= 1). lambda=1: chỉ stake; lambda=0: chỉ perf/time.")
    CONSENSUS_PARAM_STAKE_LOG_BASE: float = Field(math.e, gt=1.0, description=f"Cơ số log cho stake trong W_v (> 1). Dùng {math.e:.2f} cho ln, 10 cho log10.")
    CONSENSUS_PARAM_TIME_LOG_BASE: float = Field(10, gt=1.0, description="Cơ số log cho thời gian tham gia trong W_v (> 1).")

    # --- Validator Performance (E_v) ---
    CONSENSUS_PARAM_THETA1: float = Field(0.1, ge=0.0, description="Trọng số theta1 cho E_v (Q_task - hiện không dùng).")
    CONSENSUS_PARAM_THETA2: float = Field(0.6, ge=0.0, description="Trọng số theta2 cho E_v (Metric Quality - ví dụ: độ ổn định).")
    CONSENSUS_PARAM_THETA3: float = Field(0.3, ge=0.0, description="Trọng số theta3 cho E_v (Penalty Term - phạt do sai lệch). Tổng theta nên = 1.")
    CONSENSUS_PARAM_PENALTY_THRESHOLD_DEV: float = Field(0.05, ge=0.0, description="Ngưỡng độ lệch trung bình bắt đầu phạt trong E_v (Threshold_dev >= 0).")
    CONSENSUS_PARAM_PENALTY_K_PENALTY: float = Field(10.0, ge=0.0, description="Hệ số phạt độ lệch trong E_v (k' >= 0).")
    CONSENSUS_PARAM_PENALTY_P_PENALTY: float = Field(1.0, ge=1.0, description="Bậc phạt độ lệch trong E_v (p >= 1). p=1: tuyến tính; p=2: bậc hai.")

    # --- DAO ---
    CONSENSUS_PARAM_DAO_KG: float = Field(1.0, ge=0.0, description="Hệ số bonus thời gian (sqrt) cho voting power DAO (k_g >= 0).")
    CONSENSUS_PARAM_DAO_TOTAL_TIME: float = Field(365.0 * 24 * 60 * 60, gt=0.0, description="Khoảng thời gian tham chiếu (giây) cho bonus thời gian DAO (ví dụ: 1 năm).") # Đổi sang giây

    # Giữ nguyên validator của bạn
    @field_validator("CARDANO_NETWORK", mode="before")
    def validate_network(cls, value: Optional[str]): # Cho phép Optional để xử lý default tốt hơn
        if value is None: value = "TESTNET" # Gán default nếu là None
        normalized = str(value).upper().strip()
        if normalized == "MAINNET": return Network.MAINNET
        # Mặc định là TESTNET cho các giá trị khác hoặc không hợp lệ
        return Network.TESTNET

# --- Tạo một instance để sử dụng trong toàn bộ ứng dụng ---
try:
    settings = Settings()
    # --- Kiểm tra tổng Theta ---
    if not math.isclose(settings.CONSENSUS_PARAM_THETA1 + settings.CONSENSUS_PARAM_THETA2 + settings.CONSENSUS_PARAM_THETA3, 1.0, abs_tol=1e-9):
         logging.warning(f"Sum of Theta parameters (theta1+theta2+theta3 = {settings.CONSENSUS_PARAM_THETA1 + settings.CONSENSUS_PARAM_THETA2 + settings.CONSENSUS_PARAM_THETA3}) is not equal to 1.0!")
    # ------------------------
except Exception as e:
    print(f"CRITICAL: Error loading settings: {e}. Using default values where possible.")
    # Trong trường hợp lỗi nghiêm trọng, có thể nên thoát thay vì dùng default
    # raise SystemExit(f"Failed to load critical settings: {e}")
    settings = Settings() # Cố gắng tạo với default

# --- LOGGING CONFIGURATION ---
# Giữ nguyên cấu hình logging của bạn, đảm bảo nó dùng settings.LOG_LEVEL
try:
    log_level_str = settings.log_level.upper()
    if log_level_str not in ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]:
        log_level_str = "INFO"
    LOG_LEVEL = getattr(logging, log_level_str)
except Exception:
    LOG_LEVEL = logging.INFO

logging.basicConfig(
    level=LOG_LEVEL,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    handlers=[ logging.StreamHandler() ] # Giữ nguyên handlers của bạn
)
logger = logging.getLogger(__name__)
logger.info(f"Settings loaded. Log level set to {logging.getLevelName(LOG_LEVEL)}.")

