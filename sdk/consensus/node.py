# sdk/consensus/node.py
"""
Định nghĩa lớp ValidatorNode chứa logic chính điều phối chu trình đồng thuận.
Sử dụng asyncio cho các tác vụ mạng và chờ đợi.
Sử dụng đối tượng settings tập trung từ sdk.config.settings.
*** Đây là khung sườn chi tiết, cần hoàn thiện logic cụ thể ***
"""
import random
import time
import math
import asyncio
import httpx
from typing import List, Dict, Any, Tuple, Optional, Set
from collections import defaultdict
import logging

# --- Import Settings ---
from sdk.config.settings import settings

# --- Import các module khác trong SDK ---
# Formulas
from sdk.formulas import * # Import tất cả hoặc import cụ thể
# Metagraph & Blockchain Interaction
from sdk.metagraph.metagraph_data import get_all_miner_data, get_all_validator_data
from sdk.metagraph import metagraph_data, update_metagraph
from sdk.metagraph.metagraph_datum import MinerDatum, ValidatorDatum, STATUS_ACTIVE, STATUS_JAILED, STATUS_INACTIVE
from sdk.smartcontract.validator import read_validator
# from sdk.metagraph.hash import hash_data, decode_history_from_hash # Cần hàm hash/decode
def hash_data(data): return f"hashed_{str(data)[:10]}" # Mock hash
async def decode_history_from_hash(hash_str): await asyncio.sleep(0); return [] # Mock decode
# Network Models (for task/result data structure)
from sdk.network.server import TaskModel, ResultModel
# Core Datatypes
from sdk.core.datatypes import (
    MinerInfo, ValidatorInfo, TaskAssignment, MinerResult, ValidatorScore
)
# Pydantic model for API communication
from sdk.network.app.api.v1.endpoints.consensus import ScoreSubmissionPayload
# PyCardano types
from pycardano import (Network, Address, ScriptHash, BlockFrostChainContext, PaymentSigningKey, StakeSigningKey, TransactionId, UTxO)

# --- Import các hàm logic đã tách ra ---
from .selection import select_miners_logic
from .scoring import score_results_logic
from .p2p import broadcast_scores_logic
from .state import (
    run_consensus_logic,
    verify_and_penalize_logic,
    prepare_miner_updates_logic,
    prepare_validator_updates_logic,
    commit_updates_logic
)

# --- Logging ---
logger = logging.getLogger(__name__)

class ValidatorNode:
    """
    Lớp điều phối chính cho Validator Node.
    Quản lý trạng thái và gọi các hàm logic từ các module con.
    """
    def __init__(self, validator_info: ValidatorInfo, cardano_context: BlockFrostChainContext, signing_key: PaymentSigningKey, stake_signing_key: Optional[StakeSigningKey] = None):
        """
        Khởi tạo Node Validator.

        Args:
            validator_info: Thông tin của validator này (UID, Address, API Endpoint).
            cardano_context: Context để tương tác Cardano (BlockFrostChainContext).
            signing_key: Khóa ký thanh toán (PaymentSigningKey) của validator.
            stake_signing_key: Khóa ký stake (StakeSigningKey) nếu có.
        """
        if not validator_info or not validator_info.uid:
             raise ValueError("Valid ValidatorInfo with a UID must be provided.")
        if not cardano_context:
             raise ValueError("Cardano context (e.g., BlockFrostChainContext) must be provided.")
        if not signing_key:
             raise ValueError("PaymentSigningKey must be provided.")

        self.info = validator_info
        self.context = cardano_context
        self.signing_key = signing_key
        self.stake_signing_key = stake_signing_key
        self.settings = settings # Sử dụng instance settings đã import

        # State variables
        self.miners_info: Dict[str, MinerInfo] = {}
        self.validators_info: Dict[str, ValidatorInfo] = {}
        self.current_utxo_map: Dict[str, UTxO] = {} # Map: uid_hex -> UTxO object
        self.current_cycle: int = 0 # TODO: Nên load từ trạng thái cuối cùng on-chain/db
        self.tasks_sent: Dict[str, TaskAssignment] = {}
        self.results_received: Dict[str, List[MinerResult]] = defaultdict(list)
        self.validator_scores: Dict[str, List[ValidatorScore]] = {} # Điểm do mình chấm

        # P2P score sharing state
        self.received_validator_scores: Dict[int, Dict[str, Dict[str, ValidatorScore]]] = defaultdict(lambda: defaultdict(dict))
        self.received_scores_lock = asyncio.Lock()

        # State for cross-cycle verification
        self.previous_cycle_results: Dict[str, Any] = {
            "final_miner_scores": {},
            "calculated_validator_states": {}
        }

        # HTTP client for P2P communication
        self.http_client = httpx.AsyncClient(
            timeout=self.settings.CONSENSUS_NETWORK_TIMEOUT_SECONDS,
            limits=httpx.Limits(max_connections=100, max_keepalive_connections=20) # Cấu hình giới hạn
        )

        # Load script details once
        try:
            validator_details = read_validator()
            if not validator_details or "script_hash" not in validator_details or "script_bytes" not in validator_details:
                raise ValueError("Failed to load valid script details (hash or bytes missing).")
            self.script_hash: ScriptHash = validator_details["script_hash"]
            self.script_bytes = validator_details["script_bytes"]
        except Exception as e:
            logger.exception("Failed to read validator script details during node initialization.")
            raise ValueError(f"Could not initialize node due to script loading error: {e}") from e

        logger.info(f"Initialized ValidatorNode {self.info.uid} using centralized settings.")
        logger.info(f"Contract Script Hash: {self.script_hash}")
        logger.info(f"Cardano Network: {self.settings.CARDANO_NETWORK}")

    # --- Tương tác Metagraph ---
    async def load_metagraph_data(self):
        """
        Tải dữ liệu miners và validators từ Metagraph bằng cách gọi các hàm
        trong sdk.metagraph.metagraph_data và cập nhật trạng thái node.
        """
        logger.info(f"[V:{self.info.uid}] Loading Metagraph data for cycle {self.current_cycle}...")
        start_time = time.time()
        network = self.settings.CARDANO_NETWORK
        datum_divisor = self.settings.METAGRAPH_DATUM_INT_DIVISOR
        max_history_len = self.settings.CONSENSUS_MAX_PERFORMANCE_HISTORY_LEN

        self.current_utxo_map = {}
        temp_miners_info = {}
        temp_validators_info = {}
        try:
            # Gọi đồng thời để tải dữ liệu
            miner_data_task = get_all_miner_data(self.context, self.script_hash, network)
            validator_data_task = get_all_validator_data(self.context, self.script_hash, network)
            # TODO: Thêm task load Subnet/Foundation data nếu cần

            all_miner_dicts, all_validator_dicts = await asyncio.gather(
                miner_data_task, validator_data_task, return_exceptions=True
            )

            # Xử lý lỗi fetch
            if isinstance(all_miner_dicts, Exception):
                logger.error(f"Failed to fetch miner data: {all_miner_dicts}")
                all_miner_dicts = []
            if isinstance(all_validator_dicts, Exception):
                logger.error(f"Failed to fetch validator data: {all_validator_dicts}")
                all_validator_dicts = []

            logger.info(f"Fetched {len(all_miner_dicts)} miner entries and {len(all_validator_dicts)} validator entries.")

            # --- Chuyển đổi Miner dicts sang MinerInfo ---
            for utxo_object, datum_dict in all_miner_dicts:
                try:
                    uid_hex = datum_dict.get("uid")
                    if not uid_hex: continue

                    # TODO: Triển khai logic load và giải mã performance_history_hash
                    # perf_history_hash_hex = datum.get("performance_history_hash")
                    # perf_history = await decode_history_from_hash(perf_history_hash_hex) if perf_history_hash_hex else []
                    perf_history = [] # Placeholder

                    temp_miners_info[uid_hex] = MinerInfo(
                        uid=uid_hex,
                        address=datum_dict.get("address", f"addr_{uid_hex[:8]}..."),
                        api_endpoint=datum_dict.get("api_endpoint"),
                        trust_score=float(datum_dict.get("trust_score", 0.0)),
                        weight=float(datum_dict.get("weight", 0.0)),
                        stake=int(datum_dict.get("stake", 0)),
                        last_selected_time=int(datum_dict.get("last_selected_cycle", -1)),
                        performance_history=perf_history[-max_history_len:],
                        subnet_uid=int(datum_dict.get("subnet_uid", -1)),
                        version=int(datum_dict.get("version", 0)),
                        status=int(datum_dict.get("status", STATUS_INACTIVE)), # Lấy status
                        registration_slot=int(datum_dict.get("registration_slot", 0)),
                        wallet_addr_hash=datum_dict.get("wallet_addr_hash"),
                        performance_history_hash=datum_dict.get("performance_history_hash"),
                    )

                    # --- Lưu UTXO vào map ---
                    self.current_utxo_map[uid_hex] = utxo_object
                except Exception as e:
                    logger.warning(f"Failed to parse Miner data dict for UID {datum_dict.get('uid', 'N/A')}: {e}", exc_info=False)
                    logger.debug(f"Problematic miner data dict: {datum_dict}")

            # --- Chuyển đổi Validator dicts sang ValidatorInfo ---
            for utxo_object, val_dict in all_validator_dicts:
                try:
                    uid_hex = datum_dict.get("uid")
                    if not uid_hex: continue

                    # --- Lấy address bytes từ datum và decode ---
                    # Lưu ý: get_all_validator_data hiện trả về hex string cho bytes
                    wallet_addr_hash_hex = datum_dict.get("wallet_addr_hash") # Lấy hex string từ dict trả về
                    validator_address_str = None
                    wallet_addr_hash_bytes = None # Biến lưu bytes gốc nếu cần
                    if wallet_addr_hash_hex:
                        try:
                            wallet_addr_hash_bytes = bytes.fromhex(wallet_addr_hash_hex)
                            validator_address_str = wallet_addr_hash_bytes.decode('utf-8')
                        except (ValueError, UnicodeDecodeError):
                            logger.warning(f"Could not decode wallet_addr_hash bytes/hex for UID {uid_hex}.")
                            validator_address_str = f"addr_undecodable_{uid_hex[:8]}..."
                    else:
                        validator_address_str = f"addr_missing_{uid_hex[:8]}..."
                    # ------------------------------------------

                    temp_validators_info[uid_hex] = ValidatorInfo(
                        uid=uid_hex,
                        address=validator_address_str,
                        api_endpoint=datum_dict.get("api_endpoint"),
                        trust_score=float(datum_dict.get("trust_score", 0.0)),
                        weight=float(datum_dict.get("weight", 0.0)),
                        stake=int(datum_dict.get("stake", 0)),
                        subnet_uid=int(datum_dict.get("subnet_uid", -1)),
                        version=int(datum_dict.get("version", 0)),
                        status=int(datum_dict.get("status", STATUS_INACTIVE)),
                        registration_slot=int(datum_dict.get("registration_slot", 0)),
                        wallet_addr_hash=wallet_addr_hash_bytes,
                        performance_history_hash=datum_dict.get("performance_history_hash"),
                    )
                    self.current_utxo_map[uid_hex] = utxo_object
                except Exception as e:
                    logger.warning(f"Failed to parse Validator data dict for UID {datum_dict.get('uid', 'N/A')}: {e}", exc_info=False)
                    logger.debug(f"Problematic validator data dict: {val_dict}")

            # --- Cập nhật trạng thái node ---
            self.miners_info = temp_miners_info
            self.validators_info = temp_validators_info

            # Cập nhật thông tin của chính mình
            self_uid_hex = self.info.uid.hex() if isinstance(self.info.uid, bytes) else self.info.uid
            if self_uid_hex in self.validators_info:
                loaded_info = self.validators_info[self_uid_hex]
                #  self.info.address = loaded_info.address
                self.info.api_endpoint = loaded_info.api_endpoint
                self.info.trust_score = loaded_info.trust_score
                self.info.weight = loaded_info.weight
                self.info.stake = loaded_info.stake
                 # Cập nhật thêm các trường khác nếu cần
                logger.info(f"Self validator info ({self_uid_hex}) updated from metagraph.")
            elif self.info.uid:
                self.validators_info[self_uid_hex] = self.info
                logger.warning(f"Self validator ({self_uid_hex}) not found in metagraph, added locally. Ensure initial state is correct.")
            else:
                logger.error("Current validator info UID is invalid after loading metagraph.")

            # TODO: Load và xử lý dữ liệu Subnet/Foundation nếu cần

            load_duration = time.time() - start_time
            logger.info(f"Processed info for {len(self.miners_info)} miners and {len(self.validators_info)} validators in {load_duration:.2f}s.")
            logger.info(f"UTXO map populated with {len(self.current_utxo_map)} entries.")

        except Exception as e:
            logger.exception(f"Critical error during metagraph data loading/processing: {e}. Cannot proceed this cycle.")
            self.current_utxo_map = {}
            self.miners_info = {}
            self.validators_info = {}
            raise RuntimeError(f"Failed to load and process metagraph data: {e}") from e

    # --- Lựa chọn Miner ---
    def select_miners(self) -> List[MinerInfo]:
        """Chọn miners để giao việc."""
        logger.info(f"[V:{self.info.uid}] Selecting miners for cycle {self.current_cycle}...")
        # Gọi hàm logic từ selection.py
        return select_miners_logic(
            miners_info=self.miners_info,
            current_cycle=self.current_cycle,
            # Các tham số khác được lấy từ self.settings bên trong hàm logic
        )

    # --- Giao Task ---
    # --- 1. Đánh dấu _create_task_data ---
    def _create_task_data(self, miner_uid: str) -> Any:
        """
        (Trừu tượng/Cần Override) Tạo dữ liệu task cụ thể cho một miner.

        Lớp Validator kế thừa cho từng Subnet PHẢI override phương thức này
        để định nghĩa nội dung và định dạng task phù hợp với bài toán AI.

        Raises:
            NotImplementedError: Nếu không được override.
        """
        logger.error(f"'_create_task_data' must be implemented by the inheriting Validator class for miner {miner_uid}.")
        raise NotImplementedError("Subnet Validator must implement task creation logic.")


    async def _send_task_via_network_async(self, miner_endpoint: str, task: TaskModel) -> bool:
        """
        Gửi task qua mạng đến miner endpoint một cách bất đồng bộ.

        Args:
            miner_endpoint: Địa chỉ API của miner (đã bao gồm http/https).
            task: Đối tượng TaskModel chứa thông tin task.

        Returns:
            True nếu gửi thành công (HTTP status 2xx), False nếu có lỗi.
        """
        if not miner_endpoint or not miner_endpoint.startswith(("http://", "https://")):
            logger.warning(f"Invalid or missing API endpoint for miner: {miner_endpoint} in task {getattr(task, 'task_id', 'N/A')}")
            return False

        # TODO: Xác định đường dẫn endpoint chính xác trên miner node để nhận task
        # Endpoint này cần được thống nhất giữa validator và miner.
        target_url = f"{miner_endpoint}/execute_task" # <<<--- GIẢ ĐỊNH ENDPOINT

        try:
            # Serialize task data thành JSON
            # Sử dụng model_dump nếu TaskModel là Pydantic v2, ngược lại dùng dict()
            task_payload = task.model_dump(mode='json') if hasattr(task, 'model_dump') else task.dict()

            logger.debug(f"Sending task {task.task_id} to {target_url}")
            # --- Gửi request POST bằng httpx ---
            response = await self.http_client.post(target_url, json=task_payload)

            # Kiểm tra HTTP status code
            response.raise_for_status() # Ném exception nếu là 4xx hoặc 5xx

            logger.info(f"Successfully sent task {task.task_id} to {miner_endpoint} (Status: {response.status_code})")
            # TODO: Có thể cần xử lý nội dung response nếu miner trả về thông tin xác nhận
            # Ví dụ: data = response.json()
            return True

        except httpx.RequestError as e:
            # Lỗi kết nối mạng, DNS, timeout,...
            logger.error(f"Network error sending task {getattr(task, 'task_id', 'N/A')} to {target_url}: {e}")
            return False
        except httpx.HTTPStatusError as e:
             # Lỗi từ phía server miner (4xx, 5xx)
             logger.error(f"HTTP error sending task {getattr(task, 'task_id', 'N/A')} to {target_url}: Status {e.response.status_code} - Response: {e.response.text[:200]}")
             return False
        except Exception as e:
            # Các lỗi khác (ví dụ: serialization,...)
            logger.exception(f"Unexpected error sending task {getattr(task, 'task_id', 'N/A')} to {target_url}: {e}")
            return False


    def send_task_and_track(self, miners: List[MinerInfo]):
        """
        Tạo và gửi task cho các miners đã chọn một cách bất đồng bộ,
        đồng thời lưu lại thông tin các task đã gửi thành công.
        """
        if not miners:
            logger.warning("send_task_and_track called with empty miner list.")
            self.tasks_sent = {}
            return

        logger.info(f"[V:{self.info.uid}] Attempting to send tasks to {len(miners)} selected miners...")
        self.tasks_sent = {} # Xóa danh sách task đã gửi của chu kỳ trước
        tasks_to_send = []
        # Tạm lưu assignment để chỉ thêm vào self.tasks_sent nếu gửi thành công
        task_assignments: Dict[str, TaskAssignment] = {} # {miner_uid: TaskAssignment}

        for miner in miners:
            # Kiểm tra xem miner có endpoint hợp lệ không
            if not miner.api_endpoint or not miner.api_endpoint.startswith(("http://", "https://")):
                logger.warning(f"Miner {miner.uid} has invalid or missing API endpoint ('{miner.api_endpoint}'). Skipping task assignment.")
                continue

            task_id = f"task_{self.current_cycle}_{self.info.uid}_{miner.uid}_{random.randint(1000,9999)}"
            try:
                task_data = self._create_task_data(miner.uid)
                # Giả sử TaskModel có thể tạo từ dict hoặc có constructor phù hợp
                # Cần đảm bảo TaskModel được import đúng
                task = TaskModel(task_id=task_id, data=task_data)
            except Exception as e:
                logger.exception(f"Failed to create task for miner {miner.uid}: {e}")
                continue # Bỏ qua miner này nếu không tạo được task

            # Tạo đối tượng TaskAssignment trước khi gửi
            assignment = TaskAssignment(
                task_id=task_id,
                task_data=task_data,
                miner_uid=miner.uid, # Lưu UID dạng hex string
                validator_uid=self.info.uid, # Lưu UID dạng hex string
                timestamp_sent=time.time(),
                expected_result_format={"output": "tensor", "loss": "float"} # Ví dụ
            )
            task_assignments[miner.uid] = assignment # Lưu tạm

            # Tạo coroutine để gửi task và thêm vào danh sách chờ
            tasks_to_send.append(self._send_task_via_network_async(miner.api_endpoint, task))

        if not tasks_to_send:
            logger.warning("No valid tasks could be prepared for sending (e.g., all selected miners lack valid endpoints).")
            return

        logger.info(f"Sending {len(tasks_to_send)} tasks concurrently...")
        # Gửi đồng thời tất cả các task
        results = asyncio.gather(*tasks_to_send, return_exceptions=True)

        successful_sends = 0
        # Xử lý kết quả gửi task
        # Lấy danh sách miners tương ứng với results (những miner thực sự được gửi task)
        miners_with_tasks = [m for m in miners if m.uid in task_assignments]
        for i, result in enumerate(results):
            # Lấy miner tương ứng với kết quả này
            if i < len(miners_with_tasks):
                miner = miners_with_tasks[i]
                assignment = task_assignments.get(miner.uid)

                if assignment and isinstance(result, bool) and result:
                    # Chỉ lưu task và cập nhật last_selected_time nếu gửi thành công
                    self.tasks_sent[assignment.task_id] = assignment
                    if miner.uid in self.miners_info: # Cập nhật trạng thái trong bộ nhớ của node
                        self.miners_info[miner.uid].last_selected_time = self.current_cycle
                        logger.debug(f"Updated last_selected_time for miner {miner.uid} to cycle {self.current_cycle}")
                    successful_sends += 1
                    # Không log thành công ở đây nữa vì đã log trong _send_task_via_network_async
                else:
                    # Ghi log lỗi nếu gửi thất bại
                    logger.warning(f"Failed to send task {assignment.task_id if assignment else 'N/A'} to Miner {miner.uid}. Error/Result: {result}")
            else:
                 # Trường hợp này không nên xảy ra nếu logic đúng
                 logger.error(f"Result index {i} out of bounds for miners_with_tasks list during task sending.")

        logger.info(f"Finished sending tasks attempt. Successful sends: {successful_sends}/{len(tasks_to_send)}.")


    # --- Nhận và Chấm điểm Kết quả ---
    # --- 2. Sửa receive_results và bỏ _listen_for_results_async ---
    async def _listen_for_results_async(self, timeout: float):
        # <<<--- Bỏ hoàn toàn logic mock này ---<<<
        # Thay vào đó, hàm receive_results sẽ chỉ đơn giản là chờ một khoảng thời gian
        # trong khi kết quả được thêm vào self.results_received thông qua API endpoint.
        pass


    async def receive_results(self, timeout: Optional[float] = None):
        """
        Chờ kết quả từ miner trong một khoảng thời gian.
        Kết quả thực tế được nhận và thêm vào self.results_received
        thông qua API endpoint '/v1/miner/submit_result'.
        """
        if timeout is None:
            receive_timeout_default = self.settings.CONSENSUS_SEND_SCORE_OFFSET_MINUTES * 60 * 0.5 # Ví dụ
            timeout = receive_timeout_default

        logger.info(f"[V:{self.info.uid}] Waiting {timeout:.1f}s for miner results via API endpoint...")

        # Đơn giản là đợi hết timeout. Kết quả sẽ được tích lũy trong self.results_received.
        await asyncio.sleep(timeout)

        # Không cần xóa self.results_received ở đây, vì nó được tích lũy qua API.
        # Có thể xóa ở đầu chu kỳ mới hoặc trước khi bắt đầu chờ.
        # => Nên xóa ở đầu hàm này để chỉ xử lý kết quả của chu kỳ hiện tại
        # Tuy nhiên, nếu API nhận kết quả chậm, có thể kết quả chu kỳ trước bị xử lý ở chu kỳ sau?
        # => Cần cơ chế quản lý kết quả theo chu kỳ trong add_miner_result.

        # Tạm thời: Giả định API đủ nhanh và chỉ xử lý kết quả đã nhận trong khoảng timeout
        async with self.results_received_lock: # Lock khi đọc số lượng
            received_count = sum(len(res_list) for res_list in self.results_received.values())
            task_ids_with_results = list(self.results_received.keys())

        logger.info(f"Finished waiting period. Total results accumulated: {received_count} for tasks: {task_ids_with_results}")
        # Logic xử lý kết quả sẽ diễn ra ở bước score_miner_results
    # -----------------------------------------------------------



    def score_miner_results(self):
        """Chấm điểm kết quả nhận được."""
        # Gọi hàm logic từ scoring.py
        self.validator_scores = score_results_logic(
            results_received=self.results_received,
            tasks_sent=self.tasks_sent,
            validator_uid=self.info.uid # Truyền UID dạng hex string
        )

    # --- 3. Thêm phương thức add_miner_result ---
    async def add_miner_result(self, result: MinerResult):
        """
        (Thread/Async Safe) Thêm một kết quả nhận được từ miner vào bộ nhớ.
        Được gọi bởi API endpoint.
        """
        # --- Validation cơ bản ---
        if not result or not result.task_id or not result.miner_uid:
            logger.warning("Attempted to add invalid miner result (missing fields).")
            return False
        # Kiểm tra xem task_id có phải là task mình đã gửi không?
        if result.task_id not in self.tasks_sent:
            logger.warning(f"Received result for unknown or unassigned task_id: {result.task_id} from miner {result.miner_uid}.")
            # Có thể bỏ qua hoặc lưu vào một khu vực riêng để phân tích
            return False # Tạm thời bỏ qua
        # Kiểm tra xem miner gửi có đúng là miner được giao task không?
        if self.tasks_sent[result.task_id].miner_uid != result.miner_uid:
             logger.warning(f"Received result for task {result.task_id} from wrong miner {result.miner_uid}. Expected {self.tasks_sent[result.task_id].miner_uid}.")
             return False # Bỏ qua
        # -----------------------

        # --- Thêm kết quả vào dict (có khóa) ---
        async with self.results_received_lock:
            # Kiểm tra xem kết quả cho task này đã nhận chưa (tránh trùng lặp?)
            # Hoặc cho phép nhiều kết quả nếu miner gửi lại? -> Tạm thời cho phép
            self.results_received[result.task_id].append(result)
            logger.info(f"Added result for task {result.task_id} from miner {result.miner_uid}.")
            return True
        # ------------------------------------
    # -----------------------------------------

    def score_miner_results(self):
        """Chấm điểm kết quả nhận được."""
        # --- Xóa điểm cũ trước khi chấm ---
        self.validator_scores = {}
        # ---------------------------------

        # --- Lock khi đọc self.results_received ---
        # Tạo bản copy để tránh giữ lock quá lâu nếu scoring chậm
        results_to_score = {}
        # Dùng asyncio.run_coroutine_threadsafe nếu gọi từ thread khác?
        # Hoặc đảm bảo score_miner_results chạy trong cùng event loop
        # Giả sử chạy trong cùng event loop, chỉ cần lock async
        # async with self.results_received_lock: # Không cần lock nếu chỉ đọc sau khi wait xong
        #     results_to_score = self.results_received.copy()
        # => Không cần lock nếu receive_results đợi xong mới gọi score

        # Lấy bản copy để xử lý
        results_to_score = self.results_received.copy()
        # Reset lại dict nhận kết quả cho chu kỳ sau
        self.results_received = defaultdict(list)


        # Gọi hàm logic từ scoring.py (truyền bản copy)
        self.validator_scores = score_results_logic(
            results_received=results_to_score, # <<<--- Dùng bản copy
            tasks_sent=self.tasks_sent,
            validator_uid=self.info.uid
        )
        # Hàm score_results_logic sẽ gọi _calculate_score_from_result (cần override)


    async def add_received_score(self, submitter_uid: str, cycle: int, scores: List[ValidatorScore]):
        """Thêm điểm số nhận được từ validator khác vào bộ nhớ (async safe)."""
        # Logic này quản lý state nội bộ nên giữ lại trong Node
        # TODO: Thêm validation cho scores và submitter_uid
        async with self.received_scores_lock:
            if cycle not in self.received_validator_scores:
                # Chỉ lưu điểm cho chu kỳ hiện tại hoặc tương lai gần? Tránh lưu trữ quá nhiều.
                if cycle < self.current_cycle - 1: # Ví dụ: chỉ giữ lại chu kỳ trước đó
                    logger.warning(f"Received scores for outdated cycle {cycle} from {submitter_uid}. Ignoring.")
                    return
                self.received_validator_scores[cycle] = defaultdict(dict)

            valid_scores_added = 0
            for score in scores:
                # Chỉ lưu điểm cho task mà node này cũng quan tâm (đã gửi đi)? Hay lưu tất cả?
                # Tạm thời lưu tất cả điểm nhận được cho chu kỳ đó.
                # if score.task_id in self.tasks_sent: # Chỉ lưu nếu task liên quan?
                if score.task_id not in self.received_validator_scores[cycle]:
                    self.received_validator_scores[cycle][score.task_id] = {}
                # Ghi đè điểm nếu validator gửi lại?
                self.received_validator_scores[cycle][score.task_id][score.validator_uid] = score
                valid_scores_added += 1
                # else:
                #     logger.debug(f"Ignoring score for irrelevant task {score.task_id} from {submitter_uid}")

            logger.debug(f"Added {valid_scores_added} scores from {submitter_uid} for cycle {cycle}")


    async def broadcast_scores(self):
        """
        Lấy danh sách validator đang hoạt động và gọi logic gửi điểm số P2P.
        """
        # Gọi hàm logic từ p2p.py
        try:
            active_validators = await self._get_active_validators()
            if active_validators:
                await broadcast_scores_logic(
                    local_scores=self.validator_scores, # Điểm mình đã chấm
                    self_validator_info=self.info,
                    signing_key=self.signing_key,
                    active_validators=active_validators,
                    current_cycle=self.current_cycle,
                    http_client=self.http_client
                )
            else:
                logger.warning("No active validators found to broadcast scores to.")
        except Exception as e:
            logger.exception(f"Error during broadcast_scores: {e}")

    async def _get_active_validators(self) -> List[ValidatorInfo]:
        """Lấy danh sách validator đang hoạt động."""
        # TODO: Triển khai logic query metagraph thực tế hoặc dùng cache.
        logger.debug("Getting active validators...")
        # Tạm thời lọc từ danh sách đã load, cần đảm bảo danh sách này được cập nhật thường xuyên
        active_vals = [
            v for v in self.validators_info.values()
            if v.api_endpoint and getattr(v, 'status', STATUS_ACTIVE) == STATUS_ACTIVE
        ]
        logger.debug(f"Found {len(active_vals)} active validators with API endpoints.")
        return active_vals

    def _has_sufficient_scores(self, task_id: str, total_active_validators: int) -> bool:
        """
        Kiểm tra xem đã nhận đủ điểm cho task cụ thể chưa để bắt đầu đồng thuận.
        """
        # Logic này quản lý state nội bộ nên giữ lại trong Node
        current_cycle_scores = self.received_validator_scores.get(self.current_cycle, {})
        task_scores = current_cycle_scores.get(task_id, {})
        received_validators_for_task = set(task_scores.keys()) # Dùng set để tránh đếm trùng

        # Đếm cả điểm của chính mình (nếu đã chấm)
        # Kiểm tra xem điểm của chính mình đã có trong validator_scores chưa và validator_uid khớp không
        if task_id in self.validator_scores:
             # Kiểm tra xem có score nào trong list của task_id này là của mình không
             if any(s.validator_uid == self.info.uid for s in self.validator_scores[task_id]):
                 received_validators_for_task.add(self.info.uid) # Thêm UID của mình vào set

        received_count = len(received_validators_for_task)

        # Tính số lượng cần thiết
        min_validators = self.settings.CONSENSUS_MIN_VALIDATORS_FOR_CONSENSUS
        # Lấy tỉ lệ phần trăm yêu cầu từ settings (thêm nếu chưa có)
        # required_percentage = self.settings.get('CONSENSUS_REQUIRED_PERCENTAGE', 0.6) # Ví dụ
        required_percentage = 0.6 # Giả định 60%

        # Yêu cầu số lượng tối thiểu HOẶC phần trăm nhất định
        required_count_by_percentage = math.ceil(total_active_validators * required_percentage)
        required_count = max(min_validators, required_count_by_percentage)

        # Đảm bảo required_count không lớn hơn tổng số validator hoạt động
        required_count = min(required_count, total_active_validators)

        logger.debug(f"Scores check for task {task_id}: Received from {received_count}/{required_count} validators (Total active: {total_active_validators}, Min: {min_validators}, %: {required_percentage*100:.0f})")
        return received_count >= required_count

    async def wait_for_consensus_scores(self, wait_timeout_seconds: float) -> bool:
        """
        Chờ nhận đủ điểm số từ các validator khác trong một khoảng thời gian giới hạn.
        """
        logger.info(f"Waiting up to {wait_timeout_seconds:.1f}s for consensus scores for cycle {self.current_cycle}...")
        start_wait = time.time()
        active_validators = await self._get_active_validators()
        total_active = len(active_validators)
        min_consensus_validators = self.settings.CONSENSUS_MIN_VALIDATORS_FOR_CONSENSUS

        if total_active == 0:
            logger.warning("No active validators found. Skipping wait for consensus scores.")
            return False # Không thể đồng thuận nếu không có ai hoạt động
        elif total_active < min_consensus_validators:
             logger.warning(f"Not enough active validators ({total_active}) for minimum consensus ({min_consensus_validators}). Proceeding with available data, but consensus might be weak.")
             # Vẫn trả về True để cho phép tính toán, nhưng log cảnh báo
             return True

        # Chỉ kiểm tra các task mà validator này đã chấm điểm (và có thể đã broadcast)
        tasks_to_check = set(self.validator_scores.keys())
        if not tasks_to_check:
             logger.info("No local scores generated, skipping wait for consensus scores.")
             return True # Không có gì để chờ

        logger.debug(f"Waiting for consensus on tasks: {list(tasks_to_check)}")
        processed_task_ids = set() # Các task đã đủ điểm

        while time.time() - start_wait < wait_timeout_seconds:
            all_relevant_tasks_sufficient = True # Kiểm tra cho các task cần check
            tasks_still_needing_check = tasks_to_check - processed_task_ids

            if not tasks_still_needing_check:
                logger.info("Sufficient scores received for all relevant tasks.")
                return True # Đã đủ hết

            async with self.received_scores_lock: # Lock khi kiểm tra
                # Tạo copy của set để tránh lỗi thay đổi kích thước khi lặp
                for task_id in list(tasks_still_needing_check):
                    if self._has_sufficient_scores(task_id, total_active):
                        logger.debug(f"Task {task_id} now has sufficient scores.")
                        processed_task_ids.add(task_id)
                    else:
                        # Chỉ cần một task chưa đủ là chưa xong
                        all_relevant_tasks_sufficient = False
                        # Vẫn tiếp tục kiểm tra các task khác trong lần lặp này

            if all_relevant_tasks_sufficient and not (tasks_to_check - processed_task_ids):
                # Điều kiện này có thể không bao giờ đạt được nếu all_relevant_tasks_sufficient=False ở trên
                # Đã kiểm tra ở đầu vòng lặp rồi.
                pass

            await asyncio.sleep(2) # Chờ 2 giây rồi kiểm tra lại

        # Nếu vòng lặp kết thúc do timeout
        remaining_tasks = tasks_to_check - processed_task_ids
        if remaining_tasks:
            logger.warning(f"Consensus score waiting timed out ({wait_timeout_seconds:.1f}s). Tasks still missing sufficient scores: {list(remaining_tasks)}")
        else:
             logger.info(f"Consensus score waiting finished within timeout ({time.time() - start_wait:.1f}s). All relevant tasks have sufficient scores.")

        # Trả về True nếu không còn task nào thiếu điểm, False nếu còn
        return not bool(remaining_tasks)


    # --- Kiểm tra và Phạt Validator (Chu kỳ trước) ---
    async def verify_and_penalize_validators(self):
        """Kiểm tra ValidatorDatum chu kỳ trước và áp dụng phạt."""
        # Gọi hàm logic từ state.py
        # Hàm này sẽ cập nhật self.validators_info trực tiếp nếu có phạt trust
        penalized_updates = await verify_and_penalize_logic(
            current_cycle=self.current_cycle,
            previous_calculated_states=self.previous_cycle_results.get("calculated_validator_states", {}),
            validators_info=self.validators_info, # Truyền trạng thái hiện tại
            context=self.context,
            settings=self.settings,
            # signing_key=self.signing_key # Có thể cần nếu commit phạt ngay
        )
        # TODO: Xử lý penalized_updates nếu cần commit ngay hoặc lưu lại
        if penalized_updates:
            logger.warning(f"Validators penalized in verification step: {list(penalized_updates.keys())}")
            # Hiện tại chỉ cập nhật trust trong self.validators_info, chưa commit

    # --- Chạy Đồng thuận và Cập nhật Trạng thái ---
    def run_consensus_and_penalties(self) -> Tuple[Dict[str, float], Dict[str, Any]]:
        """Chạy đồng thuận, tính toán trạng thái mới."""
        # Gọi hàm logic từ state.py
        return run_consensus_logic(
            current_cycle=self.current_cycle,
            tasks_sent=self.tasks_sent,
            received_scores=self.received_validator_scores.get(self.current_cycle, {}),
            validators_info=self.validators_info,
            settings=self.settings
        )

    def update_miner_state(self, final_scores: Dict[str, float]) -> Dict[str, MinerDatum]:
        """Chuẩn bị cập nhật trạng thái miners."""
        # Gọi hàm logic từ state.py
        return prepare_miner_updates_logic(
            current_cycle=self.current_cycle,
            miners_info=self.miners_info,
            final_scores=final_scores,
            settings=self.settings,
            # context=self.context # Truyền context nếu cần lấy datum cũ
        )

    async def prepare_validator_updates(self, calculated_states: Dict[str, Any]) -> Dict[str, ValidatorDatum]:
        """Chuẩn bị cập nhật trạng thái validator (chỉ cho chính mình)."""
        # Gọi hàm logic từ state.py
        return await prepare_validator_updates_logic(
            current_cycle=self.current_cycle,
            self_validator_info=self.info,
            calculated_states=calculated_states,
            settings=self.settings,
            context=self.context
        )

    async def commit_updates_to_blockchain(self, miner_updates: Dict[str, MinerDatum], validator_updates: Dict[str, ValidatorDatum]):
        """Gửi giao dịch cập nhật Datum lên blockchain (async)."""
        # Gọi hàm logic từ state.py
        await commit_updates_logic(
            miner_updates=miner_updates,
            validator_updates=validator_updates,
            context=self.context,
            signing_key=self.signing_key,
            stake_signing_key=self.stake_signing_key,
            settings=self.settings
        )


    async def run_cycle(self):
        """Thực hiện một chu kỳ đồng thuận hoàn chỉnh (async)."""
        logger.info(f"\n--- Starting Cycle {self.current_cycle} for Validator {self.info.uid} ---")
        cycle_start_time = time.time()

        # Lấy các khoảng thời gian và offset từ settings
        interval_seconds = self.settings.CONSENSUS_METAGRAPH_UPDATE_INTERVAL_MINUTES * 60
        send_offset_seconds = self.settings.CONSENSUS_SEND_SCORE_OFFSET_MINUTES * 60
        consensus_offset_seconds = self.settings.CONSENSUS_CONSENSUS_TIMEOUT_OFFSET_MINUTES * 60
        commit_offset_seconds = self.settings.CONSENSUS_COMMIT_OFFSET_SECONDS

        # Tính các thời điểm quan trọng trong chu kỳ
        metagraph_update_time = cycle_start_time + interval_seconds # Thời điểm dự kiến bắt đầu chu kỳ mới / cập nhật metagraph
        send_score_time = metagraph_update_time - send_offset_seconds # Thời điểm gửi điểm số P2P
        consensus_timeout_time = metagraph_update_time - consensus_offset_seconds # Thời điểm cuối cùng chờ điểm P2P
        commit_time = metagraph_update_time - commit_offset_seconds # Thời điểm commit lên blockchain

        # Khởi tạo các biến lưu trữ kết quả của chu kỳ
        miner_updates: Dict[str, MinerDatum] = {}
        validator_updates: Dict[str, ValidatorDatum] = {}
        penalized_validator_updates: Dict[str, ValidatorDatum] = {} # Lưu datum phạt từ bước 0

        try:
            # === BƯỚC 0: KIỂM TRA & PHẠT VALIDATOR (TỪ CHU KỲ TRƯỚC) ===
            # Mục đích: Phát hiện validator gian lận (commit sai trạng thái ở chu kỳ trước)
            # Hành động: Gọi verify_and_penalize_logic từ state.py
            # Input: current_cycle, previous_calculated_states, validators_info (hiện tại), context, settings
            # Output: Cập nhật trực tiếp trust/status trong self.validators_info nếu có phạt.
            #         Trả về penalized_validator_updates (dict datum mới cho validator bị phạt).
            penalized_validator_updates = await self.verify_and_penalize_validators() # Đã thêm return value
            logger.info(f"Step 0: Verification/Penalization check completed. Penalized datums prepared: {len(penalized_validator_updates)}")

            # === BƯỚC 1: TẢI DỮ LIỆU METAGRAPH ===
            # Mục đích: Lấy trạng thái mới nhất của miners và validators từ blockchain.
            # Hành động: Gọi self.load_metagraph_data()
            # Output: Cập nhật self.miners_info và self.validators_info (bao gồm cả self.info).
            await self.load_metagraph_data() # Sử dụng await vì hàm này là async
            if not self.miners_info:
                logger.warning("No miners found in metagraph for this cycle. Skipping task assignment.")
                # Có thể raise Exception hoặc kết thúc chu kỳ nhẹ nhàng hơn
                return # Kết thúc chu kỳ nếu không có miner

            logger.info(f"Step 1: Metagraph data loaded. Miners: {len(self.miners_info)}, Validators: {len(self.validators_info)}")

            # === BƯỚC 2: CHỌN MINERS ===
            # Mục đích: Chọn ra các miners để giao nhiệm vụ dựa trên trust và cơ chế công bằng.
            # Hành động: Gọi self.select_miners() (đồng bộ - sync) -> gọi logic trong selection.py
            # Input: self.miners_info, self.current_cycle, các tham số từ settings.
            # Output: List các MinerInfo được chọn (selected_miners).
            selected_miners = self.select_miners()
            if not selected_miners:
                logger.warning("No miners were selected for task assignment in this cycle.")
                return # Kết thúc chu kỳ nếu không chọn được miner

            logger.info(f"Step 2: Selected {len(selected_miners)} miners for tasks.")

            # === BƯỚC 3: GỬI TASK & THEO DÕI ===
            # Mục đích: Tạo và gửi nhiệm vụ đến các miners đã chọn qua mạng, lưu lại thông tin task đã gửi.
            # Hành động: Gọi self.send_task_and_track() (hiện là async để gửi đồng thời)
            # Input: selected_miners
            # Output: Cập nhật self.tasks_sent (dict: task_id -> TaskAssignment) cho các task gửi thành công.
            #         Cập nhật last_selected_time trong self.miners_info.
            # Note: Phần tạo task (_create_task_data) và gửi (_send_task_via_network_async) cần hoàn thiện logic thực tế.
            await self.send_task_and_track(selected_miners)
            logger.info(f"Step 3: Task sending attempted. Tasks tracked: {len(self.tasks_sent)}")

            # === BƯỚC 4: NHẬN KẾT QUẢ ===
            # Mục đích: Chờ đợi và nhận kết quả trả về từ các miners đã giao task.
            # Hành động: Gọi self.receive_results() (async)
            # Input: Timeout (lấy từ settings).
            # Output: Cập nhật self.results_received (dict: task_id -> List[MinerResult]).
            # Note: Phần lắng nghe (_listen_for_results_async) đang là mock, cần thay bằng logic nhận qua API thực tế.
            receive_timeout = self.settings.CONSENSUS_NETWORK_TIMEOUT_SECONDS * 3 # Ví dụ timeout
            await self.receive_results(timeout=receive_timeout)
            logger.info(f"Step 4: Result reception finished. Results received for {len(self.results_received)} tasks.")

            # === BƯỚC 5: CHẤM ĐIỂM CỤC BỘ ===
            # Mục đích: Validator tự chấm điểm các kết quả nhận được từ miners.
            # Hành động: Gọi self.score_miner_results() (sync) -> gọi logic trong scoring.py
            # Input: self.results_received, self.tasks_sent
            # Output: Cập nhật self.validator_scores (dict: task_id -> List[ValidatorScore] do chính validator này chấm).
            # Note: Logic chấm điểm cụ thể (_calculate_score_from_result) cần được override.
            self.score_miner_results()
            logger.info(f"Step 5: Local scoring completed. Scores generated for {len(self.validator_scores)} tasks.")

            # === BƯỚC 6: CHỜ & BROADCAST ĐIỂM ===
            # Mục đích: Đợi đến thời điểm thích hợp và gửi điểm số vừa chấm cho các validator khác.
            # Hành động: Tính thời gian chờ, đợi (asyncio.sleep), gọi self.broadcast_scores() (async) -> gọi logic trong p2p.py
            # Input: self.validator_scores, self.info, self.signing_key, active_validators, current_cycle, http_client.
            # Output: Gửi HTTP request chứa điểm số đã ký đến các validator khác.
            wait_before_send = send_score_time - time.time()
            if wait_before_send > 0:
                logger.info(f"Step 6a: Waiting {wait_before_send:.1f}s before broadcasting scores...")
                await asyncio.sleep(wait_before_send)
            else:
                logger.warning(f"Step 6a: Send score time already passed by {-wait_before_send:.1f}s!")
            logger.info(f"Step 6b: Broadcasting local scores...")
            await self.broadcast_scores()

            # === BƯỚC 7: CHỜ NHẬN ĐIỂM P2P ===
            # Mục đích: Chờ nhận đủ điểm số từ các validator khác để bắt đầu tính toán đồng thuận.
            # Hành động: Tính thời gian chờ, gọi self.wait_for_consensus_scores() (async)
            # Input: Timeout (dựa trên consensus_timeout_time).
            # Output: boolean (consensus_possible) cho biết có đủ điểm hay không.
            #         self.received_validator_scores được cập nhật ngầm bởi API endpoint nhận điểm.
            wait_for_scores_timeout = consensus_timeout_time - time.time()
            consensus_possible = False
            if wait_for_scores_timeout > 0:
                logger.info(f"Step 7: Waiting up to {wait_for_scores_timeout:.1f}s for P2P scores...")
                consensus_possible = await self.wait_for_consensus_scores(wait_for_scores_timeout)
                logger.info(f"Step 7: Consensus possible based on received scores: {consensus_possible}")
            else:
                logger.warning(f"Step 7: Not enough time left ({wait_for_scores_timeout:.1f}s) to wait for consensus scores.")

            # === BƯỚC 8: CHẠY ĐỒNG THUẬN & TÍNH TOÁN TRẠNG THÁI ===
            # Mục đích: Tính điểm đồng thuận cuối cùng cho miners (P_adj) và trạng thái dự kiến (E_v, trust, reward) cho validators.
            # Hành động: Gọi self.run_consensus_and_penalties() (sync) -> gọi run_consensus_logic từ state.py
            # Input: current_cycle, tasks_sent, received_scores (P2P + local), validators_info, settings.
            # Output: final_miner_scores (dict), calculated_validator_states (dict trạng thái dự kiến).
            #         Cập nhật self.previous_cycle_results["calculated_validator_states"] cho chu kỳ sau.
            logger.info(f"Step 8: Running consensus calculations...")
            final_miner_scores, calculated_validator_states = self.run_consensus_and_penalties()
            if not consensus_possible:
                logger.warning("Consensus calculation was performed with potentially incomplete P2P score set!")
            # Lưu trạng thái dự kiến cho việc kiểm tra ở chu kỳ tiếp theo
            self.previous_cycle_results["calculated_validator_states"] = calculated_validator_states.copy() # Lưu bản copy
            self.previous_cycle_results["final_miner_scores"] = final_miner_scores.copy() # Lưu điểm miner nếu cần
            logger.info(f"Step 8: Consensus calculation finished. Final miner scores: {len(final_miner_scores)}, Validator states calculated: {len(calculated_validator_states)}")

            # === BƯỚC 9: CHUẨN BỊ CẬP NHẬT MINER DATUM ===
            # Mục đích: Tạo các đối tượng MinerDatum mới dựa trên kết quả đồng thuận.
            # Hành động: Gọi self.update_miner_state() (sync) -> gọi prepare_miner_updates_logic từ state.py
            # Input: current_cycle, miners_info (có thể đã bị cập nhật trust ở bước 0), final_scores, settings.
            # Output: miner_updates (dict: uid_hex -> MinerDatum mới).
            logger.info(f"Step 9: Preparing miner datum updates...")
            miner_updates = self.update_miner_state(final_miner_scores)
            logger.info(f"Step 9: Prepared {len(miner_updates)} miner datums.")

            # === BƯỚC 10: CHUẨN BỊ CẬP NHẬT VALIDATOR DATUM (CHO CHÍNH MÌNH) ===
            # Mục đích: Tạo đối tượng ValidatorDatum mới cho chính validator này.
            # Hành động: Gọi self.prepare_validator_updates() (async) -> gọi prepare_validator_updates_logic từ state.py
            # Input: current_cycle, self.info, calculated_validator_states, settings, context.
            # Output: validator_updates (dict: self_uid_hex -> ValidatorDatum mới).
            logger.info(f"Step 10: Preparing self validator datum update...")
            validator_updates = await self.prepare_validator_updates(calculated_validator_states)
            logger.info(f"Step 10: Prepared {len(validator_updates)} self validator datums.")

            # === BƯỚC 11: CHỜ COMMIT ===
            # Mục đích: Đợi đến thời điểm commit đã định sẵn.
            # Hành động: Tính thời gian chờ, đợi (asyncio.sleep).
            wait_before_commit = commit_time - time.time()
            if wait_before_commit > 0:
                logger.info(f"Step 11: Waiting {wait_before_commit:.1f}s before committing updates...")
                await asyncio.sleep(wait_before_commit)
            else:
                logger.warning(f"Step 11: Commit time already passed by {-wait_before_commit:.1f}s! Committing immediately.")

            # === BƯỚC 12: COMMIT LÊN BLOCKCHAIN ===
            # Mục đích: Gửi giao dịch cập nhật các MinerDatum và ValidatorDatum lên blockchain.
            # Hành động: Gọi self.commit_updates_to_blockchain() (async) -> gọi commit_updates_logic từ state.py
            # Input: miner_updates, validator_updates (self), penalized_validator_updates (từ Bước 0), context, keys, settings, script_hash/bytes, network.
            # Output: Gửi giao dịch lên mạng Cardano.
            # Note: Logic commit thực tế cần được hoàn thiện.
            logger.info(f"Step 12: Committing updates to blockchain...")
            await self.commit_updates_to_blockchain(
                miner_updates=miner_updates,
                validator_updates=validator_updates,
                penalized_validator_updates=penalized_validator_updates, # Truyền datum phạt vào đây
                context=self.context,
                signing_key=self.signing_key,
                stake_signing_key=self.stake_signing_key,
                settings=self.settings,
                script_hash=self.script_hash,
                script_bytes=self.script_bytes,
                network=self.settings.CARDANO_NETWORK # Truyền network từ settings
            )
            logger.info(f"Step 12: Commit process initiated.")

        except Exception as e:
            logger.exception(f"Error during consensus cycle {self.current_cycle}: {e}")
            # Cân nhắc có nên dừng hẳn node hay chỉ bỏ qua chu kỳ lỗi

        finally:
            # === DỌN DẸP CUỐI CHU KỲ ===
            cycle_end_time = time.time()
            logger.info(f"--- Cycle {self.current_cycle} Finished (Duration: {cycle_end_time - cycle_start_time:.1f}s) ---")
            # Tăng số thứ tự chu kỳ
            self.current_cycle += 1
            # Dọn dẹp dữ liệu P2P của các chu kỳ quá cũ để tiết kiệm bộ nhớ
            cleanup_cycle = self.current_cycle - 3 # Ví dụ: Giữ lại dữ liệu 2 chu kỳ trước đó
            async with self.received_scores_lock:
                if cleanup_cycle in self.received_validator_scores:
                    try:
                        del self.received_validator_scores[cleanup_cycle]
                        logger.info(f"Cleaned up received scores for cycle {cleanup_cycle}")
                    except KeyError:
                        pass # Đã bị xóa bởi luồng khác?


# --- Hàm chạy chính (Ví dụ Async) ---
async def main_validator_loop():
    logger.info("Starting validator node loop...")
    if not settings: logger.error("Settings not loaded. Exiting."); return

    # --- Khởi tạo context Cardano ---
    # cardano_ctx = await cardano_service.get_context_async(settings)

    # --- Load thông tin validator từ settings ---
    validator_uid = settings.VALIDATOR_UID
    validator_address = settings.VALIDATOR_ADDRESS
    api_endpoint = settings.VALIDATOR_API_ENDPOINT
    if not validator_uid or not validator_address or not api_endpoint:
        logger.error("Validator UID, Address, or API Endpoint not configured. Exiting.")
        return

    if not ValidatorInfo or not ValidatorNode: logger.error("Node classes not available. Exiting."); return

    # TODO: Load signing key thực tế từ file/env được bảo vệ
    signing_key: Optional[PaymentSigningKey] = None
    stake_signing_key: Optional[StakeSigningKey] = None
    # Ví dụ load từ file (cần triển khai hàm load_skey)
    # try:
    #     signing_key = load_skey(settings.PAYMENT_SKEY_PATH)
    #     if settings.STAKE_SKEY_PATH:
    #          stake_signing_key = load_skey(settings.STAKE_SKEY_PATH)
    # except Exception as e:
    #     logger.exception(f"Failed to load signing keys: {e}")
    #     return
    if signing_key is None:
         logger.error("Payment signing key could not be loaded. Exiting.")
         return


    my_validator_info = ValidatorInfo(uid=validator_uid, address=validator_address, api_endpoint=api_endpoint)
    # cardano_ctx = ... # Context thực tế

    # Tạo node validator
    validator_node = ValidatorNode(
        validator_info=my_validator_info,
        cardano_context=None, # cardano_ctx
        signing_key=signing_key,
        stake_signing_key=stake_signing_key
    )

    # --- Inject instance vào dependency của FastAPI ---
    try:
        from sdk.network.app.dependencies import set_validator_node_instance
        set_validator_node_instance(validator_node)
        logger.info("Validator node instance injected into API dependency.")
    except Exception as e:
        logger.error(f"Could not inject validator node into API dependency: {e}")

    # --- Chạy vòng lặp chính ---
    try:
        while True:
            cycle_start_time = time.time()
            await validator_node.run_cycle()
            cycle_duration = time.time() - cycle_start_time
            cycle_interval_seconds = settings.CONSENSUS_METAGRAPH_UPDATE_INTERVAL_MINUTES * 60
            min_wait = settings.CONSENSUS_CYCLE_MIN_WAIT_SECONDS
            wait_time = max(min_wait, cycle_interval_seconds - cycle_duration)
            logger.info(f"Cycle duration: {cycle_duration:.1f}s. Waiting {wait_time:.1f}s for next cycle...")
            await asyncio.sleep(wait_time)
    except asyncio.CancelledError:
        logger.info("Main node loop cancelled.")
    except Exception as e:
        logger.exception(f"Exception in main node loop: {e}")
    finally:
        await validator_node.http_client.aclose()
        logger.info("Main node loop finished.")

if __name__ == "__main__":
    try:
        if settings: asyncio.run(main_validator_loop())
        else: print("Could not load settings. Aborting.")
    except KeyboardInterrupt:
        print("\nInterrupted by user.")

