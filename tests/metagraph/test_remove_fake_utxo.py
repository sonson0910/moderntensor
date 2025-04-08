# tests/metagraph/test_remove_fake_utxo.py
from dataclasses import dataclass
import pytest
from pycardano import (
    Address,
    Network,
    PlutusData,
    Redeemer, # <<<--- Thêm Redeemer
    TransactionId
)
from typing import List # <<<--- Thêm List
from pycardano import UTxO # <<<--- Thêm UTxO

# Import các thành phần cần thiết từ SDK
from sdk.metagraph.remove_fake_utxo import remove_fake_utxos # Hàm cần test
from sdk.service.context import get_chain_context
from sdk.smartcontract.validator import read_validator
from sdk.config.settings import settings, logger # <<<--- Import settings

# --- Xóa Mock HelloWorldDatum không cần thiết ---

# Fixture cho blockchain context
@pytest.fixture(scope="session")
def chain_context_fixture():
    """Provides a blockchain context for the Cardano TESTNET using BlockFrost."""
    try:
        network = settings.CARDANO_NETWORK
    except AttributeError:
        network = Network.TESTNET # Mặc định nếu settings load lỗi
        logger.warning(f"Could not read CARDANO_NETWORK from settings, defaulting to {network}")

    logger.info(f"Using network: {network} for test session context.")
    # Đảm bảo project_id được truyền vào
    if not settings.BLOCKFROST_PROJECT_ID:
         pytest.fail("BLOCKFROST_PROJECT_ID is not set in settings.")
    return get_chain_context(method="blockfrost")


# Fixture lấy TẤT CẢ UTxOs tại địa chỉ contract
@pytest.fixture
def utxos_at_contract_address(chain_context_fixture) -> List[UTxO]: # <<<--- Đổi tên và thêm kiểu trả về
    """
    Lấy *tất cả* UTxOs hiện có tại địa chỉ Plutus contract.
    Lưu ý: Hàm remove_fake_utxos sẽ cố gắng tiêu thụ tất cả các UTxO này.
    """
    logger.info("Attempting to find ALL UTxOs at the contract address...")
    validator = read_validator()
    if not validator or "script_hash" not in validator:
         pytest.fail("Failed to load validator script hash.")

    script_hash = validator["script_hash"]
    network = Network.TESTNET
    contract_address = Address(payment_part=script_hash, network=network)
    logger.info(f"Querying UTxOs for address: {contract_address}")

    try:
        # Gọi API của chain context để lấy UTxOs
        utxos = chain_context_fixture.utxos(str(contract_address))
    except Exception as e:
        logger.exception(f"Failed to fetch UTxOs from address {contract_address}: {e}")
        pytest.fail(f"Failed to fetch UTxOs: {e}")

    # Quan trọng: Kiểm tra xem có tìm thấy UTXO nào không
    assert utxos, (
        f"No UTxOs found at contract address {contract_address}. "
        f"Cannot proceed with remove_fake_utxos test. Ensure UTxOs exist."
    )
    logger.info(f"Found {len(utxos)} UTxO(s) at contract address.")
    # logger.debug(f"UTxOs found: {utxos}") # In chi tiết nếu cần debug
    return utxos


# Fixture cho Plutus contract script
@pytest.fixture
def script():
    """Provides the Plutus contract script bytes."""
    validator = read_validator()
    if not validator or "script_bytes" not in validator:
         pytest.fail("Failed to load validator script bytes.")
    return validator["script_bytes"]


# Hàm test chính
@pytest.mark.integration
def test_remove_fake_utxos(chain_context_fixture, utxos_at_contract_address, script, hotkey_skey_fixture):
    """
    Tests the remove_fake_utxos function by attempting to remove all UTxOs found at the contract address.

    Args:
        chain_context_fixture: The blockchain context fixture.
        utxos_at_contract_address: List of UTxOs found at the contract address.
        script: The Plutus contract script.
        hotkey_skey_fixture: A tuple containing payment and stake signing keys.

    Asserts:
        - The transaction ID object is not None.
        - The transaction ID object is an instance of TransactionId.
        - The transaction ID hex string is 64 characters long.
    """
    # Unpack signing keys
    payment_xsk, stake_xsk = hotkey_skey_fixture
    if not payment_xsk:
         pytest.fail("Payment signing key (payment_xsk) not found in hotkey_skey_fixture.")

    # Define network from settings
    network = Network.TESTNET

    # Define a simple Redeemer (placeholder)
    # WARNING: Redeemer(0) might not be valid for spending all UTxOs depending on script logic.
    # This test assumes the script allows spending these UTxOs with Redeemer(0).
    redeemer = Redeemer(0)
    logger.info(f"Using Redeemer: {redeemer} to attempt removal.")
    logger.warning(f"Attempting to remove {len(utxos_at_contract_address)} UTxOs found at the contract address.")

    # Execute the function to remove UTxOs
    try:
        tx_id_obj = remove_fake_utxos(
            payment_xsk=payment_xsk,
            stake_xsk=stake_xsk, # Can be None
            fake_utxos=utxos_at_contract_address, # Pass the list of found UTxOs
            script=script,
            context=chain_context_fixture,
            network=network,
        )
    except Exception as e:
        # Log chi tiết lỗi nếu hàm raise exception
        logger.exception(f"remove_fake_utxos failed with exception: {e}")
        # In thông tin UTXO để debug
        logger.error(f"Attempted to remove UTxOs: {utxos_at_contract_address}")
        pytest.fail(f"remove_fake_utxos raised an exception: {e}")

    # Validate the transaction ID
    assert tx_id_obj is not None, "Transaction ID object must not be None"
    # assert isinstance(tx_id_obj, TransactionId), f"Expected TransactionId object, got {type(tx_id_obj)}"

    tx_id_hex = str(tx_id_obj)
    logger.info(f"Removal transaction submitted successfully. Transaction ID: {tx_id_hex}")
    assert len(tx_id_hex) == 64, "Transaction ID hex string must be 64 characters long"

