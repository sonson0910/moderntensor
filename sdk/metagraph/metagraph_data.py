# sdk/metagraph/metagraph_data.py
import logging
from typing import List, Dict, Any, Optional, Type, Tuple, DefaultDict
from collections import defaultdict  # Import defaultdict
from sdk.compat.pycardano import (
    Address,
    BlockFrostChainContext,
    UTxO,
    PlutusData,
    ScriptHash,
    Network,
)
import cbor2  # Vẫn cần cho trường hợp datum không chuẩn

# Import các lớp Datum đã cập nhật
from .metagraph_datum import (
    MinerDatum,
    ValidatorDatum,
    SubnetDynamicDatum,
    SubnetStaticDatum,
)

logger = logging.getLogger(__name__)


# --- Hàm lấy dữ liệu cho Miners ---
async def get_all_miner_data(
    context: BlockFrostChainContext, script_hash: ScriptHash, network: Network
) -> List[Tuple[UTxO, Dict[str, Any]]]:
    """
    Lấy và decode tất cả dữ liệu MinerDatum từ các UTXO tại địa chỉ script.
    CHỈ trả về dữ liệu từ UTXO mới nhất cho mỗi UID.

    Args:
        context: Context blockchain (ví dụ: BlockFrostChainContext).
        script_hash: Script hash của smart contract chứa Datum.
        network: Mạng Cardano đang sử dụng (Network.TESTNET hoặc Network.MAINNET).

    Returns:
        Một list các tuple (UTxO, dictionary), mỗi tuple chứa thông tin chi tiết của UTXO
        mới nhất và Datum tương ứng đã được decode cho mỗi UID. Trả về list rỗng nếu
        không tìm thấy UTXO hợp lệ hoặc có lỗi xảy ra.
    """
    latest_data_list: List[Tuple[UTxO, Dict[str, Any]]] = []
    utxos_by_uid: DefaultDict[str, List[Tuple[UTxO, MinerDatum]]] = defaultdict(
        list
    )  # Nhóm theo UID
    contract_address = Address(payment_part=script_hash, network=network)
    logger.info(f"Fetching Miner UTxOs from address: {contract_address}")

    try:
        utxos: List[UTxO] = context.utxos(str(contract_address))
        logger.info(f"Found {len(utxos)} potential UTxOs at miner contract address.")
    except Exception as e:
        logger.exception(f"Failed to fetch UTxOs for address {contract_address}: {e}")
        return []  # Trả về list rỗng nếu không fetch được

    # 1. Decode và nhóm tất cả UTXO hợp lệ theo UID
    for utxo in utxos:
        if utxo.output.datum and utxo.output.datum.cbor:  # type: ignore[attr-defined]
            try:
                # Decode bằng phương thức của lớp MinerDatum
                decoded_datum: MinerDatum = MinerDatum.from_cbor(utxo.output.datum.cbor)  # type: ignore[assignment, attr-defined]

                uid_bytes = getattr(decoded_datum, "uid", None)
                if uid_bytes and isinstance(uid_bytes, bytes):
                    uid_hex = uid_bytes.hex()
                    utxos_by_uid[uid_hex].append((utxo, decoded_datum))
                else:
                    logger.warning(
                        f"Skipping UTxO {utxo.input} due to invalid or missing UID in decoded MinerDatum."
                    )

            except Exception as e:
                logger.warning(
                    f"Failed to decode MinerDatum for UTxO {utxo.input}: {e}"
                )
                logger.debug(f"Datum CBOR: {utxo.output.datum.cbor.hex()}")  # type: ignore[attr-defined]
                continue  # Bỏ qua UTXO lỗi decode
        elif utxo.output.datum_hash:
            logger.debug(
                f"Skipping UTxO {utxo.input} with datum hash (inline datum required)."
            )

    # 2. Lọc và chọn UTXO mới nhất cho mỗi UID (giả định cái cuối trong list là mới nhất)
    for uid_hex, utxo_datum_list in utxos_by_uid.items():
        if not utxo_datum_list:
            continue

        # Chọn cái cuối cùng trong list, giả định nó là mới nhất
        latest_utxo, latest_decoded_datum = utxo_datum_list[-1]

        try:
            # Trích xuất thông tin từ datum mới nhất đã decode
            datum_dict = {
                "uid": uid_hex,  # Đã có từ key
                "subnet_uid": getattr(latest_decoded_datum, "subnet_uid", -1),
                "stake": getattr(latest_decoded_datum, "stake", 0),
                "last_performance": getattr(
                    latest_decoded_datum, "last_performance", 0.0
                ),
                "trust_score": getattr(latest_decoded_datum, "trust_score", 0.0),
                "accumulated_rewards": getattr(
                    latest_decoded_datum, "accumulated_rewards", 0
                ),
                "last_update_slot": getattr(
                    latest_decoded_datum, "last_update_slot", 0
                ),
                "performance_history_hash": getattr(
                    latest_decoded_datum, "performance_history_hash", b""
                ).hex()
                or None,
                "wallet_addr_hash": getattr(
                    latest_decoded_datum, "wallet_addr_hash", b""
                ).hex()
                or None,  # Allow None
                "status": getattr(latest_decoded_datum, "status", -1),
                "registration_slot": getattr(
                    latest_decoded_datum, "registration_slot", 0
                ),
                "api_endpoint": getattr(
                    latest_decoded_datum, "api_endpoint", b""
                ).decode("utf-8", errors="replace")
                or None,
            }
            latest_data_list.append((latest_utxo, datum_dict))
        except Exception as e:
            logger.warning(
                f"Failed to process latest MinerDatum for UID {uid_hex} from UTxO {latest_utxo.input}: {e}"
            )
            logger.debug(f"Problematic latest MinerDatum: {latest_decoded_datum}")

    if not latest_data_list:
        logger.warning(
            f"No valid latest MinerDatum UTxOs found for any UID at address {contract_address}."
        )

    logger.info(f"Processed {len(latest_data_list)} latest MinerDatum entries.")
    return latest_data_list


# --- Hàm lấy dữ liệu cho Validators ---
async def get_all_validator_data(
    context: BlockFrostChainContext, script_hash: ScriptHash, network: Network
) -> List[Tuple[UTxO, Dict[str, Any]]]:
    """
    Lấy và decode tất cả dữ liệu ValidatorDatum từ các UTXO tại địa chỉ script.
    CHỈ trả về dữ liệu từ UTXO mới nhất cho mỗi UID.

    Args:
        context: Context blockchain (ví dụ: BlockFrostChainContext).
        script_hash: Script hash của smart contract chứa Datum.
        network: Mạng Cardano đang sử dụng.

    Returns:
        Một list các tuple (UTxO, dictionary) chứa thông tin UTXO mới nhất và
        ValidatorDatum tương ứng đã decode cho mỗi UID. Trả về list rỗng nếu có lỗi hoặc không tìm thấy.
    """
    latest_data_list: List[Tuple[UTxO, Dict[str, Any]]] = []
    utxos_by_uid: DefaultDict[str, List[Tuple[UTxO, ValidatorDatum]]] = defaultdict(
        list
    )  # Nhóm theo UID
    contract_address = Address(payment_part=script_hash, network=network)
    logger.info(f"Fetching Validator UTxOs from address: {contract_address}")

    try:
        utxos: List[UTxO] = context.utxos(str(contract_address))
        logger.info(
            f"Found {len(utxos)} potential UTxOs at validator contract address."
        )
    except Exception as e:
        logger.exception(f"Failed to fetch UTxOs for address {contract_address}: {e}")
        return []

    # 1. Decode và nhóm tất cả UTXO hợp lệ theo UID
    for utxo in utxos:
        if utxo.output.datum and utxo.output.datum.cbor:  # type: ignore[attr-defined]
            try:
                # Decode bằng phương thức của lớp ValidatorDatum
                decoded_datum: ValidatorDatum = ValidatorDatum.from_cbor(  # type: ignore[assignment]
                    utxo.output.datum.cbor  # type: ignore[attr-defined]
                )

                uid_bytes = getattr(decoded_datum, "uid", None)
                if uid_bytes and isinstance(uid_bytes, bytes):
                    uid_hex = uid_bytes.hex()
                    utxos_by_uid[uid_hex].append((utxo, decoded_datum))
                else:
                    logger.warning(
                        f"Skipping UTxO {utxo.input} due to invalid or missing UID in decoded ValidatorDatum."
                    )

            except Exception as e:
                logger.warning(
                    f"Failed to decode ValidatorDatum for UTxO {utxo.input}: {e}"
                )
                logger.debug(f"Datum CBOR: {utxo.output.datum.cbor.hex()}")  # type: ignore[attr-defined]
                continue
        elif utxo.output.datum_hash:
            logger.debug(f"Skipping UTxO {utxo.input} with datum hash.")

    # 2. Lọc và chọn UTXO mới nhất cho mỗi UID (giả định cái cuối trong list là mới nhất)
    for uid_hex, utxo_datum_list in utxos_by_uid.items():
        if not utxo_datum_list:
            continue

        # Chọn cái cuối cùng trong list, giả định nó là mới nhất
        latest_utxo, latest_decoded_datum = utxo_datum_list[-1]

        try:
            # Trích xuất thông tin từ datum mới nhất đã decode
            datum_dict = {
                "uid": uid_hex,
                "subnet_uid": getattr(latest_decoded_datum, "subnet_uid", -1),
                "stake": getattr(latest_decoded_datum, "stake", 0),
                "last_performance": getattr(
                    latest_decoded_datum, "last_performance", 0.0
                ),
                "trust_score": getattr(latest_decoded_datum, "trust_score", 0.0),
                "accumulated_rewards": getattr(
                    latest_decoded_datum, "accumulated_rewards", 0
                ),
                "last_update_slot": getattr(
                    latest_decoded_datum, "last_update_slot", 0
                ),
                "performance_history_hash": getattr(
                    latest_decoded_datum, "performance_history_hash", b""
                ).hex()
                or None,
                "wallet_addr_hash": getattr(
                    latest_decoded_datum, "wallet_addr_hash", b""
                ).hex()
                or None,  # Allow None
                "status": getattr(latest_decoded_datum, "status", -1),
                "registration_slot": getattr(
                    latest_decoded_datum, "registration_slot", 0
                ),
                "api_endpoint": getattr(
                    latest_decoded_datum, "api_endpoint", b""
                ).decode("utf-8", errors="replace")
                or None,
            }
            latest_data_list.append((latest_utxo, datum_dict))
        except Exception as e:
            logger.warning(
                f"Failed to process latest ValidatorDatum for UID {uid_hex} from UTxO {latest_utxo.input}: {e}"
            )
            logger.debug(f"Problematic latest ValidatorDatum: {latest_decoded_datum}")

    if not latest_data_list:
        logger.warning(
            f"No valid latest ValidatorDatum UTxOs found for any UID at address {contract_address}."
        )

    logger.info(f"Processed {len(latest_data_list)} latest ValidatorDatum entries.")
    return latest_data_list


# --- Có thể thêm các hàm tương tự cho SubnetDatum nếu cần ---
# async def get_all_subnet_dynamic_data(...) -> List[Dict[str, Any]]: ...
# async def get_subnet_static_data(...) -> Optional[Dict[str, Any]]: ...
