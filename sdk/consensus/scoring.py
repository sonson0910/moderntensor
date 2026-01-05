# sdk/consensus/scoring.py
"""
Logic chấm điểm kết quả từ miners.
Chứa hàm cơ sở cần được kế thừa và triển khai bởi các validator/subnet cụ thể.
"""
import logging
from typing import List, Dict, Any, Optional, Union, cast, TYPE_CHECKING
from collections import defaultdict
from sdk.metagraph.metagraph_datum import STATUS_ACTIVE, STATUS_INACTIVE
import binascii
import asyncio
import httpx
import json
import nacl.signing
from nacl.exceptions import CryptoError
from pycardano import (
    PaymentVerificationKey,
    ExtendedVerificationKey,
    ExtendedSigningKey,
    PaymentSigningKey,
)

# Giả định các kiểu dữ liệu này đã được import hoặc định nghĩa đúng
try:
    from sdk.core.datatypes import (
        MinerResult,
        TaskAssignment,
        ValidatorScore,
        ValidatorInfo,
        ScoreSubmissionPayload,
    )
except ImportError as e:
    raise ImportError(f"Error importing dependencies in scoring.py: {e}")

from sdk.utils.zkml import ZkmlManager  # Import zkML Manager

logger = logging.getLogger(__name__)

# Khởi tạo ZkmlManager (có thể cần cấu hình path từ settings)
zkml_manager = ZkmlManager(model_path="model.onnx", settings_path="settings.json")


# --- Helper function for canonical serialization ---
def canonical_json_serialize(data: Any) -> str:
    """Serialize dữ liệu thành chuỗi JSON ổn định (sắp xếp key).

    Recursively converts dataclasses and dictionaries, handling bytes by
    encoding them as hex strings. Ensures consistent output for signing
    by sorting dictionary keys.

    Args:
        data: Dữ liệu cần serialize (có thể là dataclass, dict, list, etc.).

    Returns:
        Chuỗi JSON đại diện cho dữ liệu, với keys được sắp xếp.
    """
    import dataclasses  # Ensure dataclasses is imported here or globally

    def convert_to_dict(obj):
        if dataclasses.is_dataclass(obj):
            result = {}
            for f in dataclasses.fields(obj):
                value = getattr(obj, f.name)
                result[f.name] = convert_to_dict(value)
            return result
        elif isinstance(obj, list):
            return [convert_to_dict(item) for item in obj]
        elif isinstance(obj, dict):
            return {k: convert_to_dict(v) for k, v in obj.items()}
        # Thêm xử lý bytes -> hex string để JSON serialize được
        elif isinstance(obj, bytes):
            return obj.hex()
        else:
            return obj

    data_to_serialize = convert_to_dict(data)
    return json.dumps(data_to_serialize, sort_keys=True, separators=(",", ":"))


# --- END INSERT ---

if TYPE_CHECKING:
    # Đảm bảo import đúng đường dẫn tương đối hoặc tuyệt đối
    # from ..consensus.node import ValidatorNode # Incorrect relative path likely
    from sdk.consensus.node import ValidatorNode  # Use absolute path
    from ..core.datatypes import (
        ValidatorScore,
        ValidatorInfo,
    )


# --- 1. Đánh dấu hàm này cần override ---
# DEPRECATED: Logic này đã được chuyển vào SubnetProtocol.score_result
# Giữ lại để tương thích ngược nếu cần, nhưng nên sử dụng SubnetProtocol.
def _calculate_score_from_result(task_data: Any, result_data: Any) -> float:
    """
    (Deprecated) Tính điểm P_miner,v từ dữ liệu task và kết quả.

    Hiện tại logic này nên được implement trong SubnetProtocol.score_result.
    """
    logger.warning(
        f"'_calculate_score_from_result' in scoring.py is deprecated. "
        f"Use SubnetProtocol.score_result instead."
    )
    return 0.0


# ---------------------------------------


def score_results_logic(
    results_received: Dict[str, List[MinerResult]],
    tasks_sent: Dict[str, TaskAssignment],
    validator_uid: str,
    subnet_protocol: Optional[Any] = None,  # Thêm tham số subnet_protocol
) -> Dict[str, List[ValidatorScore]]:
    """
    Chấm điểm tất cả các kết quả hợp lệ nhận được từ miners cho chu kỳ hiện tại.

    Iterates through results received for each task ID. For each result, it verifies:
    1. If the task ID corresponds to a task actually sent by this validator.
    2. If the result came from the miner the task was assigned to.

    Valid results are then scored using `subnet_protocol.score_result` (if available)
    or fallback to `_calculate_score_from_result`.

    Args:
        results_received: Dictionary mapping task IDs to lists of `MinerResult` objects received.
                          {task_id: [MinerResult, MinerResult, ...]}.
        tasks_sent: Dictionary mapping task IDs to the `TaskAssignment` objects sent out.
                    {task_id: TaskAssignment}.
        validator_uid: UID (hex string) of the validator performing the scoring.
        subnet_protocol: (Optional) Instance of SubnetProtocol to use for
        results_received: Dictionary mapping task IDs to lists of `MinerResult` objects received.
                          {task_id: [MinerResult, MinerResult, ...]}.
        tasks_sent: Dictionary mapping task IDs to the `TaskAssignment` objects sent out.
                    {task_id: TaskAssignment}.
        validator_uid: UID (hex string) of the validator performing the scoring.

    Returns:
        Dictionary mapping task IDs to lists of `ValidatorScore` objects generated by this validator.
        {task_id: [ValidatorScore, ValidatorScore, ...]}. Returns scores only for valid, processed results.
    """
    logger.info(
        f"[V:{validator_uid}] Scoring {len(results_received)} received tasks..."
    )
    validator_scores: Dict[str, List[ValidatorScore]] = defaultdict(list)

    for task_id, results in results_received.items():
        assignment = tasks_sent.get(task_id)
        if not assignment:
            logger.warning(
                f"Scoring skipped: Task assignment not found for task_id {task_id}."
            )
            continue

        # Chỉ chấm điểm kết quả đầu tiên hợp lệ từ đúng miner? Hay chấm tất cả?
        # Tạm thời chấm kết quả đầu tiên từ đúng miner
        valid_result_found = False
        for result in results:
            if result.miner_uid == assignment.miner_uid:
                # --- Cải tiến: Verify zkML Proof ---
                penalty = 0.0
                if result.proof:
                    is_valid_proof = zkml_manager.verify_proof(result.proof)
                    if not is_valid_proof:
                        logger.warning(
                            f"Invalid zkML proof from Miner {result.miner_uid}. Score = 0. Penalty = 1.0"
                        )
                        score = 0.0
                        penalty = 1.0  # Phạt nặng vì fake proof
                    else:
                        # Proof hợp lệ, tiến hành chấm điểm logic
                        try:
                            if subnet_protocol:
                                score = subnet_protocol.score_result(
                                    assignment.task_data, result.result_data
                                )
                            else:
                                score = _calculate_score_from_result(
                                    assignment.task_data, result.result_data
                                )
                            score = max(0.0, min(1.0, score))
                        except Exception as e:
                            logger.error(f"Error scoring: {e}")
                            score = 0.0
                else:
                    # Nếu không có proof (với các task yêu cầu proof), có thể cho điểm 0 hoặc thấp
                    # Tạm thời vẫn chấm điểm nhưng log warning
                    logger.warning(f"Missing zkML proof from Miner {result.miner_uid}.")
                    try:
                        if subnet_protocol:
                            score = subnet_protocol.score_result(
                                assignment.task_data, result.result_data
                            )
                        else:
                            score = _calculate_score_from_result(
                                assignment.task_data, result.result_data
                            )
                        score = max(0.0, min(1.0, score))
                    except Exception:
                        score = 0.0

                valid_result_found = True

                logger.info(
                    f"  Scored Miner {result.miner_uid} for task {task_id}: {score:.4f} (Penalty: {penalty})"
                )

                val_score = ValidatorScore(
                    task_id=task_id,
                    miner_uid=result.miner_uid,
                    validator_uid=validator_uid,
                    score=score,
                    penalty=penalty,
                )
                validator_scores[task_id].append(val_score)
                break  # Chỉ chấm điểm kết quả hợp lệ đầu tiên từ đúng miner

        if not valid_result_found:
            logger.warning(
                f"No valid result found from expected miner {assignment.miner_uid} for task {task_id}. No score generated."
            )
            # Có thể tạo điểm 0 cho miner nếu không có kết quả hợp lệ?
            # val_score = ValidatorScore(...)
            # validator_scores[task_id].append(val_score)

    logger.info(
        f"Finished scoring. Generated scores for {len(validator_scores)} tasks."
    )
    return dict(validator_scores)


async def broadcast_scores_logic(
    validator_node: "ValidatorNode",
    cycle_scores_dict: Dict[str, List["ValidatorScore"]],
):
    """
    Gửi điểm số cục bộ (local_scores) đến các validator khác (peers), có ký dữ liệu.

    Performs the following steps:
    1. Fetches necessary info (signing key, active peers, http client) from the validator node.
    2. Flattens the `cycle_scores_dict` into a single list, keeping only scores generated
       by this validator (`self_uid`).
    3. If no local scores generated, logs a debug message and returns.
    4. Serializes the filtered list of scores into a canonical JSON string.
    5. Signs the serialized data using the validator's signing key (PyNaCl).
    6. Creates a `ScoreSubmissionPayload` containing the scores, signature (hex),
       and the validator's payment verification key (CBOR hex).
    7. Iterates through the list of active validator peers (excluding self).
    8. Sends the payload via HTTP POST to the `/submit_scores` endpoint of each peer.

    Args:
        validator_node: The instance of the `ValidatorNode` running this logic.
                        Provides access to configuration, keys, peers, and HTTP client.
        cycle_scores_dict: A dictionary containing scores generated or received
                           during the current cycle, keyed by task ID.
                           {task_id: [ValidatorScore, ...]}. This function will
                           filter and broadcast only the scores generated *by* this node.

    Raises:
        AttributeError: If `validator_node` is missing required attributes/methods.
        TypeError: If the derived verification key type is unexpected.
        CryptoError: If signing fails.
        httpx.RequestError: If sending the request to a peer fails (e.g., connection error, timeout).
        Exception: For other unexpected errors during setup, signing, or sending.
    """
    try:
        # Lấy thông tin cần thiết từ validator_node
        self_validator_info = validator_node.info
        # Cần ExtendedSigningKey để ký (hoặc PaymentSigningKey nếu node chỉ lưu key đó)
        signing_key: Union[ExtendedSigningKey, PaymentSigningKey] = validator_node.signing_key  # type: ignore
        # Lấy danh sách validator *active* từ node
        active_validator_peers = await validator_node._get_active_validators()
        current_cycle = validator_node.current_cycle
        http_client = validator_node.http_client
        settings = validator_node.settings
        self_uid = self_validator_info.uid  # UID của node hiện tại (dạng hex string)
    except AttributeError as e:  # Khôi phục khối except
        logger.error(
            f"Missing required attribute/method on validator_node for broadcasting: {e}"
        )
        return
    except Exception as e:  # Thêm một except chung để bắt lỗi khác khi lấy attributes
        logger.error(f"Error getting attributes from validator_node: {e}")
        return

    # --- Flatten và Lọc điểm cần gửi ---
    local_scores_list: List[ValidatorScore] = []
    for task_id, scores in cycle_scores_dict.items():
        for score in scores:
            if score.validator_uid == self_uid:
                local_scores_list.append(score)

    if not local_scores_list:
        logger.debug(f"[V:{self_uid}] No local scores to broadcast.")
        return

    logger.info(
        f"[V:{self_uid}] Preparing to broadcast {len(local_scores_list)} score entries generated by self for cycle {current_cycle}."
    )
    # Log peers for debugging
    active_peer_uids = [p.uid for p in active_validator_peers if p.uid != self_uid]
    logger.debug(
        f"[V:{self_uid}] Target active peers for broadcast: {active_peer_uids}"
    )

    # --- Ký Dữ liệu ---
    signature_hex: Optional[str] = None
    submitter_vkey_cbor_hex: Optional[str] = None
    try:
        # Lấy verification key (cần xử lý cả Extended và Payment)
        verification_key = signing_key.to_verification_key()

        # Chỉ lấy PaymentVerificationKey để gửi đi
        payment_vkey: PaymentVerificationKey
        if isinstance(verification_key, ExtendedVerificationKey):
            primitive_key = verification_key.to_primitive()[:32]
            payment_vkey = cast(
                PaymentVerificationKey,
                PaymentVerificationKey.from_primitive(primitive_key),
            )
        elif isinstance(verification_key, PaymentVerificationKey):
            payment_vkey = verification_key
        else:
            raise TypeError(
                f"Unexpected verification key type derived: {type(verification_key)}"
            )

        submitter_vkey_cbor_hex = payment_vkey.to_cbor_hex()

        # Serialize list điểm ĐÃ LỌC VÀ FLATTEN bằng hàm canonical
        data_to_sign_str = canonical_json_serialize(local_scores_list)
        data_to_sign_bytes = data_to_sign_str.encode("utf-8")

        # Ký bằng PyNaCl
        sk_primitive = signing_key.to_primitive()
        nacl_signing_key = nacl.signing.SigningKey(sk_primitive[:32])
        signed_pynacl = nacl_signing_key.sign(data_to_sign_bytes)
        signature_bytes = signed_pynacl.signature

        signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")
        logger.debug(f"[V:{self_uid}] Payload signed successfully.")
    except TypeError as type_e:
        logger.error(
            f"[V:{self_uid}] Type error during key derivation or serialization: {type_e}"
        )
        return
    except CryptoError as sign_e:  # USE IMPORTED CryptoError
        logger.exception(
            f"[V:{self_uid}] Failed to sign broadcast payload (PyNaCl): {sign_e}"
        )
        return
    except Exception as sign_e:  # Bắt lỗi chung khác
        logger.exception(
            f"[V:{self_uid}] Failed to prepare or sign broadcast payload: {sign_e}"
        )
        return

    # --- Tạo Payload ---
    # Đảm bảo các biến cần thiết đã được gán giá trị
    if signature_hex is None or submitter_vkey_cbor_hex is None:
        logger.error(
            f"[V:{self_uid}] Failed to obtain signature or verification key CBOR. Aborting broadcast."
        )
        return

    payload = ScoreSubmissionPayload(
        submitter_validator_uid=self_uid,
        cycle=current_cycle,
        scores=local_scores_list,
        signature=signature_hex,
        submitter_vkey_cbor_hex=submitter_vkey_cbor_hex,
    )
    logger.debug(
        f"[V:{self_uid}] ScoreSubmissionPayload created. Scores count: {len(payload.scores)}, Cycle: {payload.cycle}"
    )

    # --- Gửi đến Peers ---
    tasks = []
    peer_endpoints = {}
    for peer_info in active_validator_peers:  # Lấy thông tin endpoint từ ValidatorInfo
        if peer_info.uid == self_uid:
            continue  # Bỏ qua chính mình
        if peer_info.api_endpoint:
            peer_endpoints[peer_info.uid] = (
                f"{peer_info.api_endpoint.rstrip('/')}/submit_scores"
            )
        else:
            logger.warning(
                f"[V:{self_uid}] Peer {peer_info.uid} has no API endpoint defined. Skipping broadcast."
            )

    async def send_score(peer_uid: str, peer_endpoint: str, payload_dict: dict):
        """Gửi payload điểm số đến một peer cụ thể."""
        try:
            # === FIX: Remove incorrect async with for shared client ===
            # async with http_client as client:  # Sử dụng http_client từ validator_node
            # Use the shared http_client directly
            response = await http_client.post(  # <<< Use http_client directly
                peer_endpoint,
                json=payload_dict,
                headers={"Content-Type": "application/json"},
                timeout=settings.CONSENSUS_NETWORK_TIMEOUT_SECONDS,
            )
            if response.status_code == 200:
                logger.info(
                    f"[V:{self_uid}] Successfully sent scores to peer {peer_uid} at {peer_endpoint}"
                )
            else:
                logger.warning(
                    f"[V:{self_uid}] Failed to send scores to peer {peer_uid} at {peer_endpoint}: Status {response.status_code} - {response.text[:100]}..."
                )
        except httpx.RequestError as req_err:
            logger.warning(
                f"[V:{self_uid}] HTTP request error sending scores to peer {peer_uid} at {peer_endpoint}: {req_err}"
            )
        except Exception as e:
            logger.error(
                f"[V:{self_uid}] Unexpected error sending scores to peer {peer_uid} ({peer_endpoint}): {e}",
                exc_info=True,
            )

    payload_as_dict = payload.dict()  # Chuyển payload thành dict một lần
    for peer_uid, endpoint in peer_endpoints.items():
        tasks.append(send_score(peer_uid, endpoint, payload_as_dict))

    if tasks:
        logger.info(f"[V:{self_uid}] Broadcasting scores to {len(tasks)} peers...")
        await asyncio.gather(*tasks)
        logger.info(
            f"[V:{self_uid}] Finished broadcasting scores for cycle {current_cycle}."
        )
    else:
        logger.info(
            f"[V:{self_uid}] No active peers with endpoints found to broadcast scores to."
        )
