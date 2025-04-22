# sdk/smartcontract/minting.py
"""Contains logic for building and submitting minting transactions."""

import logging
from typing import Optional, Dict, Any

from pycardano import (
    BlockFrostChainContext,
    ExtendedSigningKey,
    ExtendedVerificationKey,
    PaymentVerificationKey,
    StakeVerificationKey,
    VerificationKeyHash,
    ScriptHash,
    PlutusV3Script,  # Assuming Plutus V3 for Aiken
    AssetName,
    Asset,
    MultiAsset,
    Value,
    TransactionBuilder,
    TransactionOutput,
    Address,
    Network,
    Redeemer,  # Needed for minting redeemer (even if empty Data)
    RawPlutusData,  # For empty redeemer data
    TransactionId,
    min_lovelace,
    UTxO,
)
from pycardano.plutus import script_hash
from blockfrost import ApiError

# Import settings to get policy ID etc.
from sdk.config.settings import settings

logger = logging.getLogger(__name__)

# Determine network object from settings string
NETWORK = Network.TESTNET if settings.CARDANO_NETWORK == "TESTNET" else Network.MAINNET


async def mint_native_tokens(
    context: BlockFrostChainContext,
    signing_key: ExtendedSigningKey,
    stake_signing_key: Optional[ExtendedSigningKey],
    policy_script_cbor_hex: str,
    asset_name_str: str,  # Use string for asset name initially
    amount_to_mint: int,
    recipient_address: Optional[Address] = None,
    network: Network = NETWORK,  # <<< Use the determined Network object
    excluded_tx_id: Optional[TransactionId] = None,
) -> Optional[TransactionId]:
    """
    Builds and submits a transaction to mint native tokens using a given policy.

    Args:
        context: Cardano chain context (BlockFrost).
        signing_key: The extended signing key of the wallet paying fees and signing the mint.
        stake_signing_key: Optional stake signing key associated with the payment key.
        policy_script_cbor_hex: The CBOR hex of the compiled Plutus minting script (V3).
        asset_name_str: The desired name for the native token (e.g., "MOD").
        amount_to_mint: The quantity of the token to mint (in the smallest unit).
        recipient_address: The address to send the newly minted tokens to.
                           If None, sends to the address derived from signing_key.
        network: The Cardano network (Testnet or Mainnet).
        excluded_tx_id: Optional transaction ID to exclude from UTxO selection.

    Returns:
        The TransactionId if submission is successful, None otherwise.
    """
    logger.info(f"Attempting to mint {amount_to_mint} of token '{asset_name_str}'...")

    try:
        # --- 1. Derive Keys and Addresses ---
        payment_vkey = signing_key.to_verification_key()
        payment_pkh: VerificationKeyHash = payment_vkey.hash()
        stake_key_hash: Optional[VerificationKeyHash] = None
        if stake_signing_key:
            stake_vkey = stake_signing_key.to_verification_key()
            stake_key_hash = stake_vkey.hash()
        minter_address = Address(
            payment_part=payment_pkh, staking_part=stake_key_hash, network=network
        )
        logger.debug(f"Minter Address (paying fees): {minter_address}")
        if recipient_address is None:
            recipient_address = minter_address
        logger.debug(f"Recipient Address (receiving tokens): {recipient_address}")

        # --- 2. Prepare Script and Policy ID ---
        try:
            policy_script = PlutusV3Script(bytes.fromhex(policy_script_cbor_hex))
            policy_id: ScriptHash = script_hash(policy_script)
            logger.debug(
                f"Policy ID derived from script: {policy_id.to_primitive().hex()}"
            )
        except Exception as e:
            logger.exception(f"Failed to create PlutusScript or derive Policy ID: {e}")
            return None

        # --- 3. Prepare Asset Name and Minting Structure ---
        try:
            asset_name_bytes = asset_name_str.encode("utf-8")
            asset_name = AssetName(asset_name_bytes)
            inner_asset_dict = {asset_name: amount_to_mint}
            inner_asset_obj = Asset(inner_asset_dict)
            tokens_to_mint = MultiAsset({policy_id: inner_asset_obj})
            logger.debug(f"Tokens to mint structure prepared: {tokens_to_mint}")
        except Exception as e:
            logger.exception(f"Failed to prepare asset name or minting structure: {e}")
            return None

        # --- 4. Build Transaction ---
        builder = TransactionBuilder(context=context)

        # --- 4a. Add Inputs (builder fetches UTxOs internally) ---
        builder.add_input_address(minter_address)
        logger.debug(f"Added input address for fees: {minter_address}")

        # --- 4b. Add Collateral ---
        logger.debug(f"Attempting to select collateral UTxO from {minter_address}...")
        minter_utxos = context.utxos(str(minter_address))
        collateral_utxo = None
        MIN_COLLATERAL_ADA = 2_000_000  # Minimum 2 ADA for collateral UTXO
        if excluded_tx_id:
            original_count = len(minter_utxos)
            minter_utxos = [
                u for u in minter_utxos if u.input.transaction_id != excluded_tx_id
            ]
            filtered_count = len(minter_utxos)
            if original_count != filtered_count:
                logger.info(
                    f"Filtered out {original_count - filtered_count} UTXO(s) matching excluded TxID {excluded_tx_id}."
                )
        for utxo in minter_utxos:
            # Check if UTxO contains only ADA and has enough value
            if (
                not utxo.output.amount.multi_asset
                and utxo.output.amount.coin >= MIN_COLLATERAL_ADA
            ):
                collateral_utxo = utxo
                logger.info(
                    f"Selected collateral UTxO: {collateral_utxo.input.transaction_id.payload.hex()}#{collateral_utxo.input.index}"
                )
                break  # Use the first suitable one

        if not collateral_utxo:
            logger.error(
                f":x: Could not find a suitable collateral UTxO (>= {MIN_COLLATERAL_ADA/1_000_000} ADA only) at {minter_address}"
            )
            return None  # Cannot proceed without collateral

        # Add the selected **full** collateral UTxO to the builder.collaterals list
        builder.collaterals.append(collateral_utxo)
        logger.debug("Added collateral UTxO to the transaction builder.")

        # --- EXCLUDE the collateral UTXO from regular inputs ---
        builder.excluded_inputs.append(collateral_utxo)
        logger.debug(
            f"Excluded collateral UTxO {collateral_utxo.input.transaction_id.payload.hex()}#{collateral_utxo.input.index} from being used as regular input."
        )
        # -------------------------------------------------------

        # --- 4c. Add Minting Action ---
        builder.mint = tokens_to_mint
        builder.add_minting_script(
            script=policy_script, redeemer=Redeemer(data=RawPlutusData(data=bytes()))
        )
        logger.debug("Added mint action and minting script.")

        # --- 4d. Add Output ---
        temp_output = TransactionOutput(recipient_address, Value(0, tokens_to_mint))
        min_ada_required = min_lovelace(output=temp_output, context=context)
        logger.debug(f"Calculated minimum ADA for output: {min_ada_required}")
        output_value = Value(coin=min_ada_required, multi_asset=tokens_to_mint)
        builder.add_output(
            TransactionOutput(address=recipient_address, amount=output_value)
        )
        logger.debug(
            f"Added output for minted tokens to {recipient_address} (Value: {output_value})"
        )

        # --- 4e. Add Required Signer ---
        builder.required_signers = [payment_pkh]
        logger.debug(f"Set required signer: {payment_pkh.to_primitive().hex()}")

        # --- 5. Sign and Submit ---
        signing_keys = [signing_key]
        if stake_signing_key:
            signing_keys.append(stake_signing_key)
        logger.info("Building and signing the minting transaction...")
        signed_tx = builder.build_and_sign(
            signing_keys=signing_keys,  # type: ignore # Linter struggles with List invariance
            change_address=minter_address,
        )
        logger.info(f"Transaction signed. Fee: {signed_tx.transaction_body.fee}")
        logger.info("Submitting minting transaction...")
        tx_id_str = context.submit_tx(signed_tx.to_cbor())
        logger.info(
            f":white_check_mark: Minting transaction submitted successfully! TxID: [yellow]{tx_id_str}[/yellow]"
        )
        return TransactionId.from_primitive(tx_id_str)

    except ApiError as e:
        logger.error(
            f":x: Blockfrost API Error during minting: Status={e.status_code}, Message={e.message}"
        )
        return None
    except Exception as e:
        logger.exception(f":rotating_light: Unexpected error during token minting: {e}")
        return None
