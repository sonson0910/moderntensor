# File: sdk/agent/miner_agent.py
# Logic cho Miner để tự động lấy kết quả đồng thuận và cập nhật trạng thái on-chain.

import asyncio
import logging
import time
import httpx
import binascii  # <<<--- Import binascii
import json  # <<<--- Thêm import json
import os  # <<<--- Thêm import os
from pathlib import Path  # <<<--- Thêm Path
from typing import Optional, Dict, Tuple, List, Any

# Import từ SDK
from sdk.config.settings import Settings
from sdk.core.datatypes import MinerConsensusResult, CycleConsensusResults

# Import lớp Datum và các công thức cần thiết
from sdk.metagraph.metagraph_datum import MinerDatum  # <<<--- Chỉ cần MinerDatum
from sdk.formulas import update_trust_score  # Import hàm cập nhật trust
from sdk.metagraph.hash.hash_datum import hash_data
from sdk.metagraph.hash.verify_hash import verify_hash

# Import các tiện ích tương tác Cardano
from sdk.keymanager.decryption_utils import decode_hotkey_skey
from sdk.service.context import get_chain_context
from sdk.service.utxos import get_utxo_from_str  # Hàm tìm UTXO theo UID
from sdk.smartcontract.validator import read_validator  # Để lấy script hash/bytes

# Import các kiểu PyCardano
from pycardano import (
    BlockFrostChainContext,
    Network,
    ExtendedSigningKey,
    Address,
    UTxO,
    PlutusData,
    Redeemer,
    ScriptHash,
    TransactionBuilder,
    TransactionOutput,
    Value,
    TransactionId,
    PaymentSigningKey,
    StakeSigningKey,
    PlutusV3Script,
    SigningKey,  # Import SigningKey nếu dùng trong build_and_sign
)

# Lấy logger đã cấu hình
logger = logging.getLogger(__name__)

# --- Giả định Redeemer Tag cho Miner Self-Update ---
# Cần khớp với định nghĩa trong Plutus Script
MINER_SELF_UPDATE_REDEEMER_TAG = 1  # Ví dụ: Tag 1 cho hành động miner tự cập nhật
# ---------------------------------------------------


from sdk.subnets.protocol import SubnetProtocol
from sdk.utils.zkml import ZkmlManager
from fastapi import FastAPI, BackgroundTasks
import uvicorn
from sdk.network.server import TaskModel, ResultModel


class MinerAgent:
    """
    Agent chạy song song với Miner Server, chịu trách nhiệm:
    1. Fetch kết quả đồng thuận từ Validator API.
    2. Tìm UTXO Datum của chính mình trên blockchain.
    3. Tính toán trạng thái Datum mới (trust, reward, performance...).
    4. Gửi giao dịch self-update lên Cardano để cập nhật Datum.
    5. Xử lý Task từ Validator (sử dụng SubnetProtocol).
    """

    def __init__(
        self,
        miner_uid_hex: str,  # <<<--- Nhận UID hex dạng string từ cấu hình
        config: Settings,
        miner_skey: ExtendedSigningKey,  # Khóa ký payment chính
        miner_stake_skey: Optional[ExtendedSigningKey] = None,  # Khóa ký stake (nếu có)
        subnet_protocol: Optional[SubnetProtocol] = None,  # Inject Subnet Protocol
    ):
        """
        Khởi tạo Miner Agent.

        Args:
            miner_uid_hex: UID hex *on-chain* của miner này (dạng string).
            config: Đối tượng Settings chứa các tham số cấu hình.
            miner_skey: Khóa ký payment (ExtendedSigningKey) của ví miner.
            miner_stake_skey: Khóa ký stake (ExtendedSigningKey) của ví miner (tùy chọn).
            subnet_protocol: Giao thức Subnet để xử lý task.

        Raises:
            ValueError: Nếu miner_uid_hex không hợp lệ.
            RuntimeError: Nếu không thể khởi tạo context Cardano hoặc load script.
        """
        if not miner_uid_hex or not isinstance(miner_uid_hex, str):
            raise ValueError("MinerAgent requires a valid miner_uid_hex (string).")

        self.miner_uid_hex: str = miner_uid_hex
        self.config = config
        self.miner_skey = miner_skey
        self.miner_stake_skey = miner_stake_skey
        self.subnet_protocol = subnet_protocol

        # Initialize zkML Manager
        self.zkml_manager = ZkmlManager(
            settings_path="zkml_settings.json"
        )  # Path should be configurable
        self.subnet_protocol = subnet_protocol
        self.zkml_manager = ZkmlManager(
            model_path="model.onnx", settings_path="settings.json"
        )  # Initialize zkML

        # --- Initialize FastAPI ---
        self.app = FastAPI()
        self.setup_routes()
        # --------------------------

        try:
            # Chuyển đổi và lưu trữ UID dạng bytes
            self.miner_uid_bytes: bytes = binascii.unhexlify(miner_uid_hex)
        except (binascii.Error, TypeError) as e:
            raise ValueError(
                f"Invalid miner_uid_hex format: {miner_uid_hex}. Error: {e}"
            ) from e

        self.config = config
        self.signing_key = miner_skey
        self.stake_signing_key = miner_stake_skey
        uid_prefix = f"[Init:{self.miner_uid_hex}]"

        # --- Khởi tạo Context và Network ---
        logger.debug(f"{uid_prefix} Initializing Cardano context...")
        try:
            self.context = get_chain_context(method="blockfrost")
            if not self.context:
                raise RuntimeError("Failed to initialize Cardano context.")
            self.network = self.context.network
            logger.debug(
                f"{uid_prefix} Cardano context initialized (Network: {self.network})."
            )
        except Exception as e:
            logger.exception(f"{uid_prefix} Failed to initialize Cardano context.")
            raise RuntimeError(f"Failed to initialize Cardano context: {e}") from e
        # ------------------------------------

        # HTTP client
        logger.debug(f"{uid_prefix} Initializing HTTP client...")
        self.http_client = httpx.AsyncClient(timeout=15.0)

        # --- Load Script Details ---
        logger.debug(f"{uid_prefix} Loading validator script details...")
        try:
            validator_details = read_validator()
            if (
                not validator_details
                or "script_hash" not in validator_details
                or "script_bytes" not in validator_details
            ):
                raise ValueError("Invalid script details loaded.")
            self.script_hash: ScriptHash = validator_details["script_hash"]
            self.script_bytes: PlutusV3Script = validator_details["script_bytes"]
            self.contract_address = Address(
                payment_part=self.script_hash, network=self.network
            )
            logger.debug(
                f"{uid_prefix} Script details loaded (Contract Address: {self.contract_address})."
            )
        except Exception as e:
            logger.exception(f"{uid_prefix} Failed to load script details.")
            raise RuntimeError(f"Failed to load script details: {e}") from e
        # ---------------------------

        # --- Tạo Địa chỉ Ví Miner ---
        logger.debug(f"{uid_prefix} Deriving miner wallet address...")
        try:
            pay_vkey = self.signing_key.to_verification_key()
            stake_vkey = (
                self.stake_signing_key.to_verification_key()
                if self.stake_signing_key
                else None
            )
            self.miner_wallet_address = Address(
                payment_part=pay_vkey.hash(),
                staking_part=stake_vkey.hash() if stake_vkey else None,
                network=self.network,
            )
            logger.debug(
                f"{uid_prefix} Miner wallet address derived: {self.miner_wallet_address}"
            )
        except Exception as e:
            logger.exception(f"{uid_prefix} Failed to derive miner wallet address.")
            raise RuntimeError(f"Failed to derive miner wallet address: {e}") from e
        # ---------------------------

        # State theo dõi
        self.last_processed_cycle = -1
        self.last_known_utxo: Optional[UTxO] = None

        self.state_dir = Path(
            getattr(self.config, "AGENT_STATE_DIR", ".miner_agent_state")
        )
        self.state_dir.mkdir(parents=True, exist_ok=True)
        self.history_file = self.state_dir / f"history_{self.miner_uid_hex}.json"
        logger.debug(f"{uid_prefix} State directory: {self.state_dir}")
        logger.debug(f"{uid_prefix} History file: {self.history_file}")

        # Final init log
        logger.info(f"{uid_prefix} MinerAgent initialized.")
        logger.info(f"{uid_prefix} Wallet Address: {self.miner_wallet_address}")
        logger.info(f"{uid_prefix} Contract Address: {self.contract_address}")
        logger.info(f"{uid_prefix} UID Bytes: {self.miner_uid_bytes.hex()}")

    def setup_routes(self):
        """Set up routes for the miner server."""

        @self.app.post("/receive-task")
        async def receive_task(task: TaskModel, background_tasks: BackgroundTasks):
            logger.info(
                f":inbox_tray: [Miner:{self.miner_uid_hex}] Received task [yellow]{task.task_id}[/yellow]"
            )
            background_tasks.add_task(self.handle_task_async, task)
            return {"message": f"Task {task.task_id} received and processing"}

    async def handle_task_async(self, task: TaskModel):
        """Async handler for processing tasks and sending results."""
        try:
            # 1. Process Task
            result_data = await self.process_task(task.task_data)

            # 2. Generate zkML Proof (if required/enabled)
            proof = None
            # Check if task requires proof (e.g., via flag in task_data or global config)
            if task.task_data.get("require_proof", False):
                logger.info(f"Task {task.task_id} requires zkML proof. Generating...")
                try:
                    # Assuming result_data is compatible with zkML input
                    proof = self.zkml_manager.generate_proof(input_data=result_data)
                    if proof:
                        logger.info(
                            f"Proof generated successfully for task {task.task_id}"
                        )
                    else:
                        logger.warning(
                            f"Failed to generate proof for task {task.task_id}"
                        )
                except Exception as e:
                    logger.error(f"Error generating zkML proof: {e}")

            # 3. Prepare Result Model
            result_to_send = ResultModel(
                task_id=task.task_id,
                miner_uid=self.miner_uid_hex,
                result_data=result_data,
                proof=proof,
            )

            # 4. Send Result back to Validator
            target_validator_url = task.validator_endpoint
            if not target_validator_url:
                logger.error(f"No validator endpoint provided for task {task.task_id}")
                return

            submit_url = f"{target_validator_url.rstrip('/')}/v1/miner/submit_result"
            logger.info(f"Sending result to {submit_url}")

            async with httpx.AsyncClient() as client:
                resp = await client.post(
                    submit_url, json=result_to_send.dict(), timeout=30.0
                )
                resp.raise_for_status()
                logger.info(f"Result for task {task.task_id} submitted successfully.")

        except Exception as e:
            logger.error(f"Error handling task {task.task_id}: {e}")

    def _load_local_history(self) -> List[float]:
        """Tải danh sách lịch sử hiệu suất từ file cục bộ."""
        uid_prefix = f"[LoadHistory:{self.miner_uid_hex}]"
        if self.history_file.exists():
            try:
                with open(self.history_file, "r") as f:
                    history = json.load(f)
                    if isinstance(history, list) and all(
                        isinstance(x, (int, float)) for x in history
                    ):
                        # Use UID prefix
                        logger.debug(
                            f"{uid_prefix} Loaded {len(history)} performance scores from {self.history_file}"
                        )
                        return history
                    else:
                        # Use UID prefix
                        logger.warning(
                            f"{uid_prefix} Invalid data format in {self.history_file}. Resetting history."
                        )
            except (json.JSONDecodeError, OSError) as e:
                # Use UID prefix
                logger.error(
                    f"{uid_prefix} Error reading history file {self.history_file}: {e}. Resetting history."
                )
        else:
            # Use UID prefix
            logger.debug(
                f"{uid_prefix} History file {self.history_file} not found. Starting new history."
            )
        return []

    def _save_local_history(self, history: List[float]):
        """Lưu danh sách lịch sử hiệu suất vào file cục bộ."""
        uid_prefix = f"[SaveHistory:{self.miner_uid_hex}]"
        try:
            with open(self.history_file, "w") as f:
                json.dump(history, f)
            # Use UID prefix
            logger.debug(
                f"{uid_prefix} Saved {len(history)} performance scores to {self.history_file}"
            )
        except OSError as e:
            # Use UID prefix
            logger.error(
                f"{uid_prefix} Error writing history file {self.history_file}: {e}"
            )

    async def fetch_consensus_result(
        self, cycle_num: int, validator_api_url: str
    ) -> Optional[MinerConsensusResult]:
        """Lấy kết quả đồng thuận cho miner từ API validator cho một chu kỳ cụ thể."""
        target_url = f"{validator_api_url.rstrip('/')}/v1/consensus/results/{cycle_num}"
        uid_prefix = f"[FetchResult:{self.miner_uid_hex}:C{cycle_num}]"
        logger.debug(f"{uid_prefix} Fetching results from {target_url}")
        try:
            response = await self.http_client.get(target_url, timeout=10.0)
            response.raise_for_status()
            data = response.json()
            cycle_results = CycleConsensusResults(**data)

            if cycle_results.cycle != cycle_num:
                logger.warning(
                    f"{uid_prefix} Fetched results for wrong cycle ({cycle_results.cycle})!"
                )
                return None

            miner_result_data = cycle_results.results.get(self.miner_uid_hex)
            if miner_result_data:
                logger.info(f"{uid_prefix} Successfully fetched consensus result.")
                return miner_result_data
            else:
                logger.info(f"{uid_prefix} Result not found for self in cycle data.")
                return None
        except httpx.RequestError as e:
            logger.error(f"{uid_prefix} Network error fetching results: {e}")
            return None
        except httpx.HTTPStatusError as e:
            logger.error(
                f"{uid_prefix} HTTP error fetching results: Status {e.response.status_code}"
            )
            return None
        except Exception as e:  # Catch other errors like JSON parsing
            logger.exception(f"{uid_prefix} Unexpected error fetching results: {e}")
            return None

    async def find_own_utxo(self) -> Optional[UTxO]:
        """Tìm UTXO chứa MinerDatum của miner này trên blockchain."""
        logger.debug(
            f"Miner {self.miner_uid_hex}: Searching for own Datum UTxO at {self.contract_address}..."
        )
        try:
            # Sử dụng UID bytes đã được decode để tìm kiếm
            utxo = await asyncio.to_thread(
                get_utxo_from_str,  # get_utxo_from_str là hàm đồng bộ
                contract_address=self.contract_address,
                datumclass=MinerDatum,
                context=self.context,
                search_uid=self.miner_uid_bytes,  # <<<--- Dùng UID bytes
            )
            if utxo:
                logger.info(f"Miner {self.miner_uid_hex}: Found own UTxO: {utxo.input}")
                self.last_known_utxo = utxo
            else:
                logger.warning(
                    f"Miner {self.miner_uid_hex}: Own Datum UTxO not found at {self.contract_address}."
                )
                self.last_known_utxo = None
            return utxo
        except Exception as e:
            logger.exception(f"Miner {self.miner_uid_hex}: Error finding own UTxO: {e}")
            return None

    def calculate_new_datum(
        self,
        old_datum: MinerDatum,
        consensus_result: MinerConsensusResult,
        current_cycle: int,
    ) -> Optional[MinerDatum]:
        """
        Tính toán và tạo MinerDatum mới dựa trên trạng thái cũ và kết quả đồng thuận.
        """
        logger.debug(
            f"Miner {self.miner_uid_hex}: Calculating new datum for cycle {current_cycle}..."
        )
        try:
            divisor = self.config.METAGRAPH_DATUM_INT_DIVISOR
            p_adj = consensus_result.p_adj
            incentive_float = consensus_result.calculated_incentive

            # Lấy trạng thái cũ từ old_datum
            trust_score_old = old_datum.trust_score  # Property đã unscale
            rewards_old = old_datum.accumulated_rewards  # Đây là int đã scale
            on_chain_history_hash = old_datum.performance_history_hash
            local_history_old = self._load_local_history()

            # Xác minh local history với hash on-chain
            history_verified = False
            if on_chain_history_hash:
                try:
                    if verify_hash(local_history_old, on_chain_history_hash):
                        history_verified = True
                        logger.debug("Local performance history matches on-chain hash.")
                    else:
                        logger.warning(
                            "Local performance history MISMATCH with on-chain hash! Resetting local history."
                        )
                        local_history_old = []  # Reset nếu hash không khớp
                except Exception as e:
                    logger.error(
                        f"Error verifying history hash: {e}. Assuming mismatch and resetting."
                    )
                    local_history_old = []
            elif local_history_old:
                logger.warning(
                    "Local performance history exists but no on-chain hash found. Using local history."
                )
                # Có thể coi là hợp lệ nếu không có hash cũ để so sánh
                history_verified = (
                    True  # Hoặc đặt là False và reset tùy logic mong muốn
                )
            else:
                # Không có local history và không có hash on-chain -> Bắt đầu mới
                history_verified = True

            # Tạo history mới
            # Nếu history cũ không hợp lệ (không khớp hash), bắt đầu lại chỉ với điểm mới
            if not history_verified:
                history_for_new_datum = [p_adj] if p_adj is not None else []
            else:
                history_for_new_datum = local_history_old
                if p_adj is not None:  # Chỉ thêm nếu p_adj hợp lệ
                    history_for_new_datum.append(p_adj)

            # Cắt bớt history và tính hash mới
            history_for_new_datum = history_for_new_datum[
                -self.config.CONSENSUS_MAX_PERFORMANCE_HISTORY_LEN :
            ]
            try:
                new_perf_history_hash = (
                    hash_data(history_for_new_datum)
                    if history_for_new_datum
                    else on_chain_history_hash
                )  # Giữ hash cũ nếu history mới rỗng? Hoặc hash list rỗng?
            except Exception as e:
                logger.error(
                    f"Failed to hash new performance history: {e}. Keeping old hash."
                )
                new_perf_history_hash = on_chain_history_hash

            # Lưu history mới vào file cục bộ (chỉ khi hash thành công?)
            if (
                new_perf_history_hash != on_chain_history_hash
                or not self.history_file.exists()
                or not history_verified
            ):
                self._save_local_history(history_for_new_datum)
            history_old = []  # Tạm thời coi như bắt đầu lại history mỗi lần tính

            # --- Tính Trust Score mới ---
            # Giả định miner đã được đánh giá (time_since=1) vì có kết quả consensus
            new_trust_score_float = update_trust_score(
                trust_score_old=trust_score_old,
                time_since_last_eval=1,
                score_new=p_adj,  # Dùng P_adj từ kết quả consensus
                delta_trust=self.config.CONSENSUS_PARAM_DELTA_TRUST,
                alpha_base=self.config.CONSENSUS_PARAM_ALPHA_BASE,
                k_alpha=self.config.CONSENSUS_PARAM_K_ALPHA,
                update_sigmoid_L=self.config.CONSENSUS_PARAM_UPDATE_SIG_L,
                update_sigmoid_k=self.config.CONSENSUS_PARAM_UPDATE_SIG_K,
                update_sigmoid_x0=self.config.CONSENSUS_PARAM_UPDATE_SIG_X0,
            )
            # Đảm bảo trust score nằm trong khoảng [0, 1]
            new_trust_score_float = max(0.0, min(1.0, new_trust_score_float))

            # --- Tính Accumulated Rewards mới ---
            # Cộng dồn phần thưởng đã tính (incentive_float) vào reward cũ
            rewards_new = rewards_old + int(incentive_float * divisor)

            # --- Tính Performance History Hash mới ---
            # Thêm P_adj vào history (sau khi unscale?) -> Nên thêm P_adj (float)
            new_history = history_old + [p_adj]
            # Giữ lại độ dài tối đa theo cấu hình
            new_history = new_history[
                -self.config.CONSENSUS_MAX_PERFORMANCE_HISTORY_LEN :
            ]
            # Hash lại list history mới
            new_perf_history_hash = (
                hash_data(new_history)
                if new_history
                else old_datum.performance_history_hash
            )

            # --- Tạo đối tượng MinerDatum mới ---
            # Giữ nguyên các trường không thay đổi từ old_datum
            new_datum = MinerDatum(
                uid=old_datum.uid,  # <<<--- Dùng UID bytes từ datum cũ
                subnet_uid=old_datum.subnet_uid,
                stake=old_datum.stake,  # Giữ nguyên stake (chưa có logic thay đổi stake ở đây)
                scaled_last_performance=int(
                    p_adj * divisor
                ),  # <<<--- Cập nhật performance mới
                scaled_trust_score=int(
                    new_trust_score_float * divisor
                ),  # <<<--- Cập nhật trust mới
                accumulated_rewards=rewards_new,  # <<<--- Cập nhật reward mới
                last_update_slot=current_cycle,  # <<<--- Cập nhật slot
                performance_history_hash=new_perf_history_hash,  # <<<--- Cập nhật hash history mới
                wallet_addr_hash=old_datum.wallet_addr_hash,  # Giữ nguyên hash ví bytes
                status=old_datum.status,  # Giữ nguyên status (trừ khi có logic thay đổi)
                registration_slot=old_datum.registration_slot,  # Giữ nguyên slot đăng ký
                api_endpoint=old_datum.api_endpoint,  # Giữ nguyên endpoint bytes
            )
            logger.info(
                f"Miner {self.miner_uid_hex}: Calculated new MinerDatum: {new_datum}"
            )
            return new_datum

        except Exception as e:
            logger.exception(
                f"Miner {self.miner_uid_hex}: Error calculating new datum: {e}"
            )
            return None

    async def commit_self_update(
        self, old_utxo: UTxO, new_datum: MinerDatum
    ) -> Optional[str]:
        """Xây dựng, ký và gửi giao dịch tự cập nhật MinerDatum lên blockchain."""
        logger.info(
            f"Miner {self.miner_uid_hex}: Attempting to commit self-update transaction..."
        )
        try:
            builder = TransactionBuilder(context=self.context)

            # --- Input 1: UTXO Datum cũ của miner ---
            # Redeemer cần khớp với định nghĩa trong Plutus script cho hành động self-update
            # Giả định tag là MINER_SELF_UPDATE_REDEEMER_TAG và không cần data phức tạp
            self_update_redeemer = Redeemer(0)  # Data = None
            builder.add_script_input(
                utxo=old_utxo,
                script=self.script_bytes,  # Script Plutus dùng chung
                redeemer=self_update_redeemer,
            )
            logger.debug(f"Added script input: {old_utxo.input}")

            # --- Input 2: Từ ví cá nhân của miner để trả phí ---
            # TransactionBuilder sẽ tự động chọn UTXO từ địa chỉ này
            builder.add_input_address(self.miner_wallet_address)
            logger.debug(f"Added wallet input address: {self.miner_wallet_address}")

            # --- Output 1: Trả về contract với Datum mới và giữ nguyên giá trị ---
            output_value: Value = (
                old_utxo.output.amount
            )  # Giữ nguyên value (ADA + tokens nếu có)
            builder.add_output(
                TransactionOutput(
                    address=self.contract_address,
                    amount=output_value,
                    datum=new_datum,  # <<<--- Datum mới đã tính toán
                )
            )
            logger.debug(
                f"Added script output with new datum. Amount: {output_value.coin} Lovelace"
            )

            # --- Required Signer: Chính là miner ---
            # Chữ ký được kiểm tra dựa trên hash của payment verification key
            pay_vkey = self.signing_key.to_verification_key()
            builder.required_signers = [pay_vkey.hash()]  # type: ignore
            logger.debug(f"Set required signer: {pay_vkey.hash().to_primitive().hex()}")  # type: ignore

            # --- Build, Sign, Submit ---
            logger.debug("Building and signing transaction...")
            # Cần truyền danh sách khóa ký. Ít nhất là khóa payment.
            # Khóa stake chỉ cần nếu script yêu cầu hoặc UTXO input từ ví có stake part.
            signing_keys_list: List[SigningKey | ExtendedSigningKey] = [
                self.signing_key
            ]
            if self.stake_signing_key:
                # Kiểm tra xem input từ ví có cần khóa stake không (thường là cần nếu ví có stake part)
                # Hoặc nếu script yêu cầu (script always_true hiện tại thì không)
                signing_keys_list.append(self.stake_signing_key)

            # Build và ký giao dịch, tiền thừa trả về ví miner
            signed_tx = await asyncio.to_thread(
                builder.build_and_sign,
                signing_keys=signing_keys_list,
                change_address=self.miner_wallet_address,
            )
            # signed_tx = builder.build_and_sign(...) # Nếu build_and_sign là sync

            logger.info(
                f"Submitting self-update transaction (Tx Fee: {signed_tx.transaction_body.fee})..."
            )
            # Submit giao dịch - dùng to_thread nếu context.submit_tx là sync
            tx_id: TransactionId = await asyncio.to_thread(self.context.submit_tx, signed_tx.to_cbor())  # type: ignore
            # tx_id: TransactionId = self.context.submit_tx(signed_tx.to_cbor()) # Nếu submit_tx là sync
            tx_id_str = str(tx_id)
            logger.info(
                f"Miner {self.miner_uid_hex}: Self-update submitted successfully! TxID: {tx_id_str}"
            )
            # Cập nhật cache UTXO sau khi gửi thành công để tránh dùng lại UTXO cũ
            self.last_known_utxo = None
            return tx_id_str

        except Exception as e:
            logger.exception(
                f"Miner {self.miner_uid_hex}: Failed to commit self-update: {e}"
            )
            # Có thể log thêm thông tin builder để debug nếu cần
            # logger.error(f"Transaction builder state: Inputs={getattr(builder,'inputs',[])}, Outputs={getattr(builder,'outputs',[])}")
            return None

    async def consensus_loop(
        self, validator_api_url: str, check_interval_seconds: int = 60
    ):
        """
        Vòng lặp chính của Miner Agent: Fetch kết quả -> Tìm UTXO -> Tính Datum -> Commit.
        """
        uid_prefix = f"[RunLoop:{self.miner_uid_hex}]"
        logger.info(
            f"{uid_prefix} Starting consensus loop. Checking every {check_interval_seconds}s."
        )
        target_cycle = self.last_processed_cycle + 1  # Start checking the next cycle

        while True:
            try:
                logger.info(
                    f"{uid_prefix}:C{target_cycle} Checking for consensus results..."
                )
                consensus_result = await self.fetch_consensus_result(
                    target_cycle, validator_api_url
                )

                if consensus_result:
                    logger.info(
                        f"{uid_prefix}:C{target_cycle} Found results. Processing update..."
                    )

                    logger.info(f"{uid_prefix}:C{target_cycle} Finding own UTXO...")
                    current_utxo = await self.find_own_utxo()
                    if not current_utxo:
                        logger.error(
                            f"{uid_prefix}:C{target_cycle} Could not find own UTXO containing MinerDatum. Cannot update state."
                        )
                        # Optionally wait longer or implement retry logic?
                        target_cycle += 1  # Move to next cycle check
                        continue
                    self.last_known_utxo = current_utxo  # Cache it
                    logger.info(
                        f"{uid_prefix}:C{target_cycle} Found UTXO: {current_utxo.input}"
                    )

                    logger.info(
                        f"{uid_prefix}:C{target_cycle} Calculating new datum..."
                    )
                    old_datum_obj = MinerDatum.from_cbor(current_utxo.output.datum.cbor)  # type: ignore
                    if not isinstance(old_datum_obj, MinerDatum):
                        logger.error(
                            f"{uid_prefix}:C{target_cycle} Failed to decode old datum from UTXO {current_utxo.input} into MinerDatum object."
                        )
                        target_cycle += 1  # Skip this cycle
                        continue

                    # Now old_datum_obj is confirmed to be MinerDatum
                    new_datum = self.calculate_new_datum(
                        old_datum_obj, consensus_result, target_cycle
                    )

                    if not new_datum:
                        logger.error(
                            f"{uid_prefix}:C{target_cycle} Failed to calculate new datum. Skipping update."
                        )
                        target_cycle += 1
                        continue

                    logger.info(
                        f"{uid_prefix}:C{target_cycle} New datum calculated. Committing self-update transaction..."
                    )
                    tx_id = await self.commit_self_update(current_utxo, new_datum)

                    if tx_id:
                        logger.info(
                            f"{uid_prefix}:C{target_cycle} Self-update transaction submitted: {tx_id}. Update complete for this cycle."
                        )
                        self.last_processed_cycle = target_cycle
                        self.last_known_utxo = (
                            None  # Clear cache after successful update
                        )
                    else:
                        logger.error(
                            f"{uid_prefix}:C{target_cycle} Failed to submit self-update transaction. Will retry finding UTXO next time."
                        )
                        self.last_known_utxo = (
                            None  # Clear cache as UTXO might be spent by failed tx
                        )
                        # Do not increment target_cycle yet, retry this cycle after interval

                else:
                    logger.info(f"{uid_prefix}:C{target_cycle} No results found yet.")
                    # Only increment cycle if we didn't process it
                    # This logic might need adjustment depending on whether validator API guarantees results eventually
                    # If results might be missing permanently, we might need to skip cycles after some time.
                    # For now, assume results will appear eventually if the cycle happened.
                    # Maybe check current block height vs expected cycle end?
                    pass  # Keep checking the same target_cycle

            except Exception as loop_err:
                logger.exception(
                    f"{uid_prefix}:C{target_cycle} Unhandled error in MinerAgent run loop: {loop_err}"
                )
                # Avoid busy-looping on persistent errors
                await asyncio.sleep(check_interval_seconds * 2)

            # Wait before next check
            logger.debug(
                f"{uid_prefix}:C{target_cycle} Waiting {check_interval_seconds}s before next check..."
            )
            await asyncio.sleep(check_interval_seconds)
            # Increment target cycle ONLY if we successfully processed OR confirmed no results yet for a reasonable time? TBD.
            # Simplest for now: always check next cycle if current has been processed or if fetch returned None explicitly.
            if (
                consensus_result is None
            ):  # Only increment if fetch explicitly returned None (meaning cycle check is done)
                target_cycle += 1

    async def run(
        self,
        validator_api_url: str = "http://localhost:8000",
        check_interval_seconds: int = 60,
        host: str = "0.0.0.0",
        port: int = 8001,
    ):
        """
        Starts the Miner Agent: API Server + Consensus Loop.
        """
        logger.info(f"Starting Miner Agent on {host}:{port}")

        # Start API Server
        config = uvicorn.Config(self.app, host=host, port=port, log_level="info")
        server = uvicorn.Server(config)

        # Run server and consensus loop concurrently
        await asyncio.gather(
            server.serve(),
            self.consensus_loop(validator_api_url, check_interval_seconds),
        )

    async def process_task(self, task_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Xử lý task nhận được từ Validator.
        1. Sử dụng SubnetProtocol để giải quyết task.
        2. Tạo zkML proof cho quá trình tính toán.
        """
        if not self.subnet_protocol:
            logger.error("No Subnet Protocol loaded. Cannot process task.")
            return {}

        try:
            # 1. Solve Task
            logger.info(
                f"Processing task with protocol: {self.subnet_protocol.get_metadata()['name']}"
            )
            result_data = self.subnet_protocol.solve_task(task_data)

            # 2. Generate zkML Proof (Mocked input for now)
            # Trong thực tế, input_data sẽ là tensor đầu vào và output của model
            input_data = [1.0, 2.0, 3.0]  # Placeholder
            proof = self.zkml_manager.generate_proof(input_data)

            if proof:
                result_data["proof"] = proof
                logger.info("Generated zkML proof for task result.")

            return result_data

        except Exception as e:
            logger.exception(f"Error processing task: {e}")
            return {}

    async def close(self):
        """Đóng các tài nguyên (ví dụ: http client) khi agent dừng."""
        if self.http_client:
            try:
                await self.http_client.aclose()
                logger.info(f"MinerAgent for {self.miner_uid_hex} closed HTTP client.")
            except Exception as e:
                logger.error(
                    f"Error closing HTTP client for MinerAgent {self.miner_uid_hex}: {e}"
                )
