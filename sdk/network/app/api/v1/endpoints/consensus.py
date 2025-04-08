# sdk/network/app/api/v1/endpoints/consensus.py
"""
API endpoints cho việc trao đổi thông tin đồng thuận giữa các validator.
"""
import logging
import asyncio
import json
import binascii
from typing import List, Annotated, Optional
from fastapi import APIRouter, HTTPException, Depends, status

from pydantic import BaseModel, Field, ValidationError
from sdk.core.datatypes import ValidatorScore, ValidatorInfo
# Import các kiểu Cardano cần thiết
from pycardano import Address, VerificationKey, PaymentVerificationKey, VerificationKeyHash
from sdk.consensus.node import ValidatorNode
from sdk.network.app.dependencies import get_validator_node
# Import hàm serialize từ p2p
from sdk.consensus.p2p import canonical_json_serialize


# --- Định nghĩa Pydantic Model cho Payload ---
class ScoreSubmissionPayload(BaseModel):
    """Dữ liệu điểm số gửi qua API, bao gồm VKey và chữ ký."""
    scores: List[ValidatorScore] = Field(..., description="Danh sách điểm số chi tiết ValidatorScore")
    submitter_validator_uid: str = Field(..., description="UID (dạng hex) của validator gửi điểm")
    cycle: int = Field(..., description="Chu kỳ đồng thuận mà điểm số này thuộc về")
    # --- Thêm trường VKey của người gửi ---
    submitter_vkey_cbor_hex: Optional[str] = Field(None, description="Payment Verification Key của người gửi (CBOR hex)")
    # -------------------------------------
    signature: Optional[str] = Field(None, description="Chữ ký (dạng hex) của hash(scores) để xác thực người gửi")

# --- Router ---
router = APIRouter(
    prefix="/consensus",
    tags=["Consensus P2P"],
)

logger = logging.getLogger(__name__)

# --- Hàm xác thực chữ ký (Placeholder - Cần Verification Key) ---
# --- Hàm xác thực chữ ký (Triển khai thực tế) ---
# --- Hàm xác thực chữ ký (Triển khai thực tế) ---
async def verify_payload_signature(
    submitter_info: ValidatorInfo,
    payload: ScoreSubmissionPayload
) -> bool:
    signature_hex = payload.signature
    submitter_vkey_hex = payload.submitter_vkey_cbor_hex
    scores_data = payload.scores

    if not signature_hex or not submitter_vkey_hex:
        logger.warning(f"SigVerifyFail ({submitter_info.uid}): Missing signature or VKey.")
        return False

    logger.debug(f"Verifying signature for validator {submitter_info.uid}...")

    try:
        # 1. Lấy payment hash mong đợi từ địa chỉ người gửi
        submitter_address = Address.from_primitive(submitter_info.address)
        expected_payment_hash: VerificationKeyHash = submitter_address.payment_part
        logger.debug(f"Expected payment hash: {expected_payment_hash.to_primitive().hex()}")

        # 2. Load VKey từ payload
        try:
             received_vkey = PaymentVerificationKey.from_cbor_hex(submitter_vkey_hex)
        except Exception as key_load_e:
             logger.error(f"SigVerifyFail ({submitter_info.uid}): Failed to load VKey from CBOR: {key_load_e}")
             return False

        # 3. Xác minh VKey: Hash của VKey nhận được phải khớp với hash từ địa chỉ
        received_vkey_hash: VerificationKeyHash = received_vkey.hash()
        logger.debug(f"Received VKey hash: {received_vkey_hash.to_primitive().hex()}")

        if received_vkey_hash != expected_payment_hash:
            logger.warning(f"SigVerifyFail ({submitter_info.uid}): VKey hash mismatch! Expected {expected_payment_hash.to_primitive().hex()}, got {received_vkey_hash.to_primitive().hex()}.")
            return False
        logger.debug(f"Submitter VKey matches address hash.")

        # 4. Xác minh Chữ ký
        data_to_verify_str = canonical_json_serialize(scores_data)
        data_to_verify_bytes = data_to_verify_str.encode('utf-8')
        signature_bytes = binascii.unhexlify(signature_hex)

        is_valid = received_vkey.verify(signature_bytes, data_to_verify_bytes)
        logger.debug(f"Signature verification result for {submitter_info.uid}: {is_valid}")
        if not is_valid:
            logger.warning(f"SigVerifyFail ({submitter_info.uid}): Invalid signature.")
        return is_valid

    except binascii.Error:
         logger.error(f"SigVerifyFail ({submitter_info.uid}): Invalid signature hex format.")
         return False
    except Exception as e:
        logger.error(f"SigVerifyFail ({submitter_info.uid}): Error during verification: {e}", exc_info=True)
        return False
# ---

# --- API Endpoint ---

@router.post("/receive_scores",
             summary="Nhận điểm số từ Validator khác",
             description="Endpoint để một Validator gửi danh sách điểm số (ValidatorScore) mà nó đã chấm cho các Miner trong một chu kỳ. Yêu cầu chữ ký hợp lệ.",
             status_code=status.HTTP_202_ACCEPTED)
async def receive_scores(
    payload: ScoreSubmissionPayload,
    node: Annotated[ValidatorNode, Depends(get_validator_node)]
):
    submitter_uid = payload.submitter_validator_uid
    current_cycle = node.current_cycle
    payload_cycle = payload.cycle

    logger.info(f"API: Received scores from {submitter_uid} for cycle {payload_cycle} (Node cycle: {current_cycle})")

    # Xác thực người gửi (logic như cũ)
    submitter_info = node.validators_info.get(submitter_uid)
    if not submitter_info or getattr(submitter_info, 'status', 1) == 0:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail=f"Unknown/inactive submitter: {submitter_uid}")

    # Kiểm tra chu kỳ (chỉ chấp nhận chu kỳ hiện tại)
    if payload_cycle != current_cycle:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail=f"Invalid cycle: {payload_cycle}. Current: {current_cycle}.")

    # Xác minh chữ ký (gọi hàm mới)
    if not await verify_payload_signature(submitter_info, payload):
        logger.warning(f"Rejected scores from {submitter_uid} due to invalid signature/VKey.")
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Invalid signature or verification key.")
    logger.debug(f"Signature verified for scores from {submitter_uid}")

    # Thêm điểm số đã xác minh (logic như cũ)
    try:
        await node.add_received_score(submitter_uid, payload_cycle, payload.scores)
        logger.info(f"Successfully processed {len(payload.scores)} scores from {submitter_uid}")
        return {"message": f"Accepted {len(payload.scores)} scores from {submitter_uid}."}
    except Exception as e: # Bắt lỗi chung khi thêm điểm
        logger.exception(f"API Error processing scores from {submitter_uid} after verification: {e}")
        raise HTTPException(status_code=status.HTTP_500_INTERNAL_SERVER_ERROR, detail=f"Error processing scores: {e}")
