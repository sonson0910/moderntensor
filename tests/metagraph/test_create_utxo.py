# tests/metagraph/test_create_utxo.py
import pytest
from sdk.compat.pycardano import (
    TransactionId,
    Network,
    BlockFrostChainContext,
    PaymentSigningKey,
    StakeSigningKey,
    ScriptHash,
    Address,  # Thêm các import cần thiết
)
from typing import Tuple, Optional  # Thêm Tuple

# Import các thành phần đã cập nhật
# Đảm bảo import cả ValidatorDatum và các hằng số status nếu cần
from sdk.metagraph.metagraph_datum import MinerDatum, ValidatorDatum, STATUS_ACTIVE
from sdk.metagraph.hash.hash_datum import hash_data
from sdk.metagraph.hash.verify_hash import verify_hash
from sdk.metagraph.create_utxo import create_utxo
from sdk.service.context import get_chain_context
from sdk.smartcontract.validator import read_validator
from sdk.config.settings import settings, logger  # Import settings

# --- Fixtures ---


@pytest.fixture(scope="session")
def chain_context_fixture() -> BlockFrostChainContext:
    """Provides a BlockFrostChainContext for the test session."""
    try:
        network = settings.CARDANO_NETWORK
    except AttributeError:
        network = Network.TESTNET  # Mặc định nếu settings load lỗi
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
    if not validator or "script_hash" not in validator:
        pytest.fail("Failed to load validator script details from read_validator().")
    logger.info(f"Loaded validator script hash: {validator['script_hash']}")
    return validator


@pytest.fixture(scope="session")
def script_hash(validator_details) -> ScriptHash:
    """Provides the actual script hash."""
    return validator_details["script_hash"]


@pytest.fixture(scope="session")
def network() -> Network:
    """Provides the network from settings."""
    return Network.TESTNET


# --- Fixtures cho Miner Datum ---


@pytest.fixture
def miner_data():
    """Provides sample raw data for a miner, updated for new fields."""
    return {
        "uid": "miner_test_002",
        "subnet_uid": 1,
        "stake": 600_000_000,
        "last_performance": 0.95,
        "trust_score": 0.88,
        "accumulated_rewards": 250_000_000,
        "last_update_slot": 110000,
        "history": [0.80, 0.85, 0.95],
        "wallet_addr": "addr_test1qzkxr3kpzex93m646qr7w82d56md2kchtsv9jy39dykn4cmcxuuneyeqhdc4wy7de9mk54fndmckahxwqtwy3qg8pumsabcde",
        "status": STATUS_ACTIVE,
        "registration_slot": 55000,
        "api_endpoint": "http://miner2.example.com:9000",
    }


@pytest.fixture
def miner_datum(miner_data):
    """Creates an updated MinerDatum object with scaled, hashed, and encoded fields."""
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    history_hash = hash_data(miner_data["history"])
    wallet_addr_hash = hash_data(miner_data["wallet_addr"])
    uid_bytes = miner_data["uid"].encode("utf-8")
    api_endpoint_bytes = (
        miner_data["api_endpoint"].encode("utf-8")
        if miner_data["api_endpoint"]
        else None
    )

    return MinerDatum(
        uid=uid_bytes,
        subnet_uid=miner_data["subnet_uid"],
        stake=miner_data["stake"],
        scaled_last_performance=int(miner_data["last_performance"] * divisor),
        scaled_trust_score=int(miner_data["trust_score"] * divisor),
        accumulated_rewards=miner_data["accumulated_rewards"],
        last_update_slot=miner_data["last_update_slot"],
        performance_history_hash=history_hash,
        wallet_addr_hash=wallet_addr_hash,
        status=miner_data["status"],
        registration_slot=miner_data["registration_slot"],
        api_endpoint=api_endpoint_bytes,  # type: ignore
    )


# --- Fixtures cho Validator Datum ---


@pytest.fixture
def validator_data():
    """Provides sample raw data for a validator."""
    return {
        "uid": "validator_test_001",
        "subnet_uid": 0,  # Ví dụ validator hoạt động ở subnet 0 (toàn cục)
        "stake": 1_500_000_000,  # 1500 ADA
        "last_performance": 0.99,  # E_v
        "trust_score": 0.96,
        "accumulated_rewards": 50_000_000,
        "last_update_slot": 115000,
        "history": [0.95, 0.97, 0.99],  # Lịch sử hiệu suất của validator
        "wallet_addr": "addr_test1qrjvm22mvsht5svwd7pswhk546g6z2t8j0vhyhrj86lq7asc0z0xyz",
        "status": STATUS_ACTIVE,
        "registration_slot": 40000,
        "api_endpoint": "http://validator1.example.com:8000",
    }


@pytest.fixture
def validator_datum(validator_data):
    """Creates a ValidatorDatum object with scaled, hashed, and encoded fields."""
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR
    history_hash = hash_data(validator_data["history"])
    wallet_addr_hash = hash_data(validator_data["wallet_addr"])
    uid_bytes = validator_data["uid"].encode("utf-8")  # Đảm bảo UID là bytes
    api_endpoint_bytes = (
        validator_data["api_endpoint"].encode("utf-8")
        if validator_data["api_endpoint"]
        else None
    )

    return ValidatorDatum(
        uid=uid_bytes,
        subnet_uid=validator_data["subnet_uid"],
        stake=validator_data["stake"],
        scaled_last_performance=int(validator_data["last_performance"] * divisor),
        scaled_trust_score=int(validator_data["trust_score"] * divisor),
        accumulated_rewards=validator_data["accumulated_rewards"],
        last_update_slot=validator_data["last_update_slot"],
        performance_history_hash=history_hash,
        wallet_addr_hash=wallet_addr_hash,
        status=validator_data["status"],
        registration_slot=validator_data["registration_slot"],
        api_endpoint=api_endpoint_bytes,  # type: ignore
    )


# --- Fixture khóa ký (Giả định được định nghĩa trong conftest.py) ---
# @pytest.fixture(scope="session")
# def hotkey_skey_fixture() -> Tuple[PaymentSigningKey, Optional[StakeSigningKey]]:
#     # ... logic load khóa ký ...
#     pass

# --- Hàm Test ---


@pytest.mark.integration
def test_create_miner_utxo(
    chain_context_fixture: BlockFrostChainContext,
    miner_datum: MinerDatum,
    miner_data: dict,
    hotkey_skey_fixture: Tuple[PaymentSigningKey, Optional[StakeSigningKey]],
    script_hash: ScriptHash,
    network: Network,
):
    """
    Tests creating a UTxO with MinerDatum.
    """
    payment_xsk, stake_xsk = hotkey_skey_fixture
    amount = 2_000_000  # 2 ADA

    logger.info(
        f"[Miner Test] Attempting to create UTxO at script {script_hash} on {network}..."
    )
    logger.info(f"[Miner Test] Using Datum: {miner_datum}")

    try:
        tx_id_obj = create_utxo(
            payment_xsk=payment_xsk, # type: ignore
            stake_xsk=stake_xsk, # type: ignore
            amount=amount,  # type: ignore
            script_hash=script_hash,
            datum=miner_datum,
            context=chain_context_fixture,
            network=network,
        )
    except Exception as e:
        logger.exception(f"[Miner Test] create_utxo failed: {e}")
        pytest.fail(f"create_utxo for MinerDatum raised an exception: {e}")

    assert (
        tx_id_obj is not None
    ), "[Miner Test] Transaction ID object should not be None"
    # assert isinstance(tx_id_obj, TransactionId), f"[Miner Test] Expected TransactionId, got {type(tx_id_obj)}"
    tx_id_hex = str(tx_id_obj)
    logger.info(f"[Miner Test] UTxO creation successful. Transaction ID: {tx_id_hex}")
    assert (
        len(tx_id_hex) == 64
    ), "[Miner Test] Transaction ID hex should be 64 characters long"


@pytest.mark.integration
def test_create_validator_utxo(
    chain_context_fixture: BlockFrostChainContext,
    validator_datum: ValidatorDatum,
    validator_data: dict,
    hotkey_skey_fixture: Tuple[PaymentSigningKey, Optional[StakeSigningKey]],
    script_hash: ScriptHash,
    network: Network,
):
    """
    Tests creating a UTxO with ValidatorDatum.
    """
    payment_xsk, stake_xsk = hotkey_skey_fixture
    amount = 2_100_000  # 2.1 ADA (dùng lượng khác để phân biệt)

    logger.info(
        f"[Validator Test] Attempting to create UTxO at script {script_hash} on {network}..."
    )
    logger.info(f"[Validator Test] Using Datum: {validator_datum}")

    try:
        tx_id_obj = create_utxo(
            payment_xsk=payment_xsk, # type: ignore
            stake_xsk=stake_xsk, # type: ignore
            amount=amount,  # type: ignore
            script_hash=script_hash,
            datum=validator_datum,  # <<<--- Sử dụng validator_datum
            context=chain_context_fixture,
            network=network,
        )
    except Exception as e:
        logger.exception(f"[Validator Test] create_utxo failed: {e}")
        pytest.fail(f"create_utxo for ValidatorDatum raised an exception: {e}")

    assert (
        tx_id_obj is not None
    ), "[Validator Test] Transaction ID object should not be None"
    # assert isinstance(tx_id_obj, TransactionId), f"[Validator Test] Expected TransactionId, got {type(tx_id_obj)}"
    tx_id_hex = str(tx_id_obj)
    logger.info(
        f"[Validator Test] UTxO creation successful. Transaction ID: {tx_id_hex}"
    )
    assert (
        len(tx_id_hex) == 64
    ), "[Validator Test] Transaction ID hex should be 64 characters long"

    # Optional: Verify hash integrity for validator data (if needed)
    # logger.info("[Validator Test] Verifying data hashes...")
    # assert verify_hash(validator_data["history"], validator_datum.performance_history_hash), "History hash does not match"
    # assert verify_hash(validator_data["wallet_addr"], validator_datum.wallet_addr_hash), "Wallet address hash does not match"
    # logger.info("[Validator Test] Data hashes verified successfully.")
