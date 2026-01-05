# tests/consensus/test_signature_verification.py

import pytest
import binascii
import time
import copy  # Để tạo bản sao dữ liệu cho việc sửa đổi

# --- Imports từ pycardano ---
from sdk.compat.pycardano import (
    PaymentKeyPair,
    PaymentSigningKey,
    PaymentVerificationKey,
    Address,
    Network,
    VerificationKeyHash,
)

# --- Imports từ SDK ---
from sdk.core.datatypes import ValidatorInfo, ValidatorScore
from sdk.network.app.api.v1.endpoints.consensus import (
    ScoreSubmissionPayload,
    verify_payload_signature,  # Import hàm cần test
)
from sdk.consensus.p2p import canonical_json_serialize  # Import hàm serialize

# --- Fixtures (Có thể tách ra conftest.py nếu dùng nhiều) ---


@pytest.fixture(scope="module")
def key_pair_submitter() -> PaymentKeyPair:
    """Tạo một cặp khóa Payment cho người gửi giả lập."""
    return PaymentKeyPair.generate()


@pytest.fixture(scope="module")
def submitter_info(key_pair_submitter: PaymentKeyPair) -> ValidatorInfo:
    """Tạo ValidatorInfo khớp với key_pair_submitter."""
    network = Network.TESTNET  # Hoặc MAINNET tùy cấu hình
    vk = key_pair_submitter.verification_key
    # Tạo địa chỉ chỉ có payment part (ví dụ)
    addr = Address(payment_part=vk.hash(), network=network)
    # Tạo ValidatorInfo mẫu
    return ValidatorInfo(
        uid="validator_submitter_hex",  # UID dạng hex string
        address=str(addr),
        api_endpoint="http://submitter.example.com",
        trust_score=0.9,
        weight=10.0,
        stake=1000.0,
        last_performance=0.95,
        status=1,  # ACTIVE
        # Thêm các trường khác nếu ValidatorInfo yêu cầu
    )


@pytest.fixture
def sample_scores() -> list[ValidatorScore]:
    """Tạo danh sách điểm số mẫu."""
    return [
        ValidatorScore(
            task_id="t1",
            miner_uid="m1_hex",
            validator_uid="validator_submitter_hex",
            score=0.95,
            timestamp=time.time(),
        ),
        ValidatorScore(
            task_id="t2",
            miner_uid="m2_hex",
            validator_uid="validator_submitter_hex",
            score=0.88,
            timestamp=time.time() + 0.1,
        ),
    ]


# --- Test Cases ---


@pytest.mark.asyncio
async def test_verify_signature_valid(
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra trường hợp chữ ký và VKey hoàn toàn hợp lệ."""
    signing_key = key_pair_submitter.signing_key
    verification_key = key_pair_submitter.verification_key

    # 1. Serialize dữ liệu điểm số
    data_to_sign_str = canonical_json_serialize(sample_scores)
    data_to_sign_bytes = data_to_sign_str.encode("utf-8")

    # 2. Ký dữ liệu
    signature_bytes = signing_key.sign(data_to_sign_bytes)
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")

    # --- THÊM BƯỚC KIỂM TRA NGAY LẬP TỨC ---
    print("\nDEBUG: Checking verification key immediately after signing...")
    try:
        # --- Sử dụng PyNaCl để xác thực ---
        import nacl.signing
        import nacl.exceptions

        vk_bytes = verification_key.to_primitive()  # Lấy raw bytes
        nacl_vk = nacl.signing.VerifyKey(vk_bytes)
        try:
            # PyNaCl verify(message, signature) raises BadSignatureError on failure
            nacl_vk.verify(data_to_sign_bytes, signature_bytes)
            verify_immediately = True
            print(f"DEBUG: Immediate PyNaCl verification result: {verify_immediately}")
        except nacl.exceptions.BadSignatureError:
            verify_immediately = False
            print("DEBUG: Immediate PyNaCl verification FAILED (BadSignatureError)")
        # --- Kết thúc sử dụng PyNaCl ---

        assert (
            verify_immediately is True
        ), "Immediate verification using original key failed!"
        print("DEBUG: Immediate verification successful.")
    except AttributeError as ae:
        # This shouldn't happen if to_primitive exists
        print(
            f"ERROR: The original 'verification_key' object MISSES to_primitive() method? Details: {ae}"
        )
        pytest.fail("Original VerificationKey object lacks to_primitive method.")
    except Exception as e:
        print(f"ERROR: Immediate verification failed with exception: {e}")
        pytest.fail(f"Immediate verification failed: {e}")
    # --- KẾT THÚC BƯỚC KIỂM TRA NGAY LẬP TỨC ---

    # 3. Lấy VKey CBOR hex
    vkey_cbor_hex = verification_key.to_cbor_hex()

    # 4. Tạo Payload
    payload = ScoreSubmissionPayload(
        scores=sample_scores,
        submitter_validator_uid=submitter_info.uid,
        cycle=101,
        submitter_vkey_cbor_hex=vkey_cbor_hex,
        signature=signature_hex,
    )

    # 5. Gọi hàm xác thực (hàm này vẫn có thể lỗi nếu verify bên trong nó lỗi)
    print("DEBUG: Calling verify_payload_signature function...")
    is_valid = await verify_payload_signature(submitter_info, payload)
    print(f"DEBUG: Result from verify_payload_signature: {is_valid}")

    # 6. Kiểm tra kết quả cuối cùng
    assert is_valid is True


@pytest.mark.asyncio
async def test_verify_signature_invalid_signature(
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra trường hợp chữ ký không hợp lệ (bị sửa đổi)."""
    signing_key = key_pair_submitter.signing_key
    verification_key = key_pair_submitter.verification_key

    data_to_sign_str = canonical_json_serialize(sample_scores)
    data_to_sign_bytes = data_to_sign_str.encode("utf-8")
    signature_bytes = signing_key.sign(data_to_sign_bytes)
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")
    vkey_cbor_hex = verification_key.to_cbor_hex()

    # Sửa đổi chữ ký một chút
    tampered_signature_hex = signature_hex[:-1] + (
        "a" if signature_hex[-1] != "a" else "b"
    )

    payload = ScoreSubmissionPayload(
        scores=sample_scores,
        submitter_validator_uid=submitter_info.uid,
        cycle=101,
        submitter_vkey_cbor_hex=vkey_cbor_hex,
        signature=tampered_signature_hex,  # Chữ ký sai
    )

    is_valid = await verify_payload_signature(submitter_info, payload)
    assert is_valid is False


@pytest.mark.asyncio
async def test_verify_signature_vkey_mismatch_address(
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,  # info này có address từ key_pair_submitter
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra trường hợp VKey gửi lên không khớp với địa chỉ của submitter_info."""
    signing_key = key_pair_submitter.signing_key  # Dùng SK gốc để ký

    # Tạo một cặp khóa khác
    other_key_pair = PaymentKeyPair.generate()
    other_verification_key = other_key_pair.verification_key
    other_vkey_cbor_hex = (
        other_verification_key.to_cbor_hex()
    )  # VKey không khớp address

    data_to_sign_str = canonical_json_serialize(sample_scores)
    data_to_sign_bytes = data_to_sign_str.encode("utf-8")
    signature_bytes = signing_key.sign(data_to_sign_bytes)  # Ký bằng SK gốc
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")

    payload = ScoreSubmissionPayload(
        scores=sample_scores,
        submitter_validator_uid=submitter_info.uid,
        cycle=101,
        submitter_vkey_cbor_hex=other_vkey_cbor_hex,  # Gửi VKey không khớp
        signature=signature_hex,
    )

    is_valid = await verify_payload_signature(submitter_info, payload)
    assert is_valid is False


@pytest.mark.asyncio
async def test_verify_signature_data_tampered(
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra trường hợp dữ liệu scores trong payload bị thay đổi sau khi ký."""
    signing_key = key_pair_submitter.signing_key
    verification_key = key_pair_submitter.verification_key

    # Dữ liệu gốc để ký
    original_scores = copy.deepcopy(sample_scores)  # Tạo bản sao sâu
    data_to_sign_str = canonical_json_serialize(original_scores)
    data_to_sign_bytes = data_to_sign_str.encode("utf-8")
    signature_bytes = signing_key.sign(data_to_sign_bytes)
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")
    vkey_cbor_hex = verification_key.to_cbor_hex()

    # Dữ liệu bị sửa đổi trong payload
    tampered_scores = copy.deepcopy(sample_scores)
    if tampered_scores:
        tampered_scores[0].score = 0.1  # Thay đổi điểm số

    payload = ScoreSubmissionPayload(
        scores=tampered_scores,  # Dữ liệu scores đã bị sửa
        submitter_validator_uid=submitter_info.uid,
        cycle=101,
        submitter_vkey_cbor_hex=vkey_cbor_hex,
        signature=signature_hex,  # Chữ ký của dữ liệu gốc
    )

    is_valid = await verify_payload_signature(submitter_info, payload)
    assert is_valid is False


@pytest.mark.asyncio
async def test_verify_signature_missing_signature(
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra trường hợp thiếu chữ ký trong payload."""
    verification_key = key_pair_submitter.verification_key
    vkey_cbor_hex = verification_key.to_cbor_hex()

    payload = ScoreSubmissionPayload(
        scores=sample_scores,
        submitter_validator_uid=submitter_info.uid,
        cycle=101,
        submitter_vkey_cbor_hex=vkey_cbor_hex,
        signature=None,  # Thiếu chữ ký
    )

    is_valid = await verify_payload_signature(submitter_info, payload)
    assert is_valid is False


@pytest.mark.asyncio
async def test_verify_signature_missing_vkey(
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra trường hợp thiếu VKey trong payload."""
    signing_key = key_pair_submitter.signing_key
    data_to_sign_str = canonical_json_serialize(sample_scores)
    data_to_sign_bytes = data_to_sign_str.encode("utf-8")
    signature_bytes = signing_key.sign(data_to_sign_bytes)
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")

    payload = ScoreSubmissionPayload(
        scores=sample_scores,
        submitter_validator_uid=submitter_info.uid,
        cycle=101,
        submitter_vkey_cbor_hex=None,  # Thiếu VKey
        signature=signature_hex,
    )

    is_valid = await verify_payload_signature(submitter_info, payload)
    assert is_valid is False


@pytest.mark.asyncio
async def test_verify_signature_invalid_vkey_format(
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra trường hợp VKey CBOR hex không hợp lệ."""
    signing_key = key_pair_submitter.signing_key
    data_to_sign_str = canonical_json_serialize(sample_scores)
    data_to_sign_bytes = data_to_sign_str.encode("utf-8")
    signature_bytes = signing_key.sign(data_to_sign_bytes)
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")

    payload = ScoreSubmissionPayload(
        scores=sample_scores,
        submitter_validator_uid=submitter_info.uid,
        cycle=101,
        submitter_vkey_cbor_hex="invalid_hex_string",  # VKey sai định dạng
        signature=signature_hex,
    )

    is_valid = await verify_payload_signature(submitter_info, payload)
    assert is_valid is False
