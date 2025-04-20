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
    MultiAsset,
    Asset,
    Value,
    TransactionBuilder,
    TransactionOutput,
    Address,
    Network,
    Redeemer,  # Needed for minting redeemer (even if empty Data)
    RawPlutusData,  # For empty redeemer data
    TransactionId,
    min_lovelace,
)
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

    Returns:
        The TransactionId if submission is successful, None otherwise.
    """
    logger.info(f"Attempting to mint {amount_to_mint} of token '{asset_name_str}'...")

    try:
        # --- 1. Derive Keys and Addresses ---
        payment_vkey = signing_key.to_verification_key()
        payment_key_hash: VerificationKeyHash = payment_vkey.hash()  # type: ignore

        stake_key_hash: Optional[VerificationKeyHash] = None
        if stake_signing_key:
            stake_vkey = stake_signing_key.to_verification_key()
            stake_key_hash = stake_vkey.hash()  # type: ignore

        minter_address = Address(
            payment_part=payment_key_hash,
            staking_part=stake_key_hash,
            network=network,
        )
        logger.debug(f"Minter Address (paying fees): {minter_address}")

        # Determine recipient address
        if recipient_address is None:
            recipient_address = minter_address
        logger.debug(f"Recipient Address (receiving tokens): {recipient_address}")

        # --- 2. Prepare Script and Policy ID ---
        try:
            policy_script = PlutusV3Script(bytes.fromhex(policy_script_cbor_hex))
            policy_id: ScriptHash = policy_script.hash()  # type: ignore
            logger.debug(
                f"Policy ID derived from script: {policy_id.to_primitive().hex()}"
            )
            # Verify against settings (optional but good practice)
            if policy_id.to_primitive().hex() != settings.MINTING_POLICY_ID_PLACEHOLDER:
                logger.warning(
                    f"Derived Policy ID ({policy_id.to_primitive().hex()}) does not match settings ({settings.MINTING_POLICY_ID_PLACEHOLDER}). Using derived ID."
                )
                # Decide whether to raise an error or proceed with the derived ID
                # For now, proceed with the derived ID
        except Exception as e:
            logger.exception(f"Failed to create PlutusScript or derive Policy ID: {e}")
            return None

        # --- 3. Prepare Asset Name and Minting Structure ---
        try:
            # Encode asset name string to bytes, then hex if needed, but AssetName takes bytes
            asset_name_bytes = asset_name_str.encode("utf-8")
            asset_name = AssetName(asset_name_bytes)
            logger.debug(f"Asset Name (bytes): {asset_name_bytes.hex()}")

            # Create the MultiAsset structure for minting
            asset = Asset(asset_name=asset_name, amount=amount_to_mint)
            multi_asset = MultiAsset(assets={asset_name: asset})
            tokens_to_mint = MultiAsset(script_hash=policy_id, multi_asset=multi_asset)
            logger.debug(f"Tokens to mint structure prepared: {tokens_to_mint}")
        except Exception as e:
            logger.exception(f"Failed to prepare asset name or minting structure: {e}")
            return None

        # --- 4. Build Transaction ---
        builder = TransactionBuilder(context=context)

        builder.add_input_address(minter_address)
        logger.debug(f"Added input address for fees: {minter_address}")

        builder.mint = tokens_to_mint
        builder.add_minting_script(
            script=policy_script, redeemer=Redeemer(data=RawPlutusData(data=bytes()))
        )
        logger.debug("Added mint action and minting script.")

        # Add output to send minted tokens to the recipient
        # Calculate minimum ADA required for this output using the imported function
        temp_output = TransactionOutput(recipient_address, Value(0, tokens_to_mint))
        # Use min_lovelace from pycardano.utils
        min_ada_required = min_lovelace(output=temp_output, context=context)
        logger.debug(f"Calculated minimum ADA for output: {min_ada_required}")
        output_value = Value(coin=min_ada_required, multi_asset=tokens_to_mint)

        builder.add_output(
            TransactionOutput(
                address=recipient_address,
                amount=output_value,
            )
        )
        logger.debug(
            f"Added output for minted tokens to {recipient_address} (Value: {output_value})"
        )

        builder.required_signers = [payment_key_hash]
        logger.debug(f"Set required signer: {payment_key_hash.to_primitive().hex()}")

        # --- 5. Sign and Submit ---
        signing_keys = [signing_key]
        if stake_signing_key:
            signing_keys.append(stake_signing_key)

        logger.info("Building and signing the minting transaction...")
        signed_tx = builder.build_and_sign(
            signing_keys=signing_keys,  # type: ignore
            change_address=minter_address,  # Send change back to minter
        )
        logger.info(f"Transaction signed. Fee: {signed_tx.transaction_body.fee}")

        logger.info("Submitting minting transaction...")
        tx_id = context.submit_tx(signed_tx.to_cbor())
        logger.info(
            f":white_check_mark: Minting transaction submitted successfully! TxID: [yellow]{tx_id}[/yellow]"
        )
        return tx_id

    except ApiError as e:
        logger.error(
            f":x: Blockfrost API Error during minting: Status={e.status_code}, Message={e.message}"
        )
        return None
    except Exception as e:
        logger.exception(f":rotating_light: Unexpected error during token minting: {e}")
        return None
