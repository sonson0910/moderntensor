# sdk/core/datatypes.py
"""
Định nghĩa các cấu trúc dữ liệu cốt lõi dùng chung trong SDK Moderntensor.
"""
from pycardano import PaymentVerificationKey # Import ở đây để tránh circular dependency
from typing import List, Dict, Any, Tuple, Optional
from sdk.metagraph.metagraph_datum import STATUS_ACTIVE
from dataclasses import dataclass, field
import time # Thêm import time

@dataclass
class MinerInfo:
    """Lưu trữ thông tin trạng thái của một Miner trong bộ nhớ."""
    uid: str
    address: str # Địa chỉ ví hoặc định danh mạng
    api_endpoint: Optional[str] = None # Địa chỉ API của Miner (nếu có)
    trust_score: float = 0.0
    weight: float = 0.0 # W_x - Trọng số Miner (cần được tính toán và cập nhật)
    stake: float = 0.0 # Lượng stake (có thể load từ datum)
    last_selected_time: int = -1 # Chu kỳ cuối cùng được chọn
    performance_history: List[float] = field(default_factory=list)
    # --- Thêm các trường tương ứng từ MinerDatum nếu cần cho logic ---
    status: int = STATUS_ACTIVE # <<<--- THÊM TRƯỜNG STATUS
    subnet_uid: int = 0 # UID của Subnet miner thuộc về
    registration_slot: int = 0 # Slot đăng ký
    wallet_addr_hash: Optional[bytes] = None # Hash của địa chỉ ví liên kết (nếu có)
    performance_history_hash: Optional[bytes] = None # Hash của lịch sử hiệu suất (nếu có)
    # ---------------------------------------------------------------

@dataclass
class ValidatorInfo:
    """Lưu trữ thông tin trạng thái của một Validator."""
    uid: str
    address: str # Địa chỉ ví Cardano
    api_endpoint: Optional[str] = None
    trust_score: float = 0.0
    weight: float = 0.0 # W_v
    stake: float = 0.0
    last_performance: float = 0.0 # <<<--- THÊM LẠI TRƯỜNG NÀY
    status: int = STATUS_ACTIVE # Giả định mặc định là Active
    subnet_uid: int = 0
    registration_slot: int = 0
    wallet_addr_hash: Optional[bytes] = None # Giữ bytes hoặc hex tùy chuẩn
    performance_history_hash: Optional[bytes] = None

    # --- Có thể thêm property để dễ lấy vkey ---
    @property
    def payment_verification_key(self) -> Optional['PaymentVerificationKey']:
        """Trả về đối tượng PaymentVerificationKey nếu CBOR hex tồn tại."""
        if self.payment_vkey_cbor_hex:
            import binascii
            return PaymentVerificationKey.from_cbor(binascii.unhexlify(self.payment_vkey_cbor_hex))
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

@dataclass
class ValidatorScore:
    """Lưu trữ điểm số một Validator chấm cho một Miner về một task."""
    task_id: str
    miner_uid: str
    validator_uid: str # Validator đã chấm điểm này
    score: float # Điểm P_miner,v
    deviation: Optional[float] = None # Độ lệch so với điểm đồng thuận (tính sau)
    timestamp: float = field(default_factory=time.time) # Thời điểm chấm điểm

