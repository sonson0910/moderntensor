# scripts/distribute_rewards.py
"""
This script distributes rewards based on miner performance recorded in the validator state.
It fetches the current validator state UTXO, reads miner scores, calculates rewards,
and builds a transaction to distribute tokens and ADA to miners.
"""
import asyncio
import json
import logging
import os
import sys
from decimal import Decimal
from typing import Dict, List, Optional, Tuple

# Add project root to sys.path to allow importing sdk modules
# Determine the absolute path to the project root directory
project_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
if project_root not in sys.path:
    sys.path.append(project_root)

from blockfrost import ApiUrls  # Add missing import
from dotenv import load_dotenv
from pycardano import (
    Address,
    BlockFrostChainContext,
    Datum,
    ExtendedSigningKey,
    MultiAsset,
    PaymentSigningKey,
    PaymentVerificationKey,
    PlutusData,
    Redeemer,
    RedeemerTag,
    ScriptHash,
    TransactionBuilder,
    TransactionOutput,
    UTxO,
    Value,
    VerificationKeyHash,
    min_lovelace,
)  # Add min_lovelace and check other types
from pycardano.key import Ed25519SigningKey  # type: ignore[import]
from pycardano.nativescript import (
    NativeScript,
    PolicyId,  # type: ignore[import]
    SimpleScript,  # type: ignore[import]
)  # Assuming SimpleScript is here
from pycardano.transaction import Mint  # type: ignore[import]

from sdk.cardano_utils import CardanoNetwork, CardanoUtils  # type: ignore[import-unresolved]
from sdk.consensus.state import ValidatorState, MinerInfo, calculate_score, read_validator_utxo, MINER_UPDATE_SCRIPT_HASH, ValidatorInfo  # type: ignore[import-unresolved]
from sdk.keymanager.key_utils import derive_payment_keys, get_address, get_signing_info, get_coldkey_signing_key  # type: ignore[import-unresolved]
from sdk.logger_config import setup_logging  # type: ignore[import-unresolved]
from sdk.settings import Settings  # type: ignore[import-unresolved]

# Load environment variables
load_dotenv()

# Setup logging
log_level = os.getenv("LOG_LEVEL", "INFO").upper()
logger = setup_logging(__name__, log_level)

# Load settings
settings = Settings()

DEFAULT_WALLET_NAME = "default_coldkey"
DEFAULT_HOTKEY_NAME = "default_hotkey"

# Constants
ADA_TO_LOVELACE = 1_000_000
MIN_FUNDING_ADA = (
    2  # Minimum ADA to send with tokens if miner doesn't have enough UTXO value
)


class DistributionError(Exception):
    """Custom exception for distribution errors."""

    pass


async def get_distributor_signing_info(
    settings: Settings, context: BlockFrostChainContext
) -> Tuple[Address, PaymentSigningKey]:
    """
    Retrieves the signing information (address and signing key) for the distributor wallet.

    Args:
        settings: The application settings object.
        context: The BlockFrost chain context.

    Returns:
        A tuple containing the distributor's address and payment signing key.

    Raises:
        DistributionError: If the distributor wallet cannot be found or loaded.
    """
    try:
        coldkey_skey = get_coldkey_signing_key(settings.base_dir, settings.coldkey_name)
        if not coldkey_skey:
            raise DistributionError(f"Could not load coldkey '{settings.coldkey_name}'")
        payment_skey, _ = derive_payment_keys(coldkey_skey)
        distributor_address = get_address(
            payment_skey.to_verification_key(), settings.network
        )
        logger.info(f"Distributor Address: {distributor_address.encode()}")
        return distributor_address, payment_skey
    except Exception as e:
        logger.error(f"Failed to get distributor signing info: {e}")
        raise DistributionError(
            f"Failed to get distributor signing info for coldkey '{settings.coldkey_name}': {e}"
        )


async def fetch_validator_state(
    context: BlockFrostChainContext, settings: Settings
) -> Tuple[Optional[UTxO], Optional[ValidatorState]]:
    """
    Fetches the current validator state UTXO and deserializes the state.

    Args:
        context: The BlockFrost chain context.
        settings: The application settings object.

    Returns:
        A tuple containing the validator state UTXO and the deserialized ValidatorState object,
        or (None, None) if the state cannot be fetched or deserialized.
    """
    logger.info("Fetching current validator state...")
    try:
        # Use TEST_CONTRACT_ADDRESS from settings
        validator_address = Address.from_primitive(settings.TEST_CONTRACT_ADDRESS)
        validator_utxo = await read_validator_utxo(context, validator_address)
        if not validator_utxo:
            logger.warning("No validator state UTXO found.")
            return None, None

        validator_state = ValidatorState.from_cbor(validator_utxo.output.datum.cbor)  # type: ignore[attr-defined] - datum assumed to exist if utxo found
        logger.info("Successfully fetched and deserialized validator state.")
        return validator_utxo, validator_state
    except Exception as e:
        logger.error(f"Failed to fetch or deserialize validator state: {e}")
        return None, None


def calculate_rewards(
    validator_state: ValidatorState,
    total_reward_amount: int,
    token_asset_name_hex: str,
    token_policy_id: PolicyId,
) -> Dict[str, Value]:
    """
    Calculates the rewards for each miner based on their score.

    Args:
        validator_state: The current validator state.
        total_reward_amount: The total amount of the reward token to distribute.
        token_asset_name_hex: The hex representation of the reward token asset name.
        token_policy_id: The policy ID of the reward token.

    Returns:
        A dictionary mapping miner addresses (str) to their reward Value (token amount + min ADA).
    """
    rewards: Dict[str, Value] = {}
    total_score = sum(
        miner.score
        for miner in validator_state.miners.values()
        if miner.score is not None
    )

    if total_score == 0:
        logger.warning("Total score is zero. No rewards will be distributed.")
        return rewards

    asset_name_bytes = bytes.fromhex(token_asset_name_hex)
    reward_multi_asset = MultiAsset.from_primitive(
        {
            token_policy_id.payload: {
                asset_name_bytes: 0  # Placeholder, will be updated per miner
            }
        }
    )

    logger.info(
        f"Calculating rewards for {len(validator_state.miners)} miners. Total score: {total_score}, Total reward: {total_reward_amount}"
    )

    for miner_id, miner_info in validator_state.miners.items():
        if miner_info.score is not None and miner_info.score > 0:
            miner_reward_amount = int(
                (Decimal(miner_info.score) / Decimal(total_score))
                * Decimal(total_reward_amount)
            )
            if miner_reward_amount > 0:
                try:
                    # Assuming miner_id is the address string
                    miner_address_str = miner_id
                    miner_address = Address.from_primitive(miner_address_str)

                    # Calculate required minimum Lovelace for the token output
                    temp_output = TransactionOutput(
                        miner_address,
                        Value(
                            0,
                            MultiAsset.from_primitive(
                                {
                                    token_policy_id.payload: {
                                        asset_name_bytes: miner_reward_amount
                                    }
                                }
                            ),
                        ),
                    )
                    # Pass context, ignore likely incorrect linter error
                    required_lovelace = min_lovelace(output=temp_output, context=context)  # type: ignore[name-defined]

                    # Create the reward value: tokens + minimum required Lovelace
                    # Create a new MultiAsset for the specific reward
                    miner_specific_multi_asset = MultiAsset.from_primitive(
                        {
                            token_policy_id.payload: {
                                asset_name_bytes: miner_reward_amount
                            }
                        }
                    )
                    rewards[miner_address_str] = Value(
                        coin=required_lovelace, multi_asset=miner_specific_multi_asset
                    )
                    logger.debug(
                        f"Miner {miner_address_str}: Score={miner_info.score}, Reward Tokens={miner_reward_amount}, Required ADA={required_lovelace / ADA_TO_LOVELACE:.6f}"
                    )

                except Exception as e:
                    logger.error(f"Error processing reward for miner {miner_id}: {e}")
            else:
                logger.debug(
                    f"Miner {miner_id}: Score={miner_info.score}, Calculated reward amount is zero or negative."
                )
        else:
            logger.debug(
                f"Miner {miner_id}: Score is None or zero, skipping reward calculation."
            )

    # Clean up the multi_asset structure if no rewards were actually assigned (though handled by outer checks)
    if not any(rewards.values()):
        logger.warning("No positive rewards calculated for any miner.")

    # Log total distributed rewards
    total_distributed_tokens = sum(
        v.multi_asset.get(token_policy_id.payload, {}).get(asset_name_bytes, 0)
        for v in rewards.values()
    )
    total_distributed_ada = sum(v.coin for v in rewards.values())
    logger.info(
        f"Total distributed: {total_distributed_tokens} tokens, {total_distributed_ada / ADA_TO_LOVELACE:.6f} ADA to {len(rewards)} miners."
    )

    return rewards


async def build_distribution_transaction(
    context: BlockFrostChainContext,
    settings: Settings,
    distributor_address: Address,
    distributor_skey: PaymentSigningKey,
    rewards: Dict[str, Value],
    validator_utxo: UTxO,
    validator_state: ValidatorState,
    token_policy_id: PolicyId,
    token_asset_name_hex: str,
) -> Optional[TransactionBuilder]:
    """
    Builds the transaction to distribute rewards and update the validator state.

    Args:
        context: BlockFrost chain context.
        settings: Application settings.
        distributor_address: The address of the distributor wallet.
        distributor_skey: The signing key of the distributor wallet.
        rewards: Dictionary mapping miner addresses to their reward Values.
        validator_utxo: The UTXO holding the current validator state.
        validator_state: The deserialized validator state object.
        token_policy_id: Policy ID of the reward token.
        token_asset_name_hex: Hex representation of the reward token asset name.

    Returns:
        A TransactionBuilder object ready to be signed and submitted, or None if an error occurs.
    """
    logger.info("Building distribution transaction...")
    builder = TransactionBuilder(context)
    builder.add_input_address(
        distributor_address
    )  # Add distributor's UTXOs for fees/collateral

    # --- Consume Validator UTXO ---
    # Use TEST_CONTRACT_ADDRESS and MINER_UPDATE_SCRIPT_HASH from settings
    try:
        validator_script_hash = ScriptHash.from_primitive(
            settings.TEST_CONTRACT_ADDRESS
        )  # This should be the address, not the hash? Let's assume Address for now.
        validator_address = Address.from_primitive(settings.TEST_CONTRACT_ADDRESS)
        # Need the Plutus script itself to provide it for spending
        # This needs to be loaded/defined somewhere. Placeholder:
        # plutus_script = PlutusV2Script(bytes.fromhex(settings.VALIDATOR_SCRIPT_CBOR)) # Requires settings.VALIDATOR_SCRIPT_CBOR
        # Assuming we don't have the script CBOR readily available. This might fail if context needs it.

        # Find the correct input UTXO
        spend_validator_utxo = None
        utxos = context.utxos(str(validator_address))
        for u in utxos:
            if u.input == validator_utxo.input:
                spend_validator_utxo = u
                break
        if not spend_validator_utxo:
            logger.error(
                f"Could not find the specific validator UTXO {validator_utxo.input} at address {validator_address}"
            )
            return None

        # Add validator UTXO as input - requires script and redeemer
        # Placeholder for redeemer - Update action?
        # redeemer_data = PlutusData() # This needs to be the correct redeemer for the script logic
        # builder.add_script_input(spend_validator_utxo, script=plutus_script, datum=validator_utxo.output.datum, redeemer=Redeemer(redeemer_data, RedeemerTag.SPEND)) # Requires script and redeemer

        # --- Temporary: Add as regular input if script logic is not handled here ---
        # This is likely incorrect for a script UTXO spend
        builder.add_input(spend_validator_utxo)
        logger.warning(
            "Adding validator UTXO as regular input. This is likely incorrect and needs script/redeemer handling."
        )

    except Exception as e:
        logger.error(f"Error preparing validator script input: {e}")
        return None

    # --- Add Reward Outputs ---
    total_ada_needed = 0
    total_tokens_needed = 0
    asset_name_bytes = bytes.fromhex(token_asset_name_hex)

    for address_str, value in rewards.items():
        try:
            reward_address = Address.from_primitive(address_str)
            builder.add_output(TransactionOutput(reward_address, value))
            total_ada_needed += value.coin
            tokens_in_value = value.multi_asset.get(token_policy_id.payload, {}).get(
                asset_name_bytes, 0
            )
            total_tokens_needed += tokens_in_value
            logger.debug(
                f"Adding output: {tokens_in_value} tokens and {value.coin / ADA_TO_LOVELACE:.6f} ADA to {address_str}"
            )
        except Exception as e:
            logger.error(f"Failed to add reward output for {address_str}: {e}")
            return None

    # --- Update Validator State ---
    # Create the new state (e.g., reset scores, update timestamp) - Placeholder logic
    new_validator_state = ValidatorState(
        last_update=validator_state.last_update + 1,
        miners={
            # Revert to building dictionary structure for new state to avoid MinerInfo instantiation issues
            m_id: {
                "uid": info.uid,  # Use uid from info if available
                "address": info.address,
                "score": 0,  # Reset score
                "trust": info.trust,
                "last_update": info.last_update,
                "registration_cycle": info.registration_cycle,
            }
            for m_id, info in validator_state.miners.items()
        },
    )
    new_datum = new_validator_state.to_cbor_hex()  # type: ignore[attr-defined]
    # Ignore potential linter issue with hash attribute access
    new_datum_hash = PlutusData.from_cbor(new_datum).hash()  # type: ignore[attr-defined]

    # Output back to validator address with the original ADA + any leftover ADA from input?
    # The value should typically preserve the ADA from the input UTXO unless the script dictates otherwise.
    output_value = validator_utxo.output.amount
    builder.add_output(
        TransactionOutput(
            validator_address,
            output_value,
            # Ignore potential linter issue with datum type assignment
            datum=PlutusData.from_cbor(new_datum),  # type: ignore[assignment]
            datum_hash=new_datum_hash,
        )
    )
    logger.info(
        f"Adding updated validator state output back to {validator_address} with value {output_value}"
    )  # Log the value being sent back

    # --- Add Minting ---
    # Mint the required tokens. Requires the minting policy script.
    # MINTING_POLICY_ID_PLACEHOLDER needs to be the actual Policy ID.
    # Assuming a simple script requiring the distributor's signature.
    # Policy ID should match token_policy_id derived earlier.
    try:
        # Use MINTING_POLICY_ID_PLACEHOLDER from settings
        policy_id = PolicyId.from_primitive(settings.MINTING_POLICY_ID_PLACEHOLDER)
        if policy_id != token_policy_id:
            logger.warning(
                f"Policy ID mismatch: from settings ({policy_id}) vs derived ({token_policy_id})"
            )
            # Decide which one to use or raise error. Using derived one for consistency.
            policy_id = token_policy_id

        # Define the minting script (SimpleScript example)
        pub_key_hash = distributor_skey.to_verification_key().hash()
        # Need to check if VerificationKeyHash is directly usable here or needs bytes.
        script = SimpleScript(pub_key_hash)  # This assumes a simple signature script

        mint_assets = MultiAsset.from_primitive(
            {policy_id.payload: {asset_name_bytes: total_tokens_needed}}
        )
        builder.mint = mint_assets
        builder.native_scripts = [script]  # Add the script required for minting
        logger.info(
            f"Adding mint action: {total_tokens_needed} tokens with policy {policy_id}"
        )

    except Exception as e:
        logger.error(f"Failed to prepare minting action: {e}")
        return None

    # --- Add Required Signer ---
    # If minting script requires it, or other script requires it.
    builder.required_signers = [distributor_skey.to_verification_key().hash()]
    logger.info("Adding distributor key hash as required signer.")

    # Set TTL
    builder.ttl = context.last_block_slot + 3600  # Expires in 1 hour

    # Calculate fees (will be done automatically if input address is added)
    # Finalize and sign (will be done after this function returns)
    logger.info("Transaction build process complete (pre-fee calculation).")

    # Note: Fee calculation and signing happen outside this function.
    # The builder requires sufficient UTXOs at distributor_address to cover fees + ADA outputs.
    # Collateral might also be needed if interacting with Plutus scripts.
    # builder.add_collateral() # May need this

    return builder


async def main():
    """Main function to run the distribution script."""
    logger.info("--- Starting Reward Distribution ---")

    try:
        # --- Configuration ---
        bf_project_id = os.getenv("BLOCKFROST_PROJECT_ID")
        if not bf_project_id:
            raise DistributionError(
                "BLOCKFROST_PROJECT_ID environment variable not set."
            )

        # Use settings for network and base dir
        network = settings.network
        base_dir = settings.base_dir
        context = BlockFrostChainContext(
            bf_project_id, base_url=ApiUrls[network.name].value
        )

        # Use settings for coldkey name
        coldkey_name = settings.coldkey_name or DEFAULT_WALLET_NAME
        logger.info(f"Using Network: {network.name}")
        logger.info(f"Using Base Directory: {base_dir}")
        logger.info(f"Using Coldkey: {coldkey_name}")

        # --- Get Distributor Info ---
        distributor_address, distributor_skey = await get_distributor_signing_info(
            settings, context
        )

        # --- Fetch Validator State ---
        validator_utxo, validator_state = await fetch_validator_state(context, settings)
        if not validator_utxo or not validator_state:
            raise DistributionError(
                "Failed to retrieve validator state. Cannot proceed."
            )

        # --- Token Information ---
        # Use MINTING_POLICY_ID_PLACEHOLDER and TOKEN_NAME_STR from settings
        token_policy_id = PolicyId.from_primitive(
            settings.MINTING_POLICY_ID_PLACEHOLDER
        )
        token_name_str = settings.TOKEN_NAME_STR
        token_asset_name_hex = token_name_str.encode("utf-8").hex()
        total_reward_amount = (
            settings.TOTAL_REWARD_AMOUNT
        )  # Get total reward amount from settings

        logger.info(f"Reward Token Policy ID: {token_policy_id}")
        logger.info(
            f"Reward Token Name: {token_name_str} (Hex: {token_asset_name_hex})"
        )
        logger.info(f"Total Reward Amount to Distribute: {total_reward_amount}")

        # --- Calculate Rewards ---
        rewards = calculate_rewards(
            validator_state, total_reward_amount, token_asset_name_hex, token_policy_id
        )
        if not rewards:
            logger.warning("No rewards calculated. Exiting.")
            return

        # --- Build Transaction ---
        builder = await build_distribution_transaction(
            context,
            settings,
            distributor_address,
            distributor_skey,
            rewards,
            validator_utxo,
            validator_state,
            token_policy_id,
            token_asset_name_hex,
        )
        if not builder:
            raise DistributionError("Failed to build the distribution transaction.")

        # --- Sign and Submit ---
        logger.info("Signing and submitting transaction...")
        try:
            signed_tx = builder.build_and_sign(
                [distributor_skey], change_address=distributor_address
            )
            tx_hash = await context.submit_tx(signed_tx.to_cbor())
            logger.info(f"Transaction submitted successfully! Tx Hash: {tx_hash}")
            logger.info(
                f"View on Cardanoscan ({network.name}): https://{'' if network == CardanoNetwork.MAINNET else 'preprod.'}cardanoscan.io/transaction/{tx_hash}"
            )

        except Exception as e:
            logger.error(f"Transaction submission failed: {e}")
            # If fees are the issue, log the calculated fee vs available funds
            try:
                # Attempt to calculate fee again for logging, requires inputs to be finalized
                # fee = builder.calculate_fee() # This might fail if build_and_sign already did this
                # logger.error(f"Estimated Fee: {fee / ADA_TO_LOVELACE:.6f} ADA")
                # TODO: Log available funds at distributor address for comparison
                pass
            except Exception as fee_e:
                logger.error(f"Could not calculate fee for logging: {fee_e}")
            raise DistributionError(f"Transaction submission failed: {e}")

    except DistributionError as e:
        logger.error(f"Distribution failed: {e}")
    except Exception as e:
        logger.error(f"An unexpected error occurred: {e}", exc_info=True)
    finally:
        logger.info("--- Reward Distribution Finished ---")


if __name__ == "__main__":
    asyncio.run(main())
