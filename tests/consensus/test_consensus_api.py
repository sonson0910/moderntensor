# tests/network/test_consensus_api.py

import pytest
import binascii
import time
import copy
import dataclasses
from unittest.mock import AsyncMock, MagicMock  # Dùng để mock ValidatorNode

# --- FastAPI testing ---
from fastapi.testclient import TestClient

# --- PyCardano imports ---
from sdk.compat.luxtensor_types import (
    PaymentKeyPair,
    PaymentSigningKey,
    PaymentVerificationKey,
    Address,
    Network,
)

# --- SDK imports ---
# Giả sử app FastAPI nằm ở sdk/network/app/main.py
from sdk.network.app.main import app  # Import FastAPI app instance
from sdk.core.datatypes import ValidatorInfo, ValidatorScore
from sdk.network.app.api.v1.endpoints.consensus import ScoreSubmissionPayload
from sdk.consensus.p2p import canonical_json_serialize
from sdk.network.app.dependencies import get_validator_node  # Dependency getter
from sdk.consensus.node import ValidatorNode  # Lớp Node gốc để mock

# --- PyNaCl for signing in test setup ---
import nacl.signing
import nacl.exceptions

# --- Fixtures ---


# Sử dụng lại fixtures từ test_signature_verification nếu chúng được đặt trong conftest.py
# Nếu không, định nghĩa lại chúng ở đây hoặc trong conftest.py của thư mục network
@pytest.fixture(scope="module")
def key_pair_submitter() -> PaymentKeyPair:
    return PaymentKeyPair.generate()


@pytest.fixture(scope="module")
def submitter_info(key_pair_submitter: PaymentKeyPair) -> ValidatorInfo:
    network = Network.TESTNET
    vk = key_pair_submitter.verification_key
    addr = Address(payment_part=vk.hash(), network=network)
    return ValidatorInfo(
        uid="validator_api_test_hex",
        address=str(addr),
        api_endpoint="http://submitter.example.com",
        trust_score=0.9,
        weight=10.0,
        stake=1000.0,
        last_performance=0.95,
        status=1,
    )


@pytest.fixture
def sample_scores() -> list[ValidatorScore]:
    # Tạo sample scores với UID của submitter trong fixture
    return [
        ValidatorScore(
            task_id="t1_api",
            miner_uid="m1_api_hex",
            validator_uid="validator_api_test_hex",
            score=0.92,
            timestamp=time.time(),
        ),
        ValidatorScore(
            task_id="t2_api",
            miner_uid="m2_api_hex",
            validator_uid="validator_api_test_hex",
            score=0.85,
            timestamp=time.time() + 0.1,
        ),
    ]


@pytest.fixture
def mock_validator_node(submitter_info: ValidatorInfo) -> MagicMock:
    """Tạo một mock object cho ValidatorNode."""
    mock_node = MagicMock(spec=ValidatorNode)  # Dùng MagicMock cho dễ
    mock_node.current_cycle = 105  # Chu kỳ hiện tại giả lập
    # Cần có thông tin của submitter trong validators_info của node mock
    mock_node.validators_info = {
        submitter_info.uid: submitter_info,
        "other_validator_hex": MagicMock(
            spec=ValidatorInfo, status=1
        ),  # Thêm validator khác nếu cần
    }
    # Mock phương thức add_received_score để kiểm tra nó có được gọi không
    # Vì endpoint là async, phương thức này cũng nên là AsyncMock
    mock_node.add_received_score = AsyncMock()
    return mock_node


@pytest.fixture
def test_client(mock_validator_node: MagicMock):
    """Tạo TestClient và override dependency get_validator_node."""
    # Override dependency: khi endpoint gọi get_validator_node, nó sẽ trả về mock_node
    app.dependency_overrides[get_validator_node] = lambda: mock_validator_node
    client = TestClient(app)
    yield client  # Trả về client cho test sử dụng
    # Dọn dẹp sau khi test xong
    app.dependency_overrides = {}


# --- Test Cases ---


def test_receive_scores_success(
    test_client: TestClient,
    mock_validator_node: MagicMock,  # Dùng để assert mock call
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra endpoint /receive_scores với request hợp lệ."""
    signing_key = key_pair_submitter.signing_key
    verification_key = key_pair_submitter.verification_key

    # Chuẩn bị dữ liệu hợp lệ
    data_to_sign_str = canonical_json_serialize(sample_scores)
    data_to_sign_bytes = data_to_sign_str.encode("utf-8")
    signature_bytes = signing_key.sign(data_to_sign_bytes)
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")
    vkey_cbor_hex = verification_key.to_cbor_hex()

    # Tạo payload JSON để gửi
    payload_dict = {
        "scores": [
            dataclasses.asdict(s) for s in sample_scores
        ],  # Chuyển score thành dict
        "submitter_validator_uid": submitter_info.uid,
        "cycle": mock_validator_node.current_cycle,  # Đúng cycle
        "submitter_vkey_cbor_hex": vkey_cbor_hex,
        "signature": signature_hex,
    }

    # Gửi request POST
    response = test_client.post("/v1/consensus/receive_scores", json=payload_dict)

    # Kiểm tra kết quả
    assert response.status_code == 202  # Accepted
    assert response.json()["message"].startswith("Accepted")

    # Kiểm tra xem mock_node.add_received_score có được gọi đúng không
    mock_validator_node.add_received_score.assert_awaited_once()
    # Lấy các tham số đã gọi
    call_args, call_kwargs = mock_validator_node.add_received_score.call_args
    # Kiểm tra các tham số
    assert call_args[0] == submitter_info.uid  # submitter_uid
    assert call_args[1] == mock_validator_node.current_cycle  # cycle
    # So sánh list các score object (cần đảm bảo chúng tương đương)
    received_scores_arg = call_args[2]
    assert len(received_scores_arg) == len(sample_scores)
    # Có thể cần so sánh chi tiết hơn từng score object nếu cần


# --- Viết thêm các test case cho trường hợp lỗi ---


def test_receive_scores_invalid_signature(
    test_client: TestClient,
    mock_validator_node: MagicMock,
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra endpoint với chữ ký không hợp lệ."""
    signing_key = key_pair_submitter.signing_key
    verification_key = key_pair_submitter.verification_key
    vkey_cbor_hex = verification_key.to_cbor_hex()
    # Tạo chữ ký sai
    signature_hex = "abcdef12345" * 10  # Chữ ký giả

    payload_dict = {
        "scores": [dataclasses.asdict(s) for s in sample_scores],
        "submitter_validator_uid": submitter_info.uid,
        "cycle": mock_validator_node.current_cycle,
        "submitter_vkey_cbor_hex": vkey_cbor_hex,
        "signature": signature_hex,  # Chữ ký sai
    }

    response = test_client.post("/v1/consensus/receive_scores", json=payload_dict)

    assert response.status_code == 401  # Unauthorized
    mock_validator_node.add_received_score.assert_not_awaited()  # Không được gọi


def test_receive_scores_vkey_mismatch(
    test_client: TestClient,
    mock_validator_node: MagicMock,
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra endpoint khi VKey không khớp địa chỉ submitter."""
    signing_key = key_pair_submitter.signing_key
    # Tạo VKey khác
    other_vk = PaymentKeyPair.generate().verification_key
    other_vkey_cbor_hex = other_vk.to_cbor_hex()

    # Ký bằng key gốc
    data_to_sign_str = canonical_json_serialize(sample_scores)
    signature_bytes = signing_key.sign(data_to_sign_str.encode("utf-8"))
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")

    payload_dict = {
        "scores": [dataclasses.asdict(s) for s in sample_scores],
        "submitter_validator_uid": submitter_info.uid,
        "cycle": mock_validator_node.current_cycle,
        "submitter_vkey_cbor_hex": other_vkey_cbor_hex,  # VKey không khớp
        "signature": signature_hex,
    }

    response = test_client.post("/v1/consensus/receive_scores", json=payload_dict)

    assert response.status_code == 401  # Unauthorized (do VKey hash mismatch)
    mock_validator_node.add_received_score.assert_not_awaited()


def test_receive_scores_invalid_cycle(
    test_client: TestClient,
    mock_validator_node: MagicMock,
    key_pair_submitter: PaymentKeyPair,
    submitter_info: ValidatorInfo,
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra endpoint khi cycle không đúng."""
    signing_key = key_pair_submitter.signing_key
    verification_key = key_pair_submitter.verification_key
    # Chuẩn bị signature/VKey hợp lệ
    data_to_sign_str = canonical_json_serialize(sample_scores)
    signature_bytes = signing_key.sign(data_to_sign_str.encode("utf-8"))
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")
    vkey_cbor_hex = verification_key.to_cbor_hex()

    payload_dict = {
        "scores": [dataclasses.asdict(s) for s in sample_scores],
        "submitter_validator_uid": submitter_info.uid,
        "cycle": mock_validator_node.current_cycle + 1,  # Cycle sai
        "submitter_vkey_cbor_hex": vkey_cbor_hex,
        "signature": signature_hex,
    }

    response = test_client.post("/v1/consensus/receive_scores", json=payload_dict)

    assert response.status_code == 400  # Bad Request
    assert "Invalid cycle" in response.json()["detail"]
    mock_validator_node.add_received_score.assert_not_awaited()


def test_receive_scores_unknown_submitter(
    test_client: TestClient,
    mock_validator_node: MagicMock,
    key_pair_submitter: PaymentKeyPair,
    # submitter_info: ValidatorInfo, # Không dùng info gốc
    sample_scores: list[ValidatorScore],
):
    """Kiểm tra endpoint khi submitter_uid không có trong validators_info."""
    signing_key = key_pair_submitter.signing_key
    verification_key = key_pair_submitter.verification_key
    # Chuẩn bị signature/VKey hợp lệ
    data_to_sign_str = canonical_json_serialize(sample_scores)
    signature_bytes = signing_key.sign(data_to_sign_str.encode("utf-8"))
    signature_hex = binascii.hexlify(signature_bytes).decode("utf-8")
    vkey_cbor_hex = verification_key.to_cbor_hex()

    payload_dict = {
        "scores": [dataclasses.asdict(s) for s in sample_scores],
        "submitter_validator_uid": "unknown_validator_hex",  # UID không tồn tại
        "cycle": mock_validator_node.current_cycle,
        "submitter_vkey_cbor_hex": vkey_cbor_hex,
        "signature": signature_hex,
    }

    response = test_client.post("/v1/consensus/receive_scores", json=payload_dict)

    assert response.status_code == 400  # Bad Request
    assert "Unknown/inactive submitter" in response.json()["detail"]
    mock_validator_node.add_received_score.assert_not_awaited()


def test_receive_scores_bad_payload(
    test_client: TestClient, mock_validator_node: MagicMock
):
    """Kiểm tra endpoint khi gửi payload sai định dạng."""
    bad_payload = {"wrong_field": "some_value"}

    response = test_client.post("/v1/consensus/receive_scores", json=bad_payload)

    assert response.status_code == 422  # Unprocessable Entity
    mock_validator_node.add_received_score.assert_not_awaited()
