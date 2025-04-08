# tests/metagraph/test_update_metagraph.py
import pytest
from pycardano import (
    TransactionId, Network, BlockFrostChainContext, Address, Redeemer
)

# Import các thành phần đã cập nhật
from sdk.metagraph.metagraph_datum import MinerDatum, STATUS_ACTIVE # Import STATUS_*
from sdk.metagraph.hash.hash_datum import hash_data
from sdk.metagraph.hash.verify_hash import verify_hash
from sdk.metagraph.update_metagraph import update_datum # Hàm cần test
from sdk.service.context import get_chain_context
from sdk.smartcontract.validator import read_validator
from sdk.service.utxos import get_utxo_from_str # Hàm tìm UTXO đầu vào
from sdk.config.settings import settings, logger # Import settings

# Fixture để cung cấp context (giữ nguyên)
@pytest.fixture(scope="session")
def chain_context_fixture():
    """Provides a BlockFrostChainContext for the test session."""
    try:
        network = settings.CARDANO_NETWORK
    except AttributeError:
        network = Network.TESTNET
    logger.info(f"Using network: {network} for test session context.")
    return get_chain_context(method="blockfrost")

# Fixture lấy UTXO đầu vào để cập nhật
@pytest.fixture
def mock_utxo(chain_context_fixture):
    """
    Lấy một UTxO hiện có tại địa chỉ contract để làm đầu vào cho việc cập nhật datum.
    *** Yêu cầu: Phải có ít nhất một UTxO chứa MinerDatum hợp lệ tại địa chỉ contract trên Testnet ***
    """
    logger.info("Attempting to find an existing UTxO at the contract address...")
    validator = read_validator()
    if not validator or "script_hash" not in validator:
         pytest.fail("Failed to load validator script hash.")
    script_hash = validator["script_hash"]
    contract_address = Address(payment_part=script_hash, network=Network.TESTNET)

    # Sử dụng hàm get_utxo_from_str để tìm UTXO đầu tiên có MinerDatum
    # Hàm này cần được đảm bảo hoạt động đúng với MinerDatum mới
    utxo = get_utxo_from_str(
        contract_address=contract_address,
        datumclass=MinerDatum, # Hàm get_utxo_from_str cần parse được MinerDatum mới
        context=chain_context_fixture,
    )
    print()

    # Quan trọng: Kiểm tra xem có tìm thấy UTXO không
    assert utxo is not None, (
        f"No UTxO with MinerDatum found at contract address {contract_address}. "
        f"Ensure at least one UTxO exists from 'test_create_utxo.py' or manual setup."
    )
    logger.info(f"Found UTxO to update: {utxo.input}")
    # logger.debug(f"Input UTxO details: {utxo}") # In chi tiết nếu cần debug
    return utxo

# Fixture cung cấp dữ liệu miner mẫu cho trạng thái "mới"
@pytest.fixture
def miner_data_for_update():
    """Provides sample raw data representing the *new* state for the miner."""
    return {
        "uid": "miner_test_002", # Giữ nguyên UID của UTXO tìm được hoặc dùng UID cụ thể bạn muốn update
        "subnet_uid": 1,
        "stake": 650_000_000, # Stake đã thay đổi
        "last_performance": 0.98, # Performance mới
        "trust_score": 0.90, # Trust score mới
        "accumulated_rewards": 280_000_000, # Rewards mới
        "last_update_slot": 120000, # Slot mới
        "history": [0.80, 0.85, 0.95, 0.98], # Thêm dữ liệu lịch sử mới
        "wallet_addr": "addr_test1qzkxr3kpzex93m646qr7w82d56md2kchtsv9jy39dykn4cmcxuuneyeqhdc4wy7de9mk54fndmckahxwqtwy3qg8pumsabcde", # Giữ nguyên hoặc thay đổi nếu cần
        "status": STATUS_ACTIVE,
        "registration_slot": 55000, # Thường không đổi
        "api_endpoint": "http://miner2-updated.example.com:9001", # Endpoint mới
    }

# Fixture tạo đối tượng MinerDatum "mới" để cập nhật
@pytest.fixture
def new_datum(miner_data_for_update):
    """Creates the new MinerDatum object for the update operation."""
    divisor = settings.METAGRAPH_DATUM_INT_DIVISOR

    history_hash = hash_data(miner_data_for_update["history"])
    wallet_addr_hash = hash_data(miner_data_for_update["wallet_addr"])
    uid_bytes = miner_data_for_update["uid"].encode('utf-8')
    api_endpoint_bytes = miner_data_for_update["api_endpoint"].encode('utf-8') if miner_data_for_update["api_endpoint"] else None

    # Tạo đối tượng MinerDatum mới
    datum = MinerDatum(
        uid=uid_bytes,
        subnet_uid=miner_data_for_update["subnet_uid"],
        stake=miner_data_for_update["stake"],
        scaled_last_performance=int(miner_data_for_update["last_performance"] * divisor),
        scaled_trust_score=int(miner_data_for_update["trust_score"] * divisor),
        accumulated_rewards=miner_data_for_update["accumulated_rewards"],
        last_update_slot=miner_data_for_update["last_update_slot"],
        performance_history_hash=history_hash,
        wallet_addr_hash=wallet_addr_hash,
        status=miner_data_for_update["status"],
        registration_slot=miner_data_for_update["registration_slot"],
        api_endpoint=api_endpoint_bytes,
    )
    logger.debug(f"Created new MinerDatum for update: {datum}")
    return datum

# Hàm test chính
@pytest.mark.integration
def test_update_datum(chain_context_fixture, mock_utxo, new_datum, hotkey_skey_fixture):
    """
    Tests the update_datum function by updating the datum of the mock_utxo.
    Verifies successful transaction submission.
    """
    # Unpack signing keys
    payment_xsk, stake_xsk = hotkey_skey_fixture
    if not payment_xsk:
         pytest.fail("Payment signing key (payment_xsk) not found in hotkey_skey_fixture.")

    # Set network from settings
    network = Network.TESTNET

    # Load validator script details
    validator = read_validator()
    if not validator or "script_bytes" not in validator or "script_hash" not in validator:
        pytest.fail("Failed to load validator script details.")
    script = validator["script_bytes"]
    script_hash = validator["script_hash"]

    # Define a simple Redeemer (placeholder, might need specific data based on script logic)
    # Redeemer(0) thường dùng cho các script không cần kiểm tra dữ liệu Redeemer phức tạp.
    redeemer = Redeemer(0)
    logger.info(f"Using Redeemer: {redeemer}")

    logger.info(f"Attempting to update UTxO {mock_utxo.input} at script {script_hash} on {network}...")
    logger.info(f"Updating to new Datum: {new_datum}")

    # Call the update_datum function
    try:
        tx_id_obj = update_datum(
            payment_xsk=payment_xsk,
            stake_xsk=stake_xsk, # Có thể là None
            # 'into' parameter is deprecated or renamed? Assuming script_hash is implicitly used via UTxO output.
            # Check the signature of update_datum function in update_metagraph.py
            # If 'into' is required: into=script_hash,
            script_hash=script_hash,
            utxo=mock_utxo,          # UTXO đầu vào để tiêu thụ
            script=script,          # Plutus script để validate input
            new_datum=new_datum,    # Datum mới cho output
            redeemer=redeemer,      # Redeemer để unlock input UTXO <<<--- Thêm Redeemer
            context=chain_context_fixture,
            network=network,
        )
    except Exception as e:
        logger.exception(f"update_datum failed with exception: {e}")
        pytest.fail(f"update_datum raised an exception: {e}")

    # Verify the result (TransactionId object)
    assert tx_id_obj is not None, "Transaction ID object should not be None"
    # assert isinstance(tx_id_obj, TransactionId), f"Expected TransactionId, got {type(tx_id_obj)}"

    tx_id_hex = str(tx_id_obj)
    logger.info(f"Datum update successful. Transaction ID: {tx_id_hex}")
    assert len(tx_id_hex) == 64, "Transaction ID hex should be 64 characters long"

