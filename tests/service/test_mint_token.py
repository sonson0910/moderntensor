# tests/service/test_mint_michielcoin.py

import os
import time
import pytest
from os.path import exists

from sdk.compat.pycardano import (
    PaymentKeyPair,
    PaymentSigningKey,
    PaymentVerificationKey,
    ScriptPubkey,
    ScriptAll,
    TransactionBuilder,
    TransactionOutput,
    Value,
    AssetName,
    Asset,
    MultiAsset,
    min_lovelace,
    Address,
    Network,
)

from sdk.config.settings import settings, logger  # Use global settings & logger
from sdk.service.context import get_chain_context


@pytest.fixture(scope="session")
def chain_context_fixture():
    """
    Creates a chain_context using BlockFrost for Cardano TESTNET.
    This fixture is shared (session-scoped) across all tests requiring blockchain access.

    Steps:
      1) Reads BLOCKFROST_PROJECT_ID from environment or settings if implemented.
      2) Calls get_chain_context(method="blockfrost") to initialize a BlockFrostChainContext (TESTNET).
      3) Returns the context for UTxO queries, transaction submissions, etc.
    """
    return get_chain_context(method="blockfrost")


@pytest.mark.integration
def test_mint_mit_token(chain_context_fixture, hotkey_skey_fixture):
    """
    Integration test: Mints 1,000,000 units of "MichielCOIN" (symbol "MIT")
    using a simple policy script. Demonstrates a "legacy" minting approach.

    Steps:
      1) Retrieve (payment_skey, staking_skey) from hotkey_skey_fixture.
      2) Construct main_address (the minter's address).
      3) Ensure that main_address has at least one UTxO.
      4) Generate/load policy keys (policy.skey/policy.vkey) if they don't exist.
      5) Build the policy script with ScriptPubkey in a ScriptAll.
      6) Create a MultiAsset for 1,000,000 "MichielCOIN" tokens.
      7) Add an output to contain the tokens + min ADA.
      8) Add inputs (main_address) and sign with payment_skey + policy_signing_key.
      9) Submit the transaction and check the TX ID is non-empty.

    Notes:
      - If main_address has no UTxOs, we skip the test.
      - The minted tokens end up at receive_address (same as main_address).
      - The policy keys are stored in coldkey_dir for potential future reuse.
    """

    time.sleep(30)  # Additional wait if needed for prior TX to settle, etc.

    # 0) Get Payment/Stake extended signing keys from fixture
    (payment_skey, staking_skey) = hotkey_skey_fixture
    logger.info(
        "[test_mint_michielcoin] Start minting 'MichielCOIN' with hotkey as payment key."
    )

    # 1) Construct main_address, also reuse it as receive_address
    from_network = chain_context_fixture.network
    main_address = Address(
        payment_part=payment_skey.to_verification_key().hash(),
        staking_part=staking_skey.to_verification_key().hash(),
        network=from_network,
    )
    receive_address = main_address
    logger.info(f"main_address   = {main_address}")
    logger.info(f"receive_address= {receive_address}")

    # 2) Check UTxOs => skip if none
    utxos = chain_context_fixture.utxos(main_address)
    if not utxos:
        pytest.skip("main_address does not have any UTxOs => skip mint test")

    # 3) Create a transaction builder
    builder = TransactionBuilder(chain_context_fixture)

    # 4) Decide where to store the policy keys
    base_dir = os.getenv("HOTKEY_BASE_DIR", "moderntensor")
    coldkey_name = os.getenv("COLDKEY_NAME", "kickoff")
    coldkey_dir = os.path.join(base_dir, coldkey_name)
    os.makedirs(coldkey_dir, exist_ok=True)

    policy_skey_path = os.path.join(coldkey_dir, "policy.skey")
    policy_vkey_path = os.path.join(coldkey_dir, "policy.vkey")

    # 5) Generate policy key pair if none exist
    if not exists(policy_skey_path) and not exists(policy_vkey_path):
        key_pair = PaymentKeyPair.generate()
        key_pair.signing_key.save(policy_skey_path)
        key_pair.verification_key.save(policy_vkey_path)
        logger.info(
            f"[test_mint_michielcoin] Generated policy keys => {policy_skey_path}, {policy_vkey_path}"
        )

    # 6) Load policy keys, build a simple ScriptAll policy with ScriptPubkey
    policy_signing_key = PaymentSigningKey.load(policy_skey_path)
    policy_verification_key = PaymentVerificationKey.load(policy_vkey_path)
    pub_key_policy = ScriptPubkey(policy_verification_key.hash()) # type: ignore
    policy = ScriptAll([pub_key_policy])

    policy_id = policy.hash()
    policy_id_hex = policy_id.payload.hex()
    logger.info(f"[test_mint_michielcoin] policy_id = {policy_id_hex}")

    # 7) Define a MultiAsset for 1,000,000 "MichielCOIN" (symbol "MIT")
    asset_name = "MIT"
    asset_name_bytes = asset_name.encode("utf-8")
    token_asset_name = AssetName(asset_name_bytes)

    new_asset = Asset()
    new_asset[token_asset_name] = 1_000_000

    multiasset = MultiAsset()
    multiasset[policy_id] = new_asset

    # Add the script and minted assets to the builder
    builder.native_scripts = [policy]
    builder.mint = multiasset

    # 8) Calculate min ADA for a UTxO with this multi-asset => add to the builder
    min_val = min_lovelace(
        chain_context_fixture,
        output=TransactionOutput(receive_address, Value(0, multiasset)),
    )
    builder.add_output(TransactionOutput(receive_address, Value(min_val, multiasset)))

    # Add inputs from main_address
    builder.add_input_address(main_address)

    # Build and sign => [payment_skey, policy_signing_key]
    signed_tx = builder.build_and_sign(
        signing_keys=[payment_skey, policy_signing_key], change_address=main_address # type: ignore
    )

    # 9) Submit the transaction
    result = chain_context_fixture.submit_tx(signed_tx.to_cbor())

    logger.info(f"Number of inputs : {len(signed_tx.transaction_body.inputs)}")
    logger.info(f"Number of outputs: {len(signed_tx.transaction_body.outputs)}")
    logger.info(f"Fee             : {signed_tx.transaction_body.fee/1_000_000} ADA")
    logger.info(f"Transaction submitted => {result}")

    # 10) Ensure the result (tx ID) is not empty
    assert len(result) > 0, "Empty TX ID => mint transaction failed."
