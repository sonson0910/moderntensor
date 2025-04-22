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
from typing import Dict, List, Optional, Tuple, Any
import binascii

# Add project root to sys.path to allow importing sdk modules
# Determine the absolute path to the project root directory
project_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
if project_root not in sys.path:
    sys.path.append(project_root)

# --- External Libraries ---
from blockfrost import ApiUrls
from dotenv import load_dotenv
from cryptography.fernet import Fernet
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
    Network as CardanoNetwork,  # Import CardanoNetwork here
    Address,  # Import Address again for get_address usage if needed
)

# Keep ignores for pycardano if linter still complains
from pycardano.key import Ed25519SigningKey  # type: ignore[import]
from pycardano.nativescript import NativeScript, ScriptPubkey  # type: ignore[import]
from pycardano.transaction import Mint  # type: ignore[import]

# --- SDK Imports (Corrected based on user feedback and assumptions) ---
from sdk.config.settings import Settings
from sdk.keymanager.encryption_utils import get_or_create_salt, generate_encryption_key  # type: ignore[import-unresolved]

# from sdk.consensus.state import ValidatorState, read_validator_utxo # type: ignore[import-unresolved]
from sdk.core.datatypes import MinerInfo, ValidatorInfo  # type: ignore[import-unresolved]

# from sdk.consensus.scoring import calculate_score # Assuming calculate_score isn't used directly here
# from sdk.consensus.state import MINER_UPDATE_SCRIPT_HASH # Assuming not used directly here
# from sdk.keymanager.wallet_manager import get_signing_info # Assuming get_distributor_signing_info is used instead

# Import get_all_miner_data
from sdk.metagraph.metagraph_data import get_all_miner_data  # type: ignore[import-unresolved]

# Load environment variables
load_dotenv()

# Setup logging - Use standard logging, relies on config in settings.py
logger = logging.getLogger(__name__)

# Load settings - Ignore linter errors on instantiation
settings = Settings()  # type: ignore[call-arg]

DEFAULT_WALLET_NAME = "default_coldkey"
DEFAULT_HOTKEY_NAME = "default_hotkey"

# Constants
ADA_TO_LOVELACE = 1_000_000
MIN_FUNDING_ADA = 2


class DistributionError(Exception):
    """Custom exception for distribution errors."""

    pass


async def get_distributor_signing_info(
    settings: Settings, context: BlockFrostChainContext
) -> Tuple[Address, ExtendedSigningKey]:
    """
    Retrieves the signing information (address and EXTENDED signing key) for the distributor wallet
    by decrypting the specified hotkey information using the coldkey password.

    Args:
        settings: The application settings object (needs HOTKEY_BASE_DIR, COLDKEY_NAME, HOTKEY_NAME, HOTKEY_PASSWORD).
        context: The BlockFrost chain context.

    Returns:
        A tuple containing the distributor's address and extended signing key.

    Raises:
        DistributionError: If the distributor hotkey/coldkey cannot be found or loaded/decrypted.
    """
    logger.info(
        f"Attempting to load distributor signing info from hotkey: {settings.COLDKEY_NAME}/{settings.HOTKEY_NAME}"
    )
    try:
        base_dir = settings.HOTKEY_BASE_DIR
        coldkey_name = settings.COLDKEY_NAME
        hotkey_name = settings.HOTKEY_NAME
        password = (
            settings.HOTKEY_PASSWORD
        )  # Assumes password for the coldkey managing the distributor hotkey

        coldkey_dir = os.path.join(base_dir, coldkey_name)
        hotkeys_json_path = os.path.join(coldkey_dir, "hotkeys.json")

        if not os.path.exists(hotkeys_json_path):
            raise DistributionError(
                f"hotkeys.json not found for coldkey '{coldkey_name}' at {coldkey_dir}"
            )

        # Retrieve salt and generate decryption key
        salt = get_or_create_salt(coldkey_dir)  # type: ignore[name-defined]
        enc_key = generate_encryption_key(password, salt)  # type: ignore[name-defined]
        cipher = Fernet(enc_key)

        # Load hotkeys data
        with open(hotkeys_json_path, "r") as f:
            data = json.load(f)

        if hotkey_name not in data.get("hotkeys", {}):
            raise DistributionError(
                f"Hotkey '{hotkey_name}' not found in {hotkeys_json_path}"
            )

        # Decrypt the data
        enc_data = data["hotkeys"][hotkey_name].get("encrypted_data")
        if not enc_data:
            raise DistributionError(
                f"Missing 'encrypted_data' for hotkey '{hotkey_name}'"
            )

        try:
            dec = cipher.decrypt(enc_data.encode("utf-8"))
            hotkey_data = json.loads(dec.decode("utf-8"))
        except Exception as decrypt_err:
            logger.error(
                f"Decryption failed for hotkey '{hotkey_name}'. Invalid password or corrupted data: {decrypt_err}"
            )
            raise DistributionError(f"Decryption failed for hotkey '{hotkey_name}'")

        # Extract payment signing key hex
        pay_hex = hotkey_data.get("payment_xsk_cbor_hex")
        if not pay_hex:
            raise DistributionError(
                f"Missing 'payment_xsk_cbor_hex' in decrypted data for hotkey '{hotkey_name}'"
            )

        # Convert CBOR hex to ExtendedSigningKey
        payment_xsk = ExtendedSigningKey.from_cbor(binascii.unhexlify(pay_hex))

        # Derive address
        payment_vkey = payment_xsk.to_verification_key()  # type: ignore[attr-defined]
        distributor_address = Address(
            payment_part=payment_vkey.hash(), network=context.network
        )

        logger.info(
            f"Successfully loaded distributor signing info. Address: {distributor_address.encode()}"
        )
        # Ignore return type error if needed
        return distributor_address, payment_xsk  # type: ignore[return-value]

    except FileNotFoundError:
        raise DistributionError(
            f"Could not find coldkey directory or hotkeys.json for '{coldkey_name}'"
        )
    except KeyError as e:
        raise DistributionError(
            f"Missing key '{e}' in hotkeys.json structure for '{hotkey_name}'"
        )
    except Exception as e:
        logger.error(f"Failed to get distributor signing info: {e}", exc_info=True)
        raise DistributionError(
            f"Failed to get distributor signing info for hotkey '{settings.HOTKEY_NAME}' under coldkey '{settings.COLDKEY_NAME}': {e}"
        )


def calculate_rewards(
    miner_data_list: List[Dict[str, Any]],
    total_reward_amount: int,
    token_asset_name_hex: str,
    token_policy_id: ScriptHash,
    context: BlockFrostChainContext,
) -> Dict[str, Value]:
    """
    Calculates rewards based on a list of miner data dictionaries.
    Assumes each dictionary contains 'score' and 'address'.
    """
    rewards: Dict[str, Value] = {}
    # Calculate total score from the list
    scores = [
        data.get("score", 0)
        for data in miner_data_list
        if data.get("score") is not None
    ]
    total_score = sum(scores) if scores else 0

    if total_score == 0:
        logger.warning("Total score is zero. No rewards will be distributed.")
        return rewards

    asset_name_bytes = bytes.fromhex(token_asset_name_hex)

    logger.info(
        f"Calculating rewards for {len(miner_data_list)} miners. Total score: {total_score}, Total reward: {total_reward_amount}"
    )

    # Iterate through the list of miner data
    for miner_data in miner_data_list:
        miner_score = miner_data.get("score")
        miner_address_str = miner_data.get(
            "address"
        )  # Assuming 'address' key holds the address string
        miner_uid = miner_data.get("uid", "Unknown UID")  # Get UID for logging

        if miner_score is not None and miner_score > 0 and miner_address_str:
            miner_reward_amount = int(
                (Decimal(miner_score) / Decimal(total_score))
                * Decimal(total_reward_amount)
            )
            if miner_reward_amount > 0:
                try:
                    miner_address = Address.from_primitive(miner_address_str)

                    # Create the MultiAsset for the temporary output calculation
                    temp_multi_asset = MultiAsset.from_primitive(
                        {
                            token_policy_id.payload: {
                                asset_name_bytes: miner_reward_amount
                            }
                        }
                    )
                    temp_output = TransactionOutput(
                        miner_address,
                        Value(coin=0, multi_asset=temp_multi_asset),
                    )

                    required_lovelace = min_lovelace(
                        output=temp_output, context=context
                    )

                    # Create the final reward Value
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
                        f"Miner {miner_uid} (Addr: {miner_address_str}): Score={miner_score}, Reward Tokens={miner_reward_amount}, Required ADA={required_lovelace / ADA_TO_LOVELACE:.6f}"
                    )

                except Exception as e:
                    logger.error(
                        f"Error processing reward for miner {miner_uid} (Addr: {miner_address_str}): {e}"
                    )
            else:
                logger.debug(
                    f"Miner {miner_uid} (Addr: {miner_address_str}): Score={miner_score}, Calculated reward amount is zero or negative."
                )
        else:
            logger.debug(
                f"Miner {miner_uid} (Addr: {miner_address_str}): Score is None, zero, or missing address. Skipping reward calculation."
            )

    # Log total distributed rewards
    total_distributed_tokens = 0
    total_distributed_ada = 0
    if rewards:
        asset_name_bytes = bytes.fromhex(token_asset_name_hex)
        for v in rewards.values():
            if v.multi_asset:
                policy_payload = token_policy_id.payload
                if policy_payload in v.multi_asset:
                    total_distributed_tokens += v.multi_asset[policy_payload].get(
                        asset_name_bytes, 0
                    )
            total_distributed_ada += v.coin

    logger.info(
        f"Total distributed: {total_distributed_tokens} tokens, {total_distributed_ada / ADA_TO_LOVELACE:.6f} ADA to {len(rewards)} miners."
    )

    return rewards


async def build_distribution_transaction(
    context: BlockFrostChainContext,
    settings: Settings,
    distributor_address: Address,
    distributor_skey: ExtendedSigningKey,
    rewards: Dict[str, Value],
    token_policy_id: ScriptHash,
    token_asset_name_hex: str,
) -> Optional[TransactionBuilder]:
    """
    Builds the transaction to distribute rewards by minting tokens.
    Does NOT handle validator state updates anymore.
    """
    logger.info("Building distribution transaction...")
    builder = TransactionBuilder(context)
    builder.add_input_address(distributor_address)

    # --- Add Reward Outputs ---
    total_ada_needed = 0
    total_tokens_needed = 0
    asset_name_bytes = bytes.fromhex(token_asset_name_hex)
    for address_str, value in rewards.items():
        try:
            reward_address = Address.from_primitive(address_str)
            builder.add_output(TransactionOutput(reward_address, value))
            total_ada_needed += value.coin
            tokens_in_value = 0
            if value.multi_asset and token_policy_id.payload in value.multi_asset:
                tokens_in_value = value.multi_asset[token_policy_id.payload].get(
                    asset_name_bytes, 0
                )
            total_tokens_needed += tokens_in_value
            logger.debug(
                f"Adding output: {tokens_in_value} tokens and {value.coin / ADA_TO_LOVELACE:.6f} ADA to {address_str}"
            )
        except Exception as e:
            logger.error(f"Failed to add reward output for {address_str}: {e}")
            return None

    # --- Add Minting ---
    try:
        policy_id = ScriptHash.from_primitive(
            bytes.fromhex(settings.MINTING_POLICY_ID_PLACEHOLDER)
        )
        if policy_id != token_policy_id:
            logger.warning(
                f"Policy ID mismatch: from settings ({policy_id}) vs derived ({token_policy_id})"
            )
            policy_id = token_policy_id

        pub_key_hash = distributor_skey.to_verification_key().hash()
        script = ScriptPubkey(key_hash=pub_key_hash)
        mint_assets = MultiAsset.from_primitive(
            {policy_id.payload: {asset_name_bytes: total_tokens_needed}}
        )
        builder.mint = mint_assets
        builder.native_scripts = [script]
        logger.info(
            f"Adding mint action: {total_tokens_needed} tokens with policy {policy_id}"
        )
    except Exception as e:
        logger.error(f"Failed to prepare minting action: {e}")
        return None

    # --- Add Required Signer ---
    builder.required_signers = [distributor_skey.to_verification_key().hash()]
    logger.info("Adding distributor key hash as required signer.")

    # Set TTL
    builder.ttl = context.last_block_slot + 3600  # Expires in 1 hour

    logger.info("Transaction build process complete (pre-fee calculation).")
    return builder


async def main():
    """Main function to run the distribution script."""
    logger.info("--- Starting Reward Distribution ---")

    try:
        # --- Configuration ---
        bf_project_id = os.getenv("BLOCKFROST_PROJECT_ID")
        if not bf_project_id:
            bf_project_id = (
                settings.BLOCKFROST_PROJECT_ID
            )  # Use from settings if env var not set

        # Determine network and BF URL from settings
        network_str = settings.CARDANO_NETWORK.upper()
        if network_str == "MAINNET":
            network_enum = CardanoNetwork.MAINNET
            bf_base_url = ApiUrls.mainnet.value
        elif network_str == "PREPROD":
            network_enum = CardanoNetwork.PREPROD  # type: ignore[attr-defined]
            bf_base_url = ApiUrls.preprod.value
        elif network_str == "PREVIEW":
            network_enum = CardanoNetwork.PREVIEW  # type: ignore[attr-defined]
            bf_base_url = ApiUrls.preview.value
        else:  # Default to testnet (legacy testnet)
            network_enum = CardanoNetwork.TESTNET
            bf_base_url = ApiUrls.testnet.value

        # Use HOTKEY_BASE_DIR
        base_dir = settings.HOTKEY_BASE_DIR
        context = BlockFrostChainContext(bf_project_id, base_url=bf_base_url)

        coldkey_name = settings.COLDKEY_NAME or DEFAULT_WALLET_NAME
        logger.info(f"Using Network: {network_enum.name}")
        logger.info(f"Using Base Directory: {base_dir}")
        logger.info(f"Using Coldkey: {coldkey_name}")

        # --- Get Distributor Info ---
        distributor_address, distributor_skey = await get_distributor_signing_info(
            settings, context
        )

        # --- Fetch ALL Miner Data instead of Validator State ---
        logger.info("Fetching all miner data from contract...")
        try:
            # Assuming TEST_CONTRACT_ADDRESS is the miner registration contract
            miner_contract_hash = ScriptHash.from_primitive(
                settings.TEST_CONTRACT_ADDRESS
            )
            # Call get_all_miner_data
            all_miner_info: List[Tuple[UTxO, Dict[str, Any]]] = await get_all_miner_data(  # type: ignore[name-defined]
                context, miner_contract_hash, network_enum  # Use network_enum
            )
            if not all_miner_info:
                logger.warning(
                    "No miner data found at the contract address. Cannot distribute rewards."
                )
                return
            # Extract just the data dictionaries
            miner_data_list = [data for utxo, data in all_miner_info]
            logger.info(f"Found data for {len(miner_data_list)} miners.")

        except Exception as e:
            logger.error(f"Failed to retrieve miner data: {e}", exc_info=True)
            return

        # --- Token Information ---
        # Use ScriptHash.from_primitive here
        token_policy_id = ScriptHash.from_primitive(
            bytes.fromhex(settings.MINTING_POLICY_ID_PLACEHOLDER)
        )
        # Use correct Settings attribute name for Token Name
        token_name_str = settings.TOKEN_NAME_STR
        token_asset_name_hex = token_name_str.encode("utf-8").hex()

        # TOTAL_REWARD_AMOUNT handling - Keep using default if attribute missing
        # Add type ignore for potential missing attribute
        if hasattr(settings, "TOTAL_REWARD_AMOUNT"):
            total_reward_amount = int(settings.TOTAL_REWARD_AMOUNT)  # type: ignore[attr-defined]
        else:
            logger.warning(
                "TOTAL_REWARD_AMOUNT not found in settings. Using default value 1000000."
            )
            total_reward_amount = 1000000  # Example default value - ADJUST AS NEEDED

        logger.info(f"Reward Token Policy ID: {token_policy_id}")
        logger.info(
            f"Reward Token Name: {token_name_str} (Hex: {token_asset_name_hex})"
        )
        logger.info(f"Total Reward Amount to Distribute: {total_reward_amount}")

        # --- Calculate Rewards ---
        # Pass the list of miner data dicts and context
        rewards = calculate_rewards(
            miner_data_list,
            total_reward_amount,
            token_asset_name_hex,
            token_policy_id,
            context,
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
            # Simplify Cardanoscan URL generation
            scan_subdomain = ""
            if network_enum == CardanoNetwork.PREPROD:  # type: ignore[attr-defined]
                scan_subdomain = "preprod."
            elif network_enum == CardanoNetwork.PREVIEW:  # type: ignore[attr-defined]
                scan_subdomain = "preview."
            elif network_enum == CardanoNetwork.TESTNET:
                scan_subdomain = "testnet."

            logger.info(
                f"View on Cardanoscan ({network_enum.name}): https://{scan_subdomain}cardanoscan.io/transaction/{tx_hash}"
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
