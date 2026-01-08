# tests/service/test_send_token.py

import os
import time
import pytest
from sdk.compat.luxtensor_types import Network

from sdk.config.settings import settings, logger  # Global logger & settings
from sdk.service.context import get_chain_context
from sdk.service.tx_service import send_token


@pytest.fixture(scope="session")
def chain_context_fixture():
    """
    Creates a BlockFrost-based chain context for Cardano TESTNET.

    Steps:
      1) Reads BLOCKFROST_PROJECT_ID from environment or from settings if applicable.
      2) Calls get_chain_context(...) with method="blockfrost", passing the project_id,
         and sets network=TESTNET.
      3) Returns the chain context fixture for building/submitting transactions.
    """
    project_id = os.getenv(
        "BLOCKFROST_PROJECT_ID", "preprod06dzhzKlynuTInzvxHDH5cXbdHo524DE"
    )
    return get_chain_context(method="blockfrost")


@pytest.mark.integration
def test_send_michielcoin(chain_context_fixture, hotkey_skey_fixture):
    """
    Integration test to send 50 "MichielCOIN" (MIT) tokens using send_token(...)
    from a hotkey-based address to a receiver address.

    Prerequisites:
      - "MichielCOIN" token is already minted under a known policy_id.
      - The environment variables define TEST_RECEIVER_ADDRESS and TEST_POLICY_ID_HEX.
      - The hotkey fixture address must have enough ADA + token balances for the transfer.

    Steps:
      1) Read the receiver address from environment. If not present, skip.
      2) Read the policy_id_hex from environment. If not present, skip.
      3) Gather base_dir, coldkey_name, hotkey_name, password from environment or defaults.
      4) Call send_token(...), specifying the token details and chain context.
      5) Verify the returned tx_id is non-empty, indicating a successful transaction.

    Notes:
      - The function requires the hotkey address to hold at least 50 "MichielCOIN"
        plus enough ADA for fees (set to 200k lovelace here).
      - If there's insufficient token/ADA, the test may fail or raise an exception.
    """

    time.sleep(30)  # Optional delay to ensure environment readiness

    # 1) Receiver address
    to_address_str = settings.TEST_RECEIVER_ADDRESS
    if not to_address_str:
        pytest.skip("TEST_RECEIVER_ADDRESS not set => skipping test_send_michielcoin")

    # 2) Policy ID
    policy_id_hex = settings.TEST_POLICY_ID_HEX
    if not policy_id_hex:
        pytest.skip("TEST_POLICY_ID_HEX not set => skipping test_send_michielcoin")

    # 3) Base directory and coldkey config from environment/defaults
    base_dir = os.getenv("HOTKEY_BASE_DIR", "moderntensor")
    coldkey_name = os.getenv("COLDKEY_NAME", "kickoff")
    hotkey_name = os.getenv("HOTKEY_NAME", "hk1")
    password = os.getenv("HOTKEY_PASSWORD", "sonlearn2003")

    # Token info
    token_amount = 50
    asset_name = "MIT"

    # 4) Send tokens
    tx_id = send_token(
        chain_context=chain_context_fixture,
        base_dir=base_dir,
        coldkey_name=coldkey_name,
        hotkey_name=hotkey_name,
        password=password,
        to_address_str=to_address_str,
        policy_id_hex=policy_id_hex,
        asset_name=asset_name,
        token_amount=token_amount,
        fee=200_000,  # 0.2 ADA fee
        network=Network.TESTNET,
    )

    # 5) Validate tx_id
    logger.info(f"[test_send_michielcoin] => TX ID: {tx_id}")
    assert len(tx_id) > 0, "Transaction ID is empty => token transfer failed"
