# scripts/run_minter.py
"""
Separate process to monitor mint trigger file and execute token minting.
Uses its own dedicated hotkey/wallet.
"""
import asyncio
import json
import logging
import os
import time
from typing import Optional

from pycardano import (
    Address,
    BlockFrostChainContext,
    ExtendedSigningKey,
    Network,
    TransactionId,
)

# --- SDK Imports ---
# Assuming settings are accessible globally or loaded differently here
# If settings are not directly importable, load necessary values from env vars
try:
    from sdk.config.settings import settings

    NETWORK_STR = settings.CARDANO_NETWORK
    TOKEN_NAME_STR = settings.TOKEN_NAME_STR
    MINTING_POLICY_SCRIPT_CBOR_HEX = settings.MINTING_POLICY_SCRIPT_CBOR_HEX
    BLOCKFROST_PROJECT_ID = settings.BLOCKFROST_PROJECT_ID  # Assumes this is available
    # Get trigger file path from validator settings or default
    MINT_TRIGGER_FILE_PATH = getattr(
        settings, "MINT_TRIGGER_FILE_PATH", "mint_trigger.json"
    )

except ImportError:
    # Fallback to environment variables if settings module not found/usable
    print(
        "WARNING: Could not import settings directly. Falling back to environment variables."
    )
    NETWORK_STR = os.getenv("CARDANO_NETWORK", "TESTNET")
    TOKEN_NAME_STR = os.getenv("TOKEN_NAME_STR", "MOD")  # Default token name
    MINTING_POLICY_SCRIPT_CBOR_HEX = os.getenv("MINTING_POLICY_SCRIPT_CBOR_HEX")
    BLOCKFROST_PROJECT_ID = os.getenv("BLOCKFROST_PROJECT_ID")
    MINT_TRIGGER_FILE_PATH = os.getenv("MINT_TRIGGER_FILE_PATH", "mint_trigger.json")

# --- Environment Variables Specific to Minter ---
MINTER_HOTKEY_BASE_DIR = os.getenv(
    "MINTER_HOTKEY_BASE_DIR",
    settings.HOTKEY_BASE_DIR if "settings" in locals() else "./keys",
)  # Default or validator's base dir
MINTER_COLDKEY_NAME = os.getenv(
    "MINTER_COLDKEY_NAME", "minter_cold"
)  # Use a distinct coldkey name
MINTER_HOTKEY_NAME = os.getenv(
    "MINTER_HOTKEY_NAME", "minter_hot"
)  # Use a distinct hotkey name
MINTER_HOTKEY_PASSWORD = os.getenv(
    "MINTER_HOTKEY_PASSWORD"
)  # Password for the *minter's* hotkey
MINTER_RECIPIENT_ADDRESS = os.getenv(
    "MINTER_RECIPIENT_ADDRESS"
)  # Address to receive minted tokens
MINTER_STATE_FILE = os.getenv("MINTER_STATE_FILE", "minter_state.json")
MINTER_CHECK_INTERVAL_SECONDS = int(
    os.getenv("MINTER_CHECK_INTERVAL_SECONDS", "30")
)  # Check every 30s

# --- SDK Function Imports ---
from sdk.keymanager.decryption_utils import decode_hotkey_skey
from sdk.service.context import get_chain_context
from sdk.smartcontract.minting import mint_native_tokens  # Import the minting function

# --- Logging Setup ---
logging.basicConfig(
    level=logging.INFO, format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger("MinterScript")

# --- Cardano Network ---
NETWORK = Network.TESTNET if NETWORK_STR == "TESTNET" else Network.MAINNET


def load_last_minted_cycle() -> int:
    """Loads the last cycle number for which minting was successful."""
    try:
        if os.path.exists(MINTER_STATE_FILE):
            with open(MINTER_STATE_FILE, "r") as f:
                state_data = json.load(f)
                last_cycle = state_data.get("last_minted_cycle", -1)
                logger.info(f"Loaded minter state: last_minted_cycle={last_cycle}")
                return last_cycle
        else:
            logger.warning(
                f"Minter state file '{MINTER_STATE_FILE}' not found. Starting from cycle -1."
            )
            return -1
    except Exception as e:
        logger.error(
            f"Error loading minter state file '{MINTER_STATE_FILE}': {e}. Starting from -1."
        )
        return -1


def save_last_minted_cycle(cycle_number: int):
    """Saves the last successfully minted cycle number."""
    state_data = {"last_minted_cycle": cycle_number}
    try:
        os.makedirs(os.path.dirname(MINTER_STATE_FILE) or ".", exist_ok=True)
        with open(MINTER_STATE_FILE, "w") as f:
            json.dump(state_data, f, indent=2)
        logger.info(f"Saved minter state: last_minted_cycle={cycle_number}")
    except Exception as e:
        logger.error(f"Error saving minter state to '{MINTER_STATE_FILE}': {e}")


async def run_minter_check(last_processed_cycle: int) -> int:
    """
    Checks the trigger file and initiates minting if necessary.
    Returns the latest cycle processed (either the old one or the new one if minting was successful).
    """
    logger.debug(f"Checking trigger file: {MINT_TRIGGER_FILE_PATH}")
    new_processed_cycle = last_processed_cycle

    try:
        if not os.path.exists(MINT_TRIGGER_FILE_PATH):
            logger.debug(f"Mint trigger file not found yet.")
            return new_processed_cycle  # Return old cycle number

        with open(MINT_TRIGGER_FILE_PATH, "r") as f:
            trigger_data = json.load(f)

        trigger_cycle = trigger_data.get("last_completed_cycle", -1)
        issuance_amount = trigger_data.get("issuance_amount", 0)

        logger.debug(
            f"Trigger file data: Cycle={trigger_cycle}, Amount={issuance_amount}"
        )

        # --- Decision Logic ---
        if trigger_cycle > last_processed_cycle and issuance_amount > 0:
            logger.info(
                f"New mint trigger detected for Cycle {trigger_cycle}. Amount: {issuance_amount}"
            )

            # --- Perform Minting ---
            minter_signing_key: Optional[ExtendedSigningKey] = None
            minter_stake_signing_key: Optional[ExtendedSigningKey] = None
            minting_context: Optional[BlockFrostChainContext] = None

            try:
                # 1. Decode Minter's Key
                if not MINTER_HOTKEY_PASSWORD:
                    raise ValueError(
                        "MINTER_HOTKEY_PASSWORD environment variable not set."
                    )
                minter_signing_key, minter_stake_signing_key = decode_hotkey_skey(
                    base_dir=MINTER_HOTKEY_BASE_DIR,
                    coldkey_name=MINTER_COLDKEY_NAME,
                    hotkey_name=MINTER_HOTKEY_NAME,
                    password=MINTER_HOTKEY_PASSWORD,
                )
                if not minter_signing_key:
                    raise ValueError("Failed to decode minter's payment signing key.")
                logger.info(
                    f"Minter keys decoded successfully for hotkey '{MINTER_HOTKEY_NAME}'."
                )

                # 2. Get Cardano Context
                minting_context = get_chain_context(method="blockfrost")
                if not minting_context:
                    raise RuntimeError("Failed to get Cardano context for minting.")

                # 3. Determine Recipient Address
                if not MINTER_RECIPIENT_ADDRESS:
                    raise ValueError(
                        "MINTER_RECIPIENT_ADDRESS environment variable not set."
                    )
                try:
                    recipient_addr = Address.from_primitive(MINTER_RECIPIENT_ADDRESS)
                except Exception as addr_err:
                    raise ValueError(
                        f"Invalid MINTER_RECIPIENT_ADDRESS format: {addr_err}"
                    ) from addr_err

                # 4. Check necessary minting config
                if not MINTING_POLICY_SCRIPT_CBOR_HEX:
                    raise ValueError("MINTING_POLICY_SCRIPT_CBOR_HEX not configured.")
                if not TOKEN_NAME_STR:
                    raise ValueError("TOKEN_NAME_STR not configured.")

                # 5. Call Mint Function
                logger.info(
                    f"Initiating minting of {issuance_amount} {TOKEN_NAME_STR} for cycle {trigger_cycle}..."
                )
                mint_tx_id: Optional[TransactionId] = await mint_native_tokens(
                    context=minting_context,
                    signing_key=minter_signing_key,
                    stake_signing_key=minter_stake_signing_key,
                    policy_script_cbor_hex=MINTING_POLICY_SCRIPT_CBOR_HEX,
                    asset_name_str=TOKEN_NAME_STR,
                    amount_to_mint=issuance_amount,
                    recipient_address=recipient_addr,
                    network=NETWORK,
                    excluded_tx_id=None,  # IMPORTANT: Do not exclude any tx for the minter
                )

                if mint_tx_id:
                    logger.info(
                        f"Successfully submitted minting transaction for cycle {trigger_cycle}. TxID: {mint_tx_id}"
                    )
                    # Update state ONLY if minting tx was submitted
                    new_processed_cycle = trigger_cycle
                    save_last_minted_cycle(new_processed_cycle)
                else:
                    logger.error(
                        f"Minting transaction failed to submit for cycle {trigger_cycle}. Will retry on next check."
                    )
                    # Do not update new_processed_cycle, so it retries

            except Exception as mint_exec_err:
                logger.exception(
                    f"Error during minting execution for cycle {trigger_cycle}: {mint_exec_err}"
                )
                # Do not update state, retry next time

        else:
            if trigger_cycle <= last_processed_cycle:
                logger.debug(
                    f"Trigger cycle {trigger_cycle} already processed (last was {last_processed_cycle}). Skipping."
                )
            elif issuance_amount <= 0:
                logger.debug(
                    f"Issuance amount for trigger cycle {trigger_cycle} is zero. Skipping mint."
                )

    except FileNotFoundError:
        logger.debug(f"Mint trigger file '{MINT_TRIGGER_FILE_PATH}' not found.")
    except json.JSONDecodeError:
        logger.error(
            f"Error decoding JSON from trigger file '{MINT_TRIGGER_FILE_PATH}'."
        )
    except Exception as e:
        logger.exception(f"An unexpected error occurred during minter check: {e}")

    return new_processed_cycle


async def main():
    """Main loop for the minter script."""
    logger.info("--- Starting Minter Script ---")
    if not BLOCKFROST_PROJECT_ID:
        logger.error("BLOCKFROST_PROJECT_ID environment variable not set. Exiting.")
        return
    # Validate other essential configs early?

    last_minted = load_last_minted_cycle()

    try:
        while True:
            last_minted = await run_minter_check(last_minted)
            logger.debug(
                f"Waiting {MINTER_CHECK_INTERVAL_SECONDS} seconds for next check..."
            )
            await asyncio.sleep(MINTER_CHECK_INTERVAL_SECONDS)
    except asyncio.CancelledError:
        logger.info("Minter script cancelled.")
    except KeyboardInterrupt:
        logger.info("Minter script interrupted by user.")
    finally:
        logger.info("--- Stopping Minter Script ---")


if __name__ == "__main__":
    # Basic check for necessary env vars before running
    required_vars = [
        "MINTER_HOTKEY_PASSWORD",
        "MINTER_RECIPIENT_ADDRESS",
        "BLOCKFROST_PROJECT_ID",
        "MINTING_POLICY_SCRIPT_CBOR_HEX",
        # Add others as needed: MINTER_HOTKEY_BASE_DIR, MINTER_COLDKEY_NAME, MINTER_HOTKEY_NAME
    ]
    missing_vars = [var for var in required_vars if not os.getenv(var)]
    if missing_vars:
        print(
            f"ERROR: Missing required environment variables: {', '.join(missing_vars)}"
        )
        exit(1)

    asyncio.run(main())
