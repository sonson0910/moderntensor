import pytest
import asyncio
import time  # For potential delays
import os

from pycardano import (
    Address,
    Network,
    Value,
    MultiAsset,
    AssetName,
    ScriptHash,
    PlutusV3Script,
    TransactionOutput,
    TransactionId,
    VerificationKeyHash,
    # Redeemer, # Maybe not needed directly for assertion
    # RawPlutusData, # Maybe not needed directly for assertion
    # ExecutionUnits, # Maybe not needed directly for assertion
    TransactionInput,
    UTxO,
    BlockFrostChainContext,  # Need the real context
)

# No longer need Transaction or ExtendedSigningKey here if fixture provides them
# from pycardano.transaction import Transaction
# from pycardano.hdwallet import ExtendedSigningKey # type: ignore
# from pycardano.plutus import RedeemerTag # Maybe not needed directly for assertion

# Function under test
from sdk.smartcontract.minting import mint_native_tokens, script_hash
from sdk.config.settings import settings, logger
from sdk.service.context import (
    get_chain_context,
)  # Import context getter like in the other test


# --- Fixture for Chain Context (can be moved to conftest.py) ---
@pytest.fixture(scope="session")
def chain_context_fixture():
    """
    Creates a real BlockFrost chain_context for Cardano TESTNET.
    """
    logger.info("Setting up BlockFrost Chain Context for integration tests...")
    try:
        context = get_chain_context(method="blockfrost")
        # Removed latest_block check due to persistent linter issues
        # # Verify connection by fetching latest block
        # # type: ignore # Linter cannot resolve attribute
        # latest_block = context.latest_block()
        # logger.info(f"BlockFrost context connected. Latest block: {latest_block.number}")
        logger.info("BlockFrost context initialized (skipping latest_block check).")
        return context
    except Exception as e:
        logger.error(f"Failed to initialize BlockFrost context: {e}")
        pytest.fail(f"Failed to initialize BlockFrost context: {e}")


# --- Get Policy Script from Settings (remains the same) ---
POLICY_SCRIPT_CBOR_HEX = settings.MINTING_SCRIPT_CBOR_HEX
if not POLICY_SCRIPT_CBOR_HEX:
    pytest.skip(
        "MINTING_SCRIPT_CBOR_HEX not found in settings, skipping integration test",
        allow_module_level=True,
    )
try:
    POLICY_SCRIPT = PlutusV3Script(bytes.fromhex(POLICY_SCRIPT_CBOR_HEX))
    POLICY_ID = script_hash(POLICY_SCRIPT)
    POLICY_ID_HEX = POLICY_ID.to_primitive().hex()
except Exception as e:
    pytest.skip(
        f"Invalid MINTING_SCRIPT_CBOR_HEX in settings: {e}", allow_module_level=True
    )

TEST_ASSET_NAME_STR = "MOD"  # Or use a name from settings if preferred
TEST_ASSET_NAME_BYTES = TEST_ASSET_NAME_STR.encode("utf-8")
TEST_ASSET_NAME = AssetName(TEST_ASSET_NAME_BYTES)
AMOUNT_TO_MINT = 5000
# DUMMY_TX_ID = TransactionId(bytes([0]*32)) # No longer needed
# DUMMY_MIN_LOVELACE = 1_500_000 # Use real min_lovelace

# --- Test Cases ---


@pytest.mark.integration  # Mark as integration test
@pytest.mark.asyncio
async def test_mint_native_tokens_integration(
    chain_context_fixture: BlockFrostChainContext, hotkey_skey_fixture
):
    """
    Integration Test: Mints tokens using the real minting function on the testnet.
    Sends tokens back to the minter's address.
    Requires a configured Blockfrost API key and funded testnet address.
    """
    logger.info(
        f"Starting integration test: Mint {AMOUNT_TO_MINT} of '{TEST_ASSET_NAME_STR}' ({POLICY_ID_HEX}) using Plutus V3 script."
    )

    # 1. Get Context and Keys from fixtures
    context = chain_context_fixture
    network = context.network
    (payment_skey, stake_skey) = hotkey_skey_fixture

    # 2. Derive Address
    payment_vkey = payment_skey.to_verification_key()
    payment_pkh = payment_vkey.hash()
    stake_pkh = stake_skey.to_verification_key().hash() if stake_skey else None
    minter_address = Address(
        payment_part=payment_pkh, staking_part=stake_pkh, network=network
    )
    logger.info(f"Minter Address: {minter_address}")

    # 3. Check for UTxOs at the address
    logger.info(f"Checking UTxOs for {minter_address}...")
    try:
        utxos = context.utxos(str(minter_address))
        if not utxos:
            logger.warning(f"Skipping test: No UTxOs found at {minter_address}")
            pytest.skip(f"No UTxOs found at {minter_address}")
        else:
            # Optional: Log balance for debugging
            total_balance = sum(utxo.output.amount.coin for utxo in utxos)
            logger.info(
                f"Found {len(utxos)} UTxOs with total balance: {total_balance / 1_000_000} ADA"
            )
            if total_balance < 2_000_000:  # Basic check for min ADA + fees
                logger.warning(
                    f"Skipping test: Insufficient balance ({total_balance} lovelace) at {minter_address}"
                )
                pytest.skip(f"Insufficient balance at {minter_address}")
    except Exception as e:
        logger.error(f"Failed to fetch UTxOs: {e}")
        pytest.fail(f"Failed to fetch UTxOs for {minter_address}: {e}")

    # 4. Call the function under test (NO MOCKS/PATCHES)
    logger.info("Calling mint_native_tokens function...")
    try:
        result_tx_id = await mint_native_tokens(
            context=context,  # Real context
            signing_key=payment_skey,
            stake_signing_key=stake_skey,
            policy_script_cbor_hex=POLICY_SCRIPT_CBOR_HEX,  # Real script
            asset_name_str=TEST_ASSET_NAME_STR,
            amount_to_mint=AMOUNT_TO_MINT,
            recipient_address=None,  # Send back to minter
            network=network,  # Real network
        )
        logger.info(f"mint_native_tokens returned: {result_tx_id}")

        # 5. Assertions
        assert (
            result_tx_id is not None
        ), "Minting function returned None, indicating an error during execution."
        assert isinstance(
            result_tx_id, TransactionId
        ), f"Expected TransactionId, got {type(result_tx_id)}"
        assert (
            len(result_tx_id.payload.hex()) == 64
        ), "Returned TransactionId has incorrect length."
        logger.info(
            f":white_check_mark: Minting transaction submitted successfully! TxID: [yellow]{result_tx_id}[/yellow]"
        )
        # Optional: Add a delay and check explorer, but might make test slow/flaky
        # time.sleep(60)
        # logger.info(f"Check explorer: https://preprod.cardanoscan.io/transaction/{result_tx_id}")

    except Exception as e:
        logger.exception(
            f":rotating_light: mint_native_tokens raised an unexpected exception: {e}"
        )
        pytest.fail(f"mint_native_tokens raised an unexpected exception: {e}")


# --- TODO: Add more integration test cases ---
# - Test with a specific recipient_address provided
# - Test minting a different amount
# - Test potential failure cases (e.g., invalid script, insufficient funds if check is removed)
