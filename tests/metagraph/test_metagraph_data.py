# tests/metagraph/test_metagraph_data.py (Integration Test Style)
import pytest
import asyncio
from typing import List, Dict, Any
from pytest import MonkeyPatch  # Vẫn cần nếu mock một phần (nhưng sẽ không dùng)

from sdk.compat.luxtensor_types import (
    Network,
    Address,
    BlockFrostChainContext,
    UTxO,
    TransactionInput,
    TransactionId,
    TransactionOutput,
    Value,
    Datum,
    RawCBOR,
    ScriptHash,
    PlutusData,
)

# Import các thành phần cần test và các lớp Datum
from sdk.metagraph.metagraph_data import get_all_miner_data, get_all_validator_data
from sdk.metagraph.metagraph_datum import MinerDatum, ValidatorDatum, STATUS_ACTIVE
from sdk.config.settings import settings, logger  # Import settings

DATUM_INT_DIVISOR = settings.METAGRAPH_DATUM_INT_DIVISOR

# Import các hàm tiện ích từ service và smartcontract
from sdk.service.context import get_chain_context
from sdk.smartcontract.validator import read_validator

# --- Fixtures ---


@pytest.fixture(scope="session")
def event_loop():
    """Fixture cần thiết cho các test async."""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


@pytest.fixture(scope="session")
def chain_context_fixture() -> BlockFrostChainContext:
    """Cung cấp context BlockFrost thật kết nối tới mạng lưới trong settings."""
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
    # Trả về context thật
    return get_chain_context(method="blockfrost")


@pytest.fixture(scope="session")
def validator_details() -> dict:
    """Loads validator script details once per session using read_validator."""
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


# Không cần fixture contract_address nữa vì sẽ tạo trong hàm test

# --- Test Cases ---


@pytest.mark.integration  # Đánh dấu là integration test
@pytest.mark.asyncio
async def test_get_all_miner_data_integration(
    chain_context_fixture: BlockFrostChainContext,
    script_hash: ScriptHash,
    network: Network,
):
    """
    Kiểm tra hàm get_all_miner_data bằng cách gọi lên blockchain thật.
    Xác minh cấu trúc dữ liệu trả về nếu tìm thấy UTXO.
    """
    logger.info(f"Running test_get_all_miner_data_integration on network {network}...")

    try:
        # Gọi hàm cần test với context và script_hash thật
        miner_data_list = await get_all_miner_data(
            context=chain_context_fixture, script_hash=script_hash, network=network
        )
    except Exception as e:
        logger.exception(f"get_all_miner_data raised an exception: {e}")
        pytest.fail(f"get_all_miner_data failed: {e}")

    # Kiểm tra kiểu trả về
    assert isinstance(miner_data_list, list), "Result should be a list"
    logger.info(f"Found {len(miner_data_list)} MinerDatum UTxOs.")

    # Nếu có kết quả, kiểm tra cấu trúc của phần tử đầu tiên
    if miner_data_list:
        logger.info("Checking structure of the first returned item...")
        item1 = miner_data_list[0]
        logger.debug(f"First item: {item1}")

        assert isinstance(item1, dict), "Each item should be a dictionary"
        assert (
            "tx_id" in item1
            and isinstance(item1["tx_id"], str) # type: ignore
            and len(item1["tx_id"]) == 64 # type: ignore
        )
        assert "index" in item1 and isinstance(item1["index"], int) # type: ignore
        assert "amount" in item1 and isinstance(item1["amount"], int) # type: ignore
        assert "datum" in item1 and isinstance(item1["datum"], dict) # type: ignore

        datum_dict = item1["datum"] # type: ignore
        # Kiểm tra các key quan trọng trong datum (dựa trên MinerDatum đã cập nhật)
        expected_datum_keys = [
            "uid",
            "subnet_uid",
            "stake",
            "last_performance",
            "trust_score",
            "accumulated_rewards",
            "last_update_slot",
            "performance_history_hash",
            "wallet_addr_hash",
            "status",
            "registration_slot",
            "api_endpoint",
        ]
        for key in expected_datum_keys:
            assert key in datum_dict, f"Datum dictionary missing key: {key}"

        # Kiểm tra kiểu dữ liệu cơ bản của một số trường datum
        assert isinstance(datum_dict["uid"], str)  # Đã decode hex
        assert isinstance(datum_dict["subnet_uid"], int)
        assert isinstance(datum_dict["stake"], int)
        assert isinstance(datum_dict["last_performance"], float)  # Đã unscale
        assert isinstance(datum_dict["trust_score"], float)  # Đã unscale
        assert isinstance(datum_dict["status"], int)
        assert isinstance(datum_dict["version"], int)
        assert datum_dict["api_endpoint"] is None or isinstance(
            datum_dict["api_endpoint"], str
        )  # Đã decode
        assert datum_dict["performance_history_hash"] is None or isinstance(
            datum_dict["performance_history_hash"], str
        )  # Đã hex
        logger.info("Structure of the first MinerDatum item looks OK.")
    else:
        # Nếu không tìm thấy UTXO, test vẫn pass nhưng có cảnh báo
        contract_address = Address(payment_part=script_hash, network=network)
        logger.warning(
            f"No MinerDatum UTxOs were found at {contract_address}. Test passes but validation was limited."
        )


@pytest.mark.integration  # Đánh dấu là integration test
@pytest.mark.asyncio
async def test_get_all_validator_data_integration(
    chain_context_fixture: BlockFrostChainContext,
    script_hash: ScriptHash,
    network: Network,
):
    """
    Kiểm tra hàm get_all_validator_data bằng cách gọi lên blockchain thật.
    Xác minh cấu trúc dữ liệu trả về nếu tìm thấy UTXO.
    """
    logger.info(
        f"Running test_get_all_validator_data_integration on network {network}..."
    )

    try:
        # Gọi hàm cần test với context và script_hash thật
        validator_data_list = await get_all_validator_data(
            context=chain_context_fixture, script_hash=script_hash, network=network
        )
    except Exception as e:
        logger.exception(f"get_all_validator_data raised an exception: {e}")
        pytest.fail(f"get_all_validator_data failed: {e}")

    # Kiểm tra kiểu trả về
    assert isinstance(validator_data_list, list), "Result should be a list"
    logger.info(f"Found {len(validator_data_list)} ValidatorDatum UTxOs.")

    # Nếu có kết quả, kiểm tra cấu trúc của phần tử đầu tiên
    if validator_data_list:
        logger.info("Checking structure of the first returned item...")
        item1 = validator_data_list[0]
        logger.debug(f"First item: {item1}")

        assert isinstance(item1, dict), "Each item should be a dictionary"
        assert (
            "tx_id" in item1
            and isinstance(item1["tx_id"], str) # type: ignore
            and len(item1["tx_id"]) == 64 # type: ignore
        )
        # ... (kiểm tra các key khác của UTXO info) ...
        assert "datum" in item1 and isinstance(item1["datum"], dict) # type: ignore

        datum_dict = item1["datum"] # type: ignore
        # Kiểm tra các key quan trọng trong datum (dựa trên ValidatorDatum đã cập nhật)
        expected_datum_keys = [
            "uid",
            "subnet_uid",
            "stake",
            "last_performance",
            "trust_score",
            "accumulated_rewards",
            "last_update_slot",
            "performance_history_hash",
            "wallet_addr_hash",
            "status",
            "registration_slot",
            "api_endpoint",
        ]
        for key in expected_datum_keys:
            assert key in datum_dict, f"Datum dictionary missing key: {key}"
        # ... (Kiểm tra kiểu dữ liệu tương tự như test miner) ...
        logger.info("Structure of the first ValidatorDatum item looks OK.")
    else:
        contract_address = Address(payment_part=script_hash, network=network)
        logger.warning(
            f"No ValidatorDatum UTxOs were found at {contract_address}. Test passes but validation was limited."
        )
