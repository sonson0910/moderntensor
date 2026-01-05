# File: scripts/prepare_testnet_datums.py
# Chuẩn bị và gửi các Datum ban đầu cho Miners và Validators lên Cardano Testnet.
# Dựa trên logic từ old.py: Sử dụng context, builder trực tiếp và read_validator().

import os
import sys
import logging
import asyncio
import hashlib
from pathlib import Path
from dotenv import load_dotenv
from typing import Optional, Tuple, List, Type, Dict, Any, cast
from rich.logging import RichHandler

from sdk.compat.pycardano import (
    TransactionBuilder,
    TransactionOutput,
    Address,
    ScriptHash,
    PlutusData,
    BlockFrostChainContext,
    Network,
    TransactionId,
    ExtendedSigningKey,
    UTxO,
    Value,
)

# --- Thêm đường dẫn gốc của project vào sys.path ---
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))
# -------------------------------------------------

# Import các thành phần cần thiết từ SDK Moderntensor
try:
    from sdk.metagraph.create_utxo import find_suitable_ada_input  # Dùng lại hàm này
    from sdk.metagraph.metagraph_datum import MinerDatum, ValidatorDatum, STATUS_ACTIVE
    from sdk.core.datatypes import MinerInfo, ValidatorInfo  # Import từ datatypes
    from sdk.metagraph.hash.hash_datum import (
        hash_data,
    )  # Dùng để hash wallet addr nếu cần
    from sdk.service.context import get_chain_context  # Lấy context trực tiếp
    from sdk.keymanager.decryption_utils import decode_hotkey_skey
    from sdk.smartcontract.validator import (
        read_validator,
    )  # <<<--- Import read_validator
    from sdk.config.settings import settings as sdk_settings

    # Không dùng BlockFrostService và TransactionService nữa
except ImportError as e:
    print(f"❌ FATAL: Import Error in prepare_testnet_datums.py: {e}")
    sys.exit(1)

# --- Tải biến môi trường ---
env_path = project_root / ".env"

# --- Cấu hình Logging với RichHandler ---
log_level_str = os.getenv("LOG_LEVEL", "INFO").upper()
log_level = getattr(logging, log_level_str, logging.INFO)
rich_handler = RichHandler(
    show_time=True,
    show_level=True,
    show_path=False,
    markup=True,
    rich_tracebacks=True,
    log_time_format="[%Y-%m-%d %H:%M:%S]",
)
logging.basicConfig(
    level=log_level, format="%(message)s", datefmt="[%X]", handlers=[rich_handler]
)
logger = logging.getLogger(__name__)
# ------------------------

# --- Tải biến môi trường (sau khi logger được cấu hình) ---
if env_path.exists():
    logger.info(f"📄 Loading environment variables from: {env_path}")
    load_dotenv(dotenv_path=env_path, override=True)
    # <<<--- THÊM DEBUG --- >>>
    logger.debug(
        f"DEBUG: Value of HOTKEY_BASE_DIR from os.getenv AFTER load: {os.getenv('HOTKEY_BASE_DIR')}"
    )
    # <<<----------------->>>
else:
    logger.warning(f"📄 Environment file (.env) not found at {env_path}.")
# --------------------------

# --- Lấy cấu hình ví Funding ---
FUNDING_COLDKEY_NAME = os.getenv("FUNDING_COLDKEY_NAME", "kickoff")
FUNDING_HOTKEY_NAME = os.getenv("FUNDING_HOTKEY_NAME", "hk1")
FUNDING_PASSWORD = os.getenv("FUNDING_PASSWORD")
HOTKEY_BASE_DIR = os.getenv(
    "HOTKEY_BASE_DIR", getattr(sdk_settings, "HOTKEY_BASE_DIR", "moderntensor")
)
# <<<--- THÊM DEBUG --- >>>
logger.debug(
    f"DEBUG: Final value assigned to HOTKEY_BASE_DIR variable: {HOTKEY_BASE_DIR} (Type: {type(HOTKEY_BASE_DIR)})"
)
# <<<----------------->>>

# ADA amount for each datum UTXO
DATUM_LOCK_AMOUNT = 2_000_000  # 2 ADA

# --- Kiểm tra các biến môi trường bắt buộc ---
required_env_vars = {
    "SUBNET1_MINER_ID": os.getenv("SUBNET1_MINER_ID"),
    "SUBNET1_VALIDATOR_UID": os.getenv("SUBNET1_VALIDATOR_UID"),
    "FUNDING_COLDKEY_NAME": FUNDING_COLDKEY_NAME,
    "FUNDING_HOTKEY_NAME": FUNDING_HOTKEY_NAME,
    "FUNDING_PASSWORD": FUNDING_PASSWORD,
    # Không cần kiểm tra BLOCKFROST_PROJECT_ID ở đây nữa
    # Không cần kiểm tra METAGRAPH_SCRIPT_ADDRESS ở đây nữa
}
missing_vars = [k for k, v in required_env_vars.items() if not v]
if missing_vars:
    logger.error(
        f"FATAL: Missing required environment variables: {', '.join(missing_vars)}"
    )
    sys.exit(1)
# --------------------------------------------------

# === Constants ===
SUBNET_ID_TO_USE = 1


# === Helper Functions ===
def load_funding_keys(
    base_dir: str, coldkey_name: str, hotkey_name: str, password: str
) -> Tuple[ExtendedSigningKey, Address]:
    """Loads funding keys and derives the address."""
    logger.info(
        f"🔑 Loading funding keys (Cold: '{coldkey_name}', Hot: '{hotkey_name}')..."
    )
    try:
        payment_esk, stake_esk = decode_hotkey_skey(
            base_dir, coldkey_name, hotkey_name, password
        )
        if not payment_esk:
            raise ValueError("Failed to decode payment key")
        payment_vk = payment_esk.to_verification_key()
        stake_vk = stake_esk.to_verification_key() if stake_esk else None
        funding_address = Address(
            payment_vk.hash(),
            stake_vk.hash() if stake_vk else None,
            network=get_network(),
        )
        logger.info(f"✅ Funding keys loaded. Address: [cyan]{funding_address}[/]")
        return payment_esk, funding_address
    except Exception as e:
        logger.exception(f"💥 Failed to load funding keys: {e}")
        raise


def get_network() -> Network:
    """Determines the Cardano network from environment or SDK settings."""
    network_str = (
        os.getenv("CARDANO_NETWORK")
        or getattr(sdk_settings, "CARDANO_NETWORK", "TESTNET")
    ).upper()
    return Network.MAINNET if network_str == "MAINNET" else Network.TESTNET


# === Main Async Function ===
async def prepare_datums():
    """Main async function to prepare and submit initial datums."""
    logger.info(
        "✨ --- Starting Metagraph Datum Preparation Script (using direct context/builder/read_validator) --- ✨"
    )
    current_slot = 0  # Initialize

    # --- Load Configuration (chỉ cần load key info) ---
    logger.info("⚙️ Loading funding key configuration from .env...")
    try:
        network = get_network()
        hotkey_base_dir = os.getenv(
            "HOTKEY_BASE_DIR", getattr(sdk_settings, "HOTKEY_BASE_DIR", "moderntensor")
        )
        funding_coldkey = os.getenv("FUNDING_COLDKEY_NAME", "kickoff")
        funding_hotkey = os.getenv("FUNDING_HOTKEY_NAME", "hk1")
        funding_password = os.getenv("FUNDING_PASSWORD")

        if not funding_coldkey:
            raise ValueError(
                "FUNDING_COLDKEY_NAME is missing or empty in .env and no default specified."
            )
        if not funding_hotkey:
            raise ValueError(
                "FUNDING_HOTKEY_NAME is missing or empty in .env and no default specified."
            )
        if not funding_password:
            raise ValueError("FUNDING_PASSWORD environment variable is not set.")
        if not hotkey_base_dir:
            raise ValueError(
                "Could not determine HOTKEY_BASE_DIR (checked .env and sdk_settings)."
            )

    except Exception as cfg_err:
        logger.exception(f"💥 Error loading key configuration: {cfg_err}")
        sys.exit(1)

    # --- Initialize Cardano Context ---
    context: Optional[BlockFrostChainContext] = None
    script_hash: Optional[ScriptHash] = None
    contract_address: Optional[Address] = None
    try:
        logger.info(
            f"🔗 Initializing BlockFrostChainContext (Network: {network.name})..."
        )
        context = get_chain_context(method="blockfrost")  # type: ignore
        if not context:
            raise RuntimeError("Failed to initialize BlockFrost context.")
        if context.network != network:
            logger.warning(
                f"Context network ({context.network.name}) differs from settings ({network.name}). Using context network."
            )
            network = context.network

        min_utxo = context.protocol_param.min_utxo
        logger.info(
            f"💰 Minimum UTXO Value per Datum (Lovelace): {DATUM_LOCK_AMOUNT} (Protocol Min: {min_utxo})"
        )
        if DATUM_LOCK_AMOUNT < min_utxo:
            logger.warning(
                f"⚠️ DATUM_LOCK_AMOUNT ({DATUM_LOCK_AMOUNT}) is less than protocol minimum ({min_utxo}). Transaction might fail."
            )

        # --- Lấy Script Hash và Địa chỉ Contract từ read_validator ---
        logger.info("📜 Reading validator script details...")
        validator_details = read_validator()  # Gọi hàm đọc script validator
        if not validator_details or "script_hash" not in validator_details:
            raise RuntimeError(
                "Could not load validator script hash using read_validator()."
            )
        script_hash = validator_details["script_hash"]
        contract_address = Address(
            payment_part=script_hash, network=network
        )  # Tạo địa chỉ từ hash
        logger.info(f"🎯 Target Contract Address: [magenta]{contract_address}[/]")
        logger.info(f"   Script Hash: {script_hash}")
        # -------------------------------------------------------------

        # Get current slot if needed (optional)
        # current_slot = context.last_block_slot
        # logger.info(f"ℹ️ Current blockchain slot (approx): {current_slot}")

    except Exception as ctx_err:
        logger.exception(
            f"💥 Error initializing Cardano context or reading validator script: {ctx_err}"
        )
        sys.exit(1)

    # --- Load Funding Wallet ---
    funding_skey: Optional[ExtendedSigningKey] = None
    funding_address: Optional[Address] = None
    try:
        funding_skey, funding_address = load_funding_keys(
            hotkey_base_dir, funding_coldkey, funding_hotkey, funding_password  # type: ignore
        )
        balance = context.utxos(str(funding_address))
        total_balance = sum(utxo.output.amount.coin for utxo in balance)
        logger.info(f"💰 Funding Wallet Balance: {total_balance / 1_000_000} ADA")
    except Exception as fund_err:
        logger.exception(f"💥 Error loading funding wallet: {fund_err}")
        sys.exit(1)

    # --- Load Initial Validators and Miners Info from .env ---
    logger.info("👥 Loading initial validator and miner configurations from .env...")
    validators_info: List[ValidatorInfo] = []
    miners_info: List[MinerInfo] = []
    try:
        # Load Validator 1 info
        val1_uid = os.getenv("SUBNET1_VALIDATOR_UID")
        val1_addr = os.getenv("SUBNET1_VALIDATOR_ADDRESS")
        val1_api = os.getenv("SUBNET1_VALIDATOR_API_ENDPOINT")
        if val1_uid and val1_addr and val1_api:
            validators_info.append(
                ValidatorInfo(
                    uid=val1_uid.encode().hex(),
                    address=val1_addr,
                    api_endpoint=val1_api,
                    status=STATUS_ACTIVE,
                )
            )
            logger.info(f"  ➕ Loaded Validator 1: UID={val1_uid}")
        else:
            logger.warning("⚠️ Validator 1 info missing in .env, skipping.")

        # Load Validator 2 info
        val2_uid = os.getenv("SUBNET1_VALIDATOR_UID2")
        val2_addr = os.getenv("SUBNET1_VALIDATOR_ADDRESS2")
        val2_api = os.getenv("SUBNET1_VALIDATOR_API_ENDPOINT2")
        if val2_uid and val2_addr and val2_api:
            validators_info.append(
                ValidatorInfo(
                    uid=val2_uid.encode().hex(),
                    address=val2_addr,
                    api_endpoint=val2_api,
                    status=STATUS_ACTIVE,
                )
            )
            logger.info(f"  ➕ Loaded Validator 2: UID={val2_uid}")
        else:
            logger.warning("⚠️ Validator 2 info missing in .env, skipping.")

        # Load Validator 3 info
        val3_uid = os.getenv("SUBNET1_VALIDATOR_UID3")
        val3_addr = os.getenv("SUBNET1_VALIDATOR_ADDRESS3")
        val3_api = os.getenv("SUBNET1_VALIDATOR_API_ENDPOINT3")
        if val3_uid and val3_addr and val3_api:
            validators_info.append(
                ValidatorInfo(
                    uid=val3_uid.encode().hex(),
                    address=val3_addr,
                    api_endpoint=val3_api,
                    status=STATUS_ACTIVE,
                )
            )
            logger.info(f"  ➕ Loaded Validator 3: UID={val3_uid}")
        else:
            logger.warning("⚠️ Validator 3 info missing in .env, skipping.")

        # Load Miner 1 info
        miner1_uid = os.getenv("SUBNET1_MINER_ID")
        miner1_addr = os.getenv("SUBNET1_MINER_WALLET_ADDR")
        miner1_api = os.getenv("SUBNET1_MINER_API_ENDPOINT")
        if miner1_uid and miner1_addr and miner1_api:
            miners_info.append(
                MinerInfo(
                    uid=miner1_uid.encode().hex(),
                    address=miner1_addr,
                    api_endpoint=miner1_api,
                    status=STATUS_ACTIVE,
                )
            )
            logger.info(f"  ➕ Loaded Miner 1: ID={miner1_uid}")
        else:
            logger.warning("⚠️ Miner 1 info missing in .env, skipping.")

        # Load Miner 2 info
        miner2_uid = os.getenv("SUBNET1_MINER_ID2")
        miner2_addr = os.getenv("SUBNET1_MINER_WALLET_ADDR2")
        miner2_api = os.getenv("SUBNET1_MINER_API_ENDPOINT2")
        if miner2_uid and miner2_addr and miner2_api:
            miners_info.append(
                MinerInfo(
                    uid=miner2_uid.encode().hex(),
                    address=miner2_addr,
                    api_endpoint=miner2_api,
                    status=STATUS_ACTIVE,
                )
            )
            logger.info(f"  ➕ Loaded Miner 2: ID={miner2_uid}")
        else:
            logger.warning("⚠️ Miner 2 info missing in .env, skipping.")

        if not validators_info and not miners_info:
            logger.critical(
                "❌ FATAL: No validators or miners could be loaded from .env configuration. Cannot create datums."
            )
            sys.exit(1)

    except Exception as list_err:
        logger.exception(f"💥 Error loading validator/miner lists: {list_err}")
        sys.exit(1)

    # --- Validate Loaded Endpoints ---
    logger.info("🔎 Validating loaded API endpoints...")
    endpoints_to_validate = {
        "Validator 1 API": os.getenv("SUBNET1_VALIDATOR_API_ENDPOINT"),
        "Validator 2 API": os.getenv("SUBNET1_VALIDATOR_API_ENDPOINT2"),
        "Validator 3 API": os.getenv("SUBNET1_VALIDATOR_API_ENDPOINT3"),
        "Miner 1 API": os.getenv("SUBNET1_MINER_API_ENDPOINT"),
        "Miner 2 API": os.getenv("SUBNET1_MINER_API_ENDPOINT2"),
    }
    invalid_endpoints = False
    for name, endpoint in endpoints_to_validate.items():
        if endpoint:
            # Basic check: starts with http and is a string
            if not isinstance(endpoint, str) or not endpoint.startswith(
                ("http://", "https://")
            ):
                logger.error(
                    f"  ❌ Invalid endpoint format for {name}: '{endpoint[:50]}...'"
                )
                invalid_endpoints = True
            else:
                logger.info(f"  ✅ Valid endpoint format for {name}: {endpoint}")
        # Allow missing endpoints for optional entities, but log warning
        elif name in ["Validator 2 API", "Validator 3 API", "Miner 2 API"]:
            logger.warning(f"  ⚠️ Optional {name} not set in .env.")
        # Require endpoint for Validator 1 and Miner 1 if their UIDs/IDs are set
        elif name == "Validator 1 API" and os.getenv("SUBNET1_VALIDATOR_UID"):
            logger.error(f"  ❌ Required {name} is missing in .env!")
            invalid_endpoints = True
        elif name == "Miner 1 API" and os.getenv("SUBNET1_MINER_ID"):
            logger.error(f"  ❌ Required {name} is missing in .env!")
            invalid_endpoints = True

    if invalid_endpoints:
        logger.critical(
            "❌ FATAL: Invalid or missing required API endpoints detected in .env configuration. Please fix and retry."
        )
        sys.exit(1)
    logger.info("✅ All loaded API endpoints validated successfully.")
    # --- End Validation ---

    # --- Create Individual Datums and Outputs ---
    logger.info("🔩 Creating individual Datum objects and Transaction Outputs...")
    outputs_to_create: List[TransactionOutput] = []
    total_output_value = 0
    try:
        # Create Validator Datums
        for info in validators_info:
            try:
                wallet_hash_bytes = b""  # Placeholder
                val_datum = ValidatorDatum(
                    uid=bytes.fromhex(info.uid),
                    subnet_uid=SUBNET_ID_TO_USE,
                    stake=0,
                    scaled_last_performance=0,
                    scaled_trust_score=0,
                    accumulated_rewards=0,
                    last_update_slot=current_slot,
                    performance_history_hash=b"",
                    wallet_addr_hash=wallet_hash_bytes,
                    status=STATUS_ACTIVE,
                    registration_slot=current_slot,
                    api_endpoint=(
                        info.api_endpoint.encode("utf-8") if info.api_endpoint else b""
                    ),
                )
                tx_out = TransactionOutput(
                    contract_address, Value(DATUM_LOCK_AMOUNT), datum=val_datum
                )  # Sử dụng contract_address đã lấy
                outputs_to_create.append(tx_out)
                total_output_value += DATUM_LOCK_AMOUNT
                logger.info(
                    f"  📄 Prepared ValidatorDatum UTXO for UID: {info.uid[:10]}..."
                )
            except Exception as datum_err:
                logger.error(
                    f"  ❌ Failed to create ValidatorDatum for UID {info.uid}: {datum_err}"
                )

        # Create Miner Datums
        for info in miners_info:
            try:
                wallet_hash_bytes = b""  # Placeholder
                miner_datum = MinerDatum(
                    uid=bytes.fromhex(info.uid),
                    subnet_uid=SUBNET_ID_TO_USE,
                    stake=0,
                    scaled_last_performance=0,
                    scaled_trust_score=0,
                    accumulated_rewards=0,
                    last_update_slot=current_slot,
                    performance_history_hash=b"",
                    wallet_addr_hash=wallet_hash_bytes,
                    status=STATUS_ACTIVE,
                    registration_slot=current_slot,
                    api_endpoint=(
                        info.api_endpoint.encode("utf-8") if info.api_endpoint else b""
                    ),
                )
                tx_out = TransactionOutput(
                    contract_address, Value(DATUM_LOCK_AMOUNT), datum=miner_datum
                )  # Sử dụng contract_address đã lấy
                outputs_to_create.append(tx_out)
                total_output_value += DATUM_LOCK_AMOUNT
                logger.info(
                    f"  📄 Prepared MinerDatum UTXO for UID: {info.uid[:10]}..."
                )
            except Exception as datum_err:
                logger.error(
                    f"  ❌ Failed to create MinerDatum for UID {info.uid}: {datum_err}"
                )

        if not outputs_to_create:
            logger.critical("❌ FATAL: No valid datum outputs could be created.")
            sys.exit(1)

        logger.info(f"✅ Prepared {len(outputs_to_create)} total datum outputs.")

    except Exception as create_err:
        logger.exception(f"💥 Error during datum/output creation: {create_err}")
        sys.exit(1)

    # --- Find Suitable Input UTXO ---
    estimated_fee_buffer = 500_000  # 0.5 ADA buffer for fees
    min_input_needed = total_output_value + estimated_fee_buffer
    logger.info(
        f"🔍 Searching for input UTXO >= {min_input_needed / 1_000_000:.6f} ADA in funding wallet..."
    )

    selected_input_utxo = find_suitable_ada_input(
        context, str(funding_address), min_input_needed
    )

    if not selected_input_utxo:
        logger.error(
            f"❌ Could not find a suitable ADA-only UTxO with at least {min_input_needed / 1_000_000:.6f} ADA at {funding_address}."
        )
        logger.error(
            "   Ensure the funding wallet has enough ADA in a single, simple UTxO."
        )
        if network == Network.TESTNET:
            logger.info(
                "   Request tADA from a faucet: https://docs.cardano.org/cardano-testnets/tools/faucet"
            )
        sys.exit(1)

    logger.info(
        f"✅ Selected input UTxO: {selected_input_utxo.input} ({selected_input_utxo.output.amount.coin / 1_000_000:.6f} ADA)"
    )

    # --- Build, Sign, Submit TX using direct context/builder ---
    try:
        builder = TransactionBuilder(context=context)
        builder.add_input(selected_input_utxo)

        for output in outputs_to_create:
            builder.add_output(output)

        logger.info("✍️ Building and signing the combined transaction...")
        signed_tx = builder.build_and_sign(
            signing_keys=[funding_skey],  # type: ignore
            change_address=funding_address,  # type: ignore
        )

        logger.info(
            f"📤 Submitting combined transaction (Estimated Fee: {signed_tx.transaction_body.fee / 1_000_000:.6f} ADA)..."
        )
        tx_id: TransactionId
        if asyncio.iscoroutinefunction(context.submit_tx):
            tx_id = await context.submit_tx(signed_tx.to_cbor())
        else:
            tx_id = await asyncio.to_thread(context.submit_tx, signed_tx.to_cbor())  # type: ignore

        tx_id_str = str(tx_id)
        logger.info(
            f"✅ Transaction submitted successfully! Tx Hash: [bold green]{tx_id_str}[/]"
        )
        scan_url = (
            f"https://preprod.cardanoscan.io/transaction/{tx_id_str}"
            if network == Network.TESTNET
            else f"https://cardanoscan.io/transaction/{tx_id_str}"
        )
        logger.info(
            f"   View on Cardanoscan ({network.name}): [link={scan_url}]{scan_url}[/link]"
        )

        logger.info(
            "✅🏁 Metagraph datum preparation script finished successfully! 🏁✅"
        )

    except Exception as final_err:
        logger.exception(
            f"💥 Final error during transaction build/sign/submit: {final_err}"
        )
        sys.exit(1)


# --- Run Main Async Function ---
if __name__ == "__main__":
    try:
        asyncio.run(prepare_datums())
    except KeyboardInterrupt:
        logger.info("\nScript interrupted by user.")
    except Exception as e:
        logger.exception(f"Failed to run prepare_datums: {e}")
