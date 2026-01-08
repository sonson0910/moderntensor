# tests/service/test_register_key.py
import pytest
from sdk.compat.luxtensor_types import (
    Network,
    Address,
    Redeemer,
    BlockFrostChainContext,
    TransactionId,
    UTxO,
    PlutusV3Script,
    ScriptHash,
    PaymentSigningKey,
    StakeSigningKey,  # Thêm các kiểu cần thiết
)
from typing import Tuple  # Thêm Tuple

# Import các thành phần từ SDK
from sdk.metagraph.metagraph_datum import MinerDatum, STATUS_ACTIVE  # Import STATUS_*
from sdk.metagraph.hash.hash_datum import hash_data
from sdk.service.context import get_chain_context
from sdk.smartcontract.validator import read_validator
from sdk.service.register_key import register_key  # Hàm cần test
from sdk.config.settings import settings, logger  # Import settings

# --- Fixtures ---


@pytest.fixture(scope="session")
def chain_context_fixture() -> BlockFrostChainContext:
    """Provides a real blockchain context using BlockFrost."""
    try:
        network = settings.CARDANO_NETWORK
    except AttributeError:
        network = Network.TESTNET
        logger.warning(
            f"Could not read CARDANO_NETWORK from settings, defaulting to {network}"
        )
    if not settings.BLOCKFROST_PROJECT_ID:
        pytest.fail("BLOCKFROST_PROJECT_ID is not set in settings.")
    logger.info(f"Using network: {network} for test session context.")
    return get_chain_context(method="blockfrost")


@pytest.fixture(scope="session")
def validator_details() -> dict:
    """Loads validator script details once per session."""
    validator = read_validator()
    if (
        not validator
        or "script_bytes" not in validator
        or "script_hash" not in validator
    ):
        pytest.fail("Failed to load validator script details.")
    return validator


@pytest.fixture
def script(validator_details) -> PlutusV3Script:
    """Provides the Plutus script bytes."""
    return validator_details["script_bytes"]


@pytest.fixture
def script_hash(validator_details) -> ScriptHash:
    """Provides the script hash."""
    return validator_details["script_hash"]


@pytest.fixture
def contract_address(script_hash) -> Address:
    """Creates the contract address."""
    network = Network.TESTNET
    return Address(payment_part=script_hash, network=network)


@pytest.fixture
def new_datum() -> MinerDatum:
    """Provides new MinerDatum for registration/update, using updated structure."""
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    # Dữ liệu mẫu cho datum mới
    miner_data = {
        "uid": "miner_test_003",  # UID mới để đăng ký
        "subnet_uid": 1,
        "stake": 100_000_000,  # Stake ban đầu khi đăng ký
        "last_performance": 0.5,  # Điểm khởi đầu
        "trust_score": 0.5,  # Điểm khởi đầu
        "accumulated_rewards": 0,
        "last_update_slot": 0,  # Sẽ được cập nhật khi chạy
        "history": [0.1],  # Lịch sử ban đầu rỗng
        "wallet_addr": "addr_test1qpj3f8nhl9da7g8a6pd3vrk2x7k9v7z8x0f9yq8z7x6x0f9yq8z7x6x0f9yq8z7x6x0f9yq8z7x6x0f9yq8z7x6x0f9yq8z7x6",  # Địa chỉ ví mới
        "status": STATUS_ACTIVE,
        "registration_slot": 0,  # Sẽ được cập nhật khi chạy
        "api_endpoint": "http://newminer.example.com:9002",
    }

    history_hash = (
        hash_data(miner_data["history"]) if miner_data["history"] else None
    )  # Handle empty history
    wallet_addr_hash = hash_data(miner_data["wallet_addr"])
    uid_bytes = miner_data["uid"].encode("utf-8")
    api_endpoint_bytes = (
        miner_data["api_endpoint"].encode("utf-8")
        if miner_data["api_endpoint"]
        else None
    )

    datum = MinerDatum(
        uid=uid_bytes,
        subnet_uid=miner_data["subnet_uid"],
        stake=miner_data["stake"],
        scaled_last_performance=int(miner_data["last_performance"] * divisor),
        scaled_trust_score=int(miner_data["trust_score"] * divisor),
        accumulated_rewards=miner_data["accumulated_rewards"],
        last_update_slot=miner_data["last_update_slot"],
        performance_history_hash=history_hash, # type: ignore
        wallet_addr_hash=wallet_addr_hash,
        status=miner_data["status"],
        registration_slot=miner_data["registration_slot"],
        api_endpoint=api_endpoint_bytes, # type: ignore
    )
    logger.debug(f"Created new MinerDatum for registration test: {datum}")
    return datum


# --- Fixture khóa ký (Giả định được định nghĩa trong conftest.py) ---
# @pytest.fixture(scope="session")
# def hotkey_skey_fixture() -> Tuple[PaymentSigningKey, StakeSigningKey]:
#     # ... logic load khóa ký ...
#     pass


# --- Hàm Test ---
@pytest.mark.integration
def test_register_new_key_service(  # Đổi tên để phản ánh rõ hơn mục đích
    chain_context_fixture: BlockFrostChainContext,
    script: PlutusV3Script,
    script_hash: ScriptHash,
    contract_address: Address,
    new_datum: MinerDatum,
    hotkey_skey_fixture,
):
    """
    Tests the register_key function. Assumes register_key finds an appropriate
    UTxO to consume (e.g., one with lowest incentive) and updates it with new_datum.
    """
    payment_xsk, stake_xsk = hotkey_skey_fixture
    if not payment_xsk:
        pytest.fail(
            "Payment signing key (payment_xsk) not found in hotkey_skey_fixture."
        )

    network = Network.TESTNET

    # Define a simple Redeemer (placeholder)
    # WARNING: Redeemer(0) is a placeholder. Verify the actual Redeemer needed
    # by your Plutus script for the registration/update action performed by register_key.
    redeemer = Redeemer(0)
    logger.warning(
        f"Using placeholder Redeemer: {redeemer}. Verify this is correct for your script!"
    )

    logger.info(
        f"Attempting to register/update key using register_key service on {network}..."
    )
    logger.info(f"Target Contract Address: {contract_address}")
    logger.info(f"New Datum to be set: {new_datum}")

    try:
        # Gọi hàm register_key
        # Hàm này cần tự tìm UTXO đầu vào phù hợp tại contract_address
        tx_id_obj = register_key(
            payment_xsk=payment_xsk,
            stake_xsk=stake_xsk,
            script_hash=script_hash,  # register_key có thể cần hash để tìm UTXO?
            new_datum=new_datum,
            script=script,  # Script cần để build transaction
            context=chain_context_fixture,
            network=network,
            contract_address=contract_address,  # Địa chỉ để tìm UTXO # type: ignore
            redeemer=redeemer,
        )
    except Exception as e:
        logger.exception(f"register_key failed with exception: {e}")
        # Thêm thông tin debug nếu cần
        pytest.fail(f"register_key raised an exception: {e}")

    # Xác minh kết quả
    assert tx_id_obj is not None, "Transaction ID object must not be None"
    # assert isinstance(tx_id_obj, TransactionId), f"Expected TransactionId object, got {type(tx_id_obj)}"

    tx_id_hex = str(tx_id_obj)
    logger.info(f"register_key successful. Transaction ID: {tx_id_hex}")
    assert len(tx_id_hex) == 64, "Transaction ID hex string must be 64 characters long"
    logger.info(
        f"Verify transaction on Cardanoscan (Testnet): https://testnet.cardanoscan.io/transaction/{tx_id_hex}"
    )
