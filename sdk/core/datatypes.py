# sdk/core/datatypes.py
"""
Định nghĩa các cấu trúc dữ liệu cốt lõi dùng chung trong SDK Moderntensor.
"""
from sdk.compat.luxtensor_types import (
    PaymentVerificationKey,
)  # Import ở đây để tránh circular dependency
from typing import List, Dict, Any, Tuple, Optional
from dataclasses import dataclass, field
import time  # Thêm import time

# REMOVED: from sdk.metagraph.metagraph_datum import STATUS_ACTIVE (Cardano-specific)
# Define status constants locally
STATUS_ACTIVE = 1
STATUS_INACTIVE = 0

from pydantic import BaseModel, Field

# --- Import PyCardano ---
from sdk.compat.luxtensor_types import (
    VerificationKeyHash,
    PaymentVerificationKey,
)  # Thêm PaymentVerificationKey nếu property dùng


@dataclass
class MinerInfo:
    """Lưu trữ thông tin trạng thái của một Miner trong bộ nhớ."""

    uid: str
    address: str  # Địa chỉ ví hoặc định danh mạng
    api_endpoint: Optional[str] = None  # Địa chỉ API của Miner (nếu có)
    trust_score: float = 0.0
    weight: float = 0.0  # W_x - Trọng số Miner (cần được tính toán và cập nhật)
    stake: float = 0.0  # Lượng stake (có thể load từ datum)
    last_selected_time: int = -1  # Chu kỳ cuối cùng được chọn
    performance_history: List[float] = field(default_factory=list)
    # --- Thêm các trường tương ứng từ MinerDatum nếu cần cho logic ---
    status: int = STATUS_ACTIVE  # <<<--- THÊM TRƯỜNG STATUS
    subnet_uid: int = 0  # UID của Subnet miner thuộc về
    registration_slot: int = 0  # Slot đăng ký
    wallet_addr_hash: Optional[bytes] = None  # Hash của địa chỉ ví liên kết (nếu có)
    performance_history_hash: Optional[bytes] = (
        None  # Hash của lịch sử hiệu suất (nếu có)
    )
    # ---------------------------------------------------------------


@dataclass
class ValidatorInfo:
    """Lưu trữ thông tin trạng thái của một Validator."""

    uid: str
    address: str  # Địa chỉ ví Cardano
    api_endpoint: Optional[str] = None
    trust_score: float = 0.0
    weight: float = 0.0  # W_v
    stake: float = 0.0
    last_performance: float = 0.0  # <<<--- THÊM LẠI TRƯỜNG NÀY
    status: int = STATUS_ACTIVE  # Giả định mặc định là Active
    subnet_uid: int = 0
    registration_slot: int = 0
    wallet_addr_hash: Optional[bytes] = None  # Giữ bytes hoặc hex tùy chuẩn
    performance_history: List[float] = field(default_factory=list)
    performance_history_hash: Optional[bytes] = None

    # --- Có thể thêm property để dễ lấy vkey ---
    @property
    def payment_verification_key(self) -> Optional["PaymentVerificationKey"]:
        """Trả về đối tượng PaymentVerificationKey nếu CBOR hex tồn tại."""
        if self.payment_vkey_cbor_hex:  # type: ignore
            import binascii

            return PaymentVerificationKey.from_cbor(binascii.unhexlify(self.payment_vkey_cbor_hex))  # type: ignore
        return None


@dataclass
class TaskAssignment:
    """Lưu trữ thông tin về một task đã được giao cho Miner."""

    task_id: str
    task_data: Any
    miner_uid: str
    validator_uid: str
    timestamp_sent: float
    expected_result_format: Any


@dataclass
class MinerResult:
    """Lưu trữ kết quả một Miner trả về cho một task."""

    task_id: str
    miner_uid: str
    result_data: Any
    timestamp_received: float
    # --- Cải tiến: Verifiable AI ---
    proof: Optional[str] = None  # zkML proof hoặc hash bằng chứng
    signature: Optional[str] = None  # Chữ ký số của Miner xác nhận kết quả
    execution_metadata: Dict[str, Any] = field(
        default_factory=dict
    )  # Metadata: thời gian chạy, model hash...


@dataclass
class MinerCommitment:
    """Lưu trữ cam kết (hash) của Miner trước khi gửi kết quả thực tế (Commit-Reveal)."""

    task_id: str
    miner_uid: str
    commitment_hash: str  # SHA256(result_hash + salt)
    timestamp: float
    salt: Optional[str] = None  # Sẽ được cập nhật ở giai đoạn Reveal


@dataclass
class ValidatorScore:
    """Lưu trữ điểm số một Validator chấm cho một Miner về một task."""

    task_id: str
    miner_uid: str
    validator_uid: str  # Validator đã chấm điểm này
    score: float  # Điểm P_miner,v
    deviation: Optional[float] = None  # Độ lệch so với điểm đồng thuận (tính sau)
    penalty: float = 0.0  # Điểm phạt (0.0 = không phạt, 1.0 = phạt nặng/fake proof)
    timestamp: float = field(default_factory=time.time)  # Thời điểm chấm điểm


# === Dán định nghĩa ScoreSubmissionPayload vào đây ===
class ScoreSubmissionPayload(BaseModel):
    """Dữ liệu điểm số gửi qua API, bao gồm VKey và chữ ký."""

    # Sử dụng ValidatorScore từ định nghĩa ở trên
    scores: List[ValidatorScore] = Field(
        ..., description="Danh sách điểm số chi tiết ValidatorScore"
    )
    submitter_validator_uid: str = Field(
        ..., description="UID (dạng hex) của validator gửi điểm"
    )
    cycle: int = Field(..., description="Chu kỳ đồng thuận mà điểm số này thuộc về")
    timestamp: float = Field(..., description="Unix timestamp khi payload được tạo (replay protection)")
    submitter_vkey_cbor_hex: Optional[str] = Field(
        None, description="Payment Verification Key của người gửi (CBOR hex)"
    )
    signature: Optional[str] = Field(
        None,
        description="Chữ ký (dạng hex) của hash(canonical_json(scores+cycle+timestamp)) để xác thực người gửi và ngăn replay",
    )


# ====================================================


class MinerConsensusResult(BaseModel):
    """
    Dữ liệu kết quả đồng thuận cho một Miner cụ thể trong một chu kỳ,
    do Validator tính toán và công bố.
    """

    miner_uid: str = Field(..., description="UID (hex) của Miner")
    p_adj: float = Field(..., description="Điểm hiệu suất đồng thuận cuối cùng (P_adj)")
    calculated_incentive: float = Field(
        ...,
        description="Phần thưởng (incentive) đã được tính toán (dạng float, chưa scale)",
    )
    # Có thể thêm các trường tham khảo khác nếu cần
    # Ví dụ: new_trust_score_estimate: Optional[float] = Field(None, description="Ước tính trust score mới (tham khảo)")


class CycleConsensusResults(BaseModel):
    """
    Toàn bộ kết quả đồng thuận của một chu kỳ, được Validator công bố.
    """

    cycle: int = Field(..., description="Số thứ tự chu kỳ")
    results: Dict[str, MinerConsensusResult] = Field(
        ..., description="Dictionary chứa kết quả cho từng Miner (key là miner_uid hex)"
    )
    # Các trường tùy chọn cho việc xác thực/metadata
    publisher_uid: Optional[str] = Field(
        None, description="UID (hex) của validator công bố"
    )
    publish_timestamp: Optional[float] = Field(
        default_factory=time.time,
        description="Timestamp khi kết quả được công bố/cache",
    )
    # signature: Optional[str] = Field(None, description="Chữ ký của validator trên hash(results) - triển khai sau")
