# File: sdk/network/app/api/v1/endpoints/consensus.py

import logging
import asyncio
import json
import binascii
import time
import dataclasses  # <<<--- Thêm import dataclasses
from typing import List, Annotated, Optional
from fastapi import APIRouter, HTTPException, Depends, status

from pydantic import BaseModel, Field, ValidationError

# Import các kiểu dữ liệu và node từ SDK
from sdk.core.datatypes import ValidatorScore, ValidatorInfo, ScoreSubmissionPayload
from sdk.consensus.node import ValidatorNode
from sdk.network.app.dependencies import get_validator_node

# Import hàm serialize từ P2P
# from sdk.consensus.p2p import canonical_json_serialize
from sdk.consensus.scoring import canonical_json_serialize


# Import các kiểu PyCardano và PyNaCl
from pycardano import (
    Address,
    VerificationKey,
    PaymentVerificationKey,
    VerificationKeyHash,
    Network,
)  # <<<--- Thêm Network
import nacl.signing
import nacl.exceptions

# --- Router ---
router = APIRouter(prefix="/consensus", tags=["Consensus P2P"])
logger = logging.getLogger(__name__)


# --- Hàm xác thực chữ ký (Đã sửa đổi và thêm logging) ---
async def verify_payload_signature(
    receiver_node: "ValidatorNode",  # Node đang nhận request
    payload: ScoreSubmissionPayload,  # Dữ liệu nhận được
) -> bool:
    """Xác minh chữ ký và VKey trong payload nhận được từ một peer."""
    signature_hex = payload.signature
    submitter_vkey_cbor_hex = payload.submitter_vkey_cbor_hex
    scores_list_dict = payload.scores  # Đây là list các dict
    submitter_uid = payload.submitter_validator_uid  # UID của người gửi

    if not signature_hex or not submitter_vkey_cbor_hex:
        logger.warning(
            f"SigVerifyFail (Receiver: {receiver_node.info.uid}, Sender: {submitter_uid}): Missing signature or VKey in payload."
        )
        return False

    logger.debug(f"Verifying signature for payload from validator {submitter_uid}...")

    # --- Lấy thông tin người gửi đã biết từ state của node NHẬN ---
    submitter_info = receiver_node.validators_info.get(submitter_uid)
    if not submitter_info:
        logger.warning(
            f"SigVerifyFail (Receiver: {receiver_node.info.uid}): Submitter validator {submitter_uid} not found in local state."
        )
        # Có thể trả về False hoặc raise lỗi tùy thiết kế bảo mật
        # Nếu không biết người gửi, không thể lấy địa chỉ để so sánh hash VKey
        return False  # Từ chối nếu không biết người gửi

    try:
        # 1. Lấy Payment Key Hash dự kiến từ địa chỉ của người gửi đã biết
        try:
            submitter_address = Address.from_primitive(submitter_info.address)
            payment_part = submitter_address.payment_part
            if isinstance(payment_part, VerificationKeyHash):
                expected_payment_hash: Optional[VerificationKeyHash] = payment_part
                expected_hash_hex = expected_payment_hash.to_primitive().hex()
                logger.debug(
                    f"Expected payment hash from known address: {expected_hash_hex}"
                )
            else:
                logger.warning(
                    f"SigVerifyFail (Sender: {submitter_uid}): Known address '{submitter_info.address}' is not a base address (payment part is {type(payment_part).__name__})."
                )
                return False  # Cannot verify VKey against non-base address
        except Exception as addr_e:
            logger.warning(
                f"SigVerifyFail (Sender: {submitter_uid}): Error parsing known address '{submitter_info.address}': {addr_e}"
            )
            return False

        # 2. Load Payment Verification Key gửi kèm trong payload
        try:
            vkey_cbor_bytes = binascii.unhexlify(submitter_vkey_cbor_hex)
            received_payment_vkey = PaymentVerificationKey.from_cbor(vkey_cbor_bytes)
            received_vk_bytes = received_payment_vkey.to_primitive()
            if not isinstance(received_vk_bytes, bytes) or len(received_vk_bytes) != 32:
                logger.warning(
                    f"SigVerifyFail (Sender: {submitter_uid}): Decoded VKey primitive is not bytes or has unexpected length ({len(received_vk_bytes) if isinstance(received_vk_bytes, bytes) else type(received_vk_bytes).__name__})."
                )
                return False
        except (binascii.Error, ValueError, TypeError) as key_load_e:
            logger.error(
                f"SigVerifyFail (Sender: {submitter_uid}): Failed to load/decode VKey from CBOR hex '{submitter_vkey_cbor_hex[:10]}...': {key_load_e}"
            )
            return False
        except Exception as e:
            logger.error(
                f"SigVerifyFail (Sender: {submitter_uid}): Unexpected error processing received VKey: {e}"
            )
            return False

        # 3. Xác minh VKey: Hash của VKey nhận được phải khớp với hash từ địa chỉ đã biết
        try:
            received_vkey_hash: VerificationKeyHash = VerificationKeyHash(
                received_vk_bytes
            )
            received_hash_hex = received_vkey_hash.to_primitive().hex()
            logger.debug(f"Received VKey hash: {received_hash_hex}")
        except Exception as hash_e:
            logger.error(
                f"SigVerifyFail (Sender: {submitter_uid}): Failed to hash received VKey: {hash_e}"
            )
            return False

        if received_vkey_hash != expected_payment_hash:
            logger.warning(
                f"SigVerifyFail (Sender: {submitter_uid}): VKey hash MISMATCH!"
            )
            logger.warning(
                f" -> Receiver expected hash {expected_hash_hex} (derived from known address '{submitter_info.address}')"
            )
            logger.warning(f" -> Payload VKey hash is {received_hash_hex}")
            logger.warning(
                f" -> Ensure the address for {submitter_uid} is correct in the receiver's on-chain metagraph data."
            )
            return False
        logger.debug(
            f"Submitter VKey hash matches expected hash derived from known address."
        )

        # 4. Xác minh Chữ ký
        try:
            # >>> Use payload.scores directly, which is already List[ValidatorScore] <<<
            scores_objects_from_payload = payload.scores
            
            # Check timestamp for replay protection
            current_time = time.time()
            timestamp_age = current_time - payload.timestamp
            max_age_seconds = 300  # 5 minutes tolerance for clock skew and network delay
            
            if timestamp_age > max_age_seconds:
                logger.warning(
                    f"SigVerifyFail (Sender: {submitter_uid}): Payload timestamp too old ({timestamp_age:.1f}s > {max_age_seconds}s). Possible replay attack."
                )
                return False
            
            if timestamp_age < -60:  # Timestamp in future by more than 1 minute
                logger.warning(
                    f"SigVerifyFail (Sender: {submitter_uid}): Payload timestamp in future ({timestamp_age:.1f}s). Clock skew or manipulation."
                )
                return False

            # Check if the list of scores is empty after Pydantic parsing
            if not scores_objects_from_payload:
                logger.warning(
                    f"SigVerifyFail (Sender: {submitter_uid}): Payload contained no valid score objects after Pydantic parsing."
                )
                # Depending on policy, maybe allow empty lists?
                # If empty lists are disallowed for signing, return False.
                # return False

            # Serialize WITH timestamp and cycle for verification (must match signing format)
            payload_with_nonce = {
                "cycle": payload.cycle,
                "timestamp": payload.timestamp,
                "scores": scores_objects_from_payload
            }
            data_str_from_payload = canonical_json_serialize(payload_with_nonce)
            data_to_verify_bytes = data_str_from_payload.encode("utf-8")
            signature_bytes = binascii.unhexlify(signature_hex)

            # Tạo đối tượng VerifyKey của PyNaCl từ raw bytes của PaymentVerificationKey
            nacl_verify_key = nacl.signing.VerifyKey(received_vk_bytes)  # type: ignore

            # Thực hiện xác minh
            nacl_verify_key.verify(
                data_to_verify_bytes, signature_bytes
            )  # Ném BadSignatureError nếu sai

            is_valid = True
            logger.info(
                f"Signature verification SUCCESSFUL for payload from {submitter_uid}"
            )

        except nacl.exceptions.BadSignatureError:
            is_valid = False
            logger.warning(
                f"SigVerifyFail (Sender: {submitter_uid}): Invalid signature (PyNaCl BadSignatureError). Data or signature mismatch."
            )
            logger.debug(
                f" - Data used for verification: {data_str_from_payload[:200]}..."
            )  # Log một phần dữ liệu
            logger.debug(f" - Signature hex received: {signature_hex}")
        except binascii.Error:
            logger.error(
                f"SigVerifyFail (Sender: {submitter_uid}): Invalid signature hex format."
            )
            is_valid = False
        except Exception as verify_e:
            logger.exception(
                f"SigVerifyFail (Sender: {submitter_uid}): Error during signature verification step: {verify_e}"
            )
            is_valid = False

        return is_valid

    except Exception as outer_e:
        logger.exception(
            f"SigVerifyFail (Sender: {submitter_uid}): Unexpected outer error during verification: {outer_e}"
        )
        return False


# --- Rate Limiting State ---
from collections import defaultdict
from typing import DefaultDict

# Simple in-memory rate limiter (for production, use Redis or similar)
_request_counts: DefaultDict[str, list] = defaultdict(list)
_MAX_REQUESTS_PER_MINUTE = 10
_MAX_SCORES_PER_REQUEST = 1000
_RATE_LIMIT_WINDOW = 60  # seconds

def _check_rate_limit(submitter_uid: str) -> bool:
    """Check if submitter is within rate limits."""
    current_time = time.time()
    # Clean old entries
    _request_counts[submitter_uid] = [
        t for t in _request_counts[submitter_uid]
        if current_time - t < _RATE_LIMIT_WINDOW
    ]
    # Check limit
    if len(_request_counts[submitter_uid]) >= _MAX_REQUESTS_PER_MINUTE:
        return False
    # Record this request
    _request_counts[submitter_uid].append(current_time)
    return True


# --- API Endpoint ---
@router.post(
    "/receive_scores",
    summary="Nhận điểm số từ Validator khác",
    description="Endpoint để một Validator gửi danh sách điểm số (ValidatorScore) mà nó đã chấm. Yêu cầu chữ ký hợp lệ.",
    status_code=status.HTTP_202_ACCEPTED,
)
async def receive_scores(
    payload: ScoreSubmissionPayload,  # Dữ liệu gửi lên từ peer
    # Lấy instance ValidatorNode của node đang chạy API này
    node: Annotated[ValidatorNode, Depends(get_validator_node)],
):
    submitter_uid = payload.submitter_validator_uid
    current_cycle = node.current_cycle
    payload_cycle = payload.cycle

    # --- Payload Size Validation ---
    if len(payload.scores) > _MAX_SCORES_PER_REQUEST:
        logger.warning(
            f"API: Rejected oversized payload from {submitter_uid}: {len(payload.scores)} scores (max: {_MAX_SCORES_PER_REQUEST})"
        )
        raise HTTPException(
            status_code=status.HTTP_413_REQUEST_ENTITY_TOO_LARGE,
            detail=f"Too many scores in payload. Maximum allowed: {_MAX_SCORES_PER_REQUEST}",
        )
    
    # --- Rate Limiting ---
    if not _check_rate_limit(submitter_uid):
        logger.warning(
            f"API: Rate limit exceeded for {submitter_uid}. Rejecting request."
        )
        raise HTTPException(
            status_code=status.HTTP_429_TOO_MANY_REQUESTS,
            detail=f"Rate limit exceeded. Maximum {_MAX_REQUESTS_PER_MINUTE} requests per minute.",
        )

    logger.info(
        f"API: Received scores submission from V:{submitter_uid} for cycle {payload_cycle} (Node cycle: {current_cycle})"
    )

    # --- Bỏ qua chính mình (dù broadcast logic đã lọc) ---
    if submitter_uid == node.info.uid:
        logger.debug(f"API: Received scores from self ({submitter_uid}). Ignoring.")
        # Trả về thành công giả để tránh client báo lỗi không cần thiết
        return {"message": "Accepted scores from self (ignored)."}

    # --- Kiểm tra chu kỳ ---
    # Cho phép nhận điểm cho chu kỳ hiện tại hoặc chu kỳ trước đó một chút (đề phòng trễ mạng)
    if not (current_cycle - 1 <= payload_cycle <= current_cycle):
        logger.warning(
            f"API: Received scores for invalid cycle {payload_cycle} from {submitter_uid}. Current: {current_cycle}. Rejecting."
        )
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Invalid cycle: {payload_cycle}. Current: {current_cycle}.",
        )

    # --- Xác minh chữ ký và VKey ---
    # Hàm verify_payload_signature đã lấy submitter_info từ state của node nhận (node)
    if not await verify_payload_signature(node, payload):
        logger.warning(
            f"API: Rejected scores from {submitter_uid} for cycle {payload_cycle} due to invalid signature/VKey."
        )
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid signature or verification key.",
        )
    logger.debug(
        f"API: Signature verified for scores from {submitter_uid} for cycle {payload_cycle}"
    )

    # --- Thêm điểm số đã xác minh vào state của node ---
    try:
        # >>> Use payload.scores directly, which is already List[ValidatorScore] <<<
        scores_objects = payload.scores

        # Check if any scores were successfully parsed by Pydantic
        if not scores_objects:
            logger.info(
                f"API: No valid score entries parsed from payload from {submitter_uid} to add."
            )
            # Return success but indicate nothing was added
            return {
                "message": f"Accepted payload from {submitter_uid} (no scores found/parsed)."
            }

        # Gọi phương thức của Node để thêm điểm (scores_objects is already List[ValidatorScore])
        await node.add_received_score(submitter_uid, payload_cycle, scores_objects)
        logger.info(
            f"API: Successfully processed and stored {len(scores_objects)} scores from {submitter_uid} for cycle {payload_cycle}"
        )
        return {
            "message": f"Accepted {len(scores_objects)} scores from {submitter_uid} for cycle {payload_cycle}."
        }
    except Exception as e:  # Bắt lỗi chung khi thêm điểm
        logger.exception(
            f"API Error processing scores from {submitter_uid} after verification: {e}"
        )
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Internal server error processing scores: {e}",
        )
