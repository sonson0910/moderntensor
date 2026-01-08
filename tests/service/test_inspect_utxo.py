# tests/service/test_inspect_utxo.py

import os
import pytest
from sdk.compat.luxtensor_types import Address
from sdk.config.settings import settings, logger  # Global logger/settings
from sdk.service.context import get_chain_context


@pytest.fixture(scope="session")
def chain_context_fixture():
    """
    Creates a BlockFrost-based chain context for Cardano TESTNET
    to be shared across tests in this session.

    Steps:
      1) Reads BLOCKFROST_PROJECT_ID from the environment or from settings if configured.
      2) Calls get_chain_context(method="blockfrost") to initialize a BlockFrostChainContext.
      3) Returns this context so that tests can query UTxOs or submit transactions.

    Returns:
        A chain context object (BlockFrostChainContext) for Cardano TESTNET.
    """
    return get_chain_context(method="blockfrost")


@pytest.mark.integration
def test_inspect_utxo(chain_context_fixture, hotkey_skey_fixture):
    """
    Integration test to inspect (log/print) all UTxOs at the address derived from
    the provided hotkey_skey_fixture. This is primarily for observation/troubleshooting.

    Steps:
      1) Decode the hotkey_skey_fixture to retrieve payment and stake signing keys.
      2) Construct an Address object (TESTNET) from these signing keys.
      3) Query the chain context for the UTxOs at this address.
      4) Print/log detailed information about each UTxO (ADA and any multi-asset tokens).

    Args:
        chain_context_fixture: A BlockFrostChainContext fixture providing blockchain access.
        hotkey_skey_fixture: A fixture that returns a tuple (payment_xsk, stake_xsk)
                            for deriving addresses.

    Notes:
      - The test simply prints UTxO details and marks itself as passed (assert True).
      - This test is useful for debugging or verifying token balances on TESTNET.
    """

    # Retrieve the payment and stake signing keys from fixture
    (payment_xsk, stake_xsk) = hotkey_skey_fixture
    from_network = chain_context_fixture.network  # Typically TESTNET

    # Convert the signing keys to verification keys
    pay_xvk = payment_xsk.to_verification_key()
    stake_xvk = stake_xsk.to_verification_key() if stake_xsk else None

    # Construct the address (TESTNET) from payment and optional stake verification keys
    from_address = Address(
        payment_part=pay_xvk.hash(),
        staking_part=stake_xvk.hash() if stake_xvk else None,
        network=from_network,
    )

    logger.info(f"Inspecting UTxOs for address: {from_address}")

    # Fetch all UTxOs for this address
    utxos = chain_context_fixture.utxos(from_address)
    if not utxos:
        logger.info("No UTxOs found => Possibly zero tokens and zero ADA.")
        # Still pass the test, as having no UTxOs is valid
        return

    # Loop through each UTxO and log details
    for idx, utxo in enumerate(utxos, start=1):
        logger.info(f"\n--- UTxO #{idx} ---")
        logger.info(f"Input (tx_in) : {utxo.input}")
        logger.info(f"ADA coin      : {utxo.output.amount.coin}")

        ma = utxo.output.amount.multi_asset
        if ma:
            for policy_id, assets_map in ma.items():
                for aname, qty in assets_map.items():
                    # Convert policy_id and asset_name to readable formats
                    policy_id_hex = policy_id.payload.hex()
                    asset_name_str = aname.payload.decode("utf-8", errors="replace")

                    logger.info(
                        f"  policy_id={policy_id_hex} "
                        f"asset_name={asset_name_str} "
                        f"quantity={qty}"
                    )
        else:
            logger.info("No multi-asset tokens in this UTxO")

    # The test passes as long as no exceptions occur
    assert True
