# sdk/metagraph/metagraph_data.py
import logging
from typing import List, Dict, Any, Optional, Type, Tuple
from pycardano import (
    Address, BlockFrostChainContext, UTxO, PlutusData, ScriptHash, Network
)
import cbor2 # Vẫn cần cho trường hợp datum không chuẩn

# Import các lớp Datum đã cập nhật
from .metagraph_datum import MinerDatum, ValidatorDatum, SubnetDynamicDatum, SubnetStaticDatum

logger = logging.getLogger(__name__)

# --- Hàm lấy dữ liệu cho Miners ---
async def get_all_miner_data(
    context: BlockFrostChainContext,
    script_hash: ScriptHash,
    network: Network
) -> List[Tuple[UTxO, Dict[str, Any]]]:
    """
    Lấy và decode tất cả dữ liệu MinerDatum từ các UTXO tại địa chỉ script.

    Args:
        context: Context blockchain (ví dụ: BlockFrostChainContext).
        script_hash: Script hash của smart contract chứa Datum.
        network: Mạng Cardano đang sử dụng (Network.TESTNET hoặc Network.MAINNET).

    Returns:
        Một list các dictionary, mỗi dictionary chứa thông tin chi tiết của một UTXO
        và Datum đã được decode thành các trường dễ đọc. Trả về list rỗng nếu
        không tìm thấy UTXO hợp lệ hoặc có lỗi xảy ra.
    """
    miner_data_list: List[Tuple[UTxO, Dict[str, Any]]] = []
    contract_address = Address(payment_part=script_hash, network=network)
    logger.info(f"Fetching Miner UTxOs from address: {contract_address}")

    try:
        utxos: List[UTxO] = await context.utxos(str(contract_address))
        logger.info(f"Found {len(utxos)} potential UTxOs at miner contract address.")
    except Exception as e:
        logger.exception(f"Failed to fetch UTxOs for address {contract_address}: {e}")
        return [] # Trả về list rỗng nếu không fetch được

    for utxo in utxos:
        if utxo.output.datum:
            try:
                # Decode bằng phương thức của lớp MinerDatum
                decoded_datum: MinerDatum = MinerDatum.from_cbor(utxo.output.datum.cbor)

                # Trích xuất thông tin từ datum đã decode
                # Sử dụng property để lấy giá trị float đã unscale
                datum_dict = {
                    "uid": getattr(decoded_datum, 'uid', b'').hex(), # Chuyển bytes thành hex
                    "subnet_uid": getattr(decoded_datum, 'subnet_uid', -1),
                    "stake": getattr(decoded_datum, 'stake', 0),
                    "last_performance": getattr(decoded_datum, 'last_performance', 0.0), # Dùng property
                    "trust_score": getattr(decoded_datum, 'trust_score', 0.0), # Dùng property
                    "accumulated_rewards": getattr(decoded_datum, 'accumulated_rewards', 0),
                    "last_update_slot": getattr(decoded_datum, 'last_update_slot', 0),
                    "performance_history_hash": getattr(decoded_datum, 'performance_history_hash', b'').hex() or None, # hex hoặc None
                    "wallet_addr_hash": getattr(decoded_datum, 'wallet_addr_hash', b'').hex(),
                    "status": getattr(decoded_datum, 'status', -1), # Là int
                    "registration_slot": getattr(decoded_datum, 'registration_slot', 0),
                    "api_endpoint": getattr(decoded_datum, 'api_endpoint', b'').decode('utf-8', errors='replace') or None, # decode bytes thành str hoặc None
                }

                # --- Thêm tuple (UTxO, datum_dict) vào list ---
                miner_data_list.append((utxo, datum_dict))

            except Exception as e:
                logger.warning(f"Failed to decode or process MinerDatum for UTxO {utxo.input}: {e}", exc_info=True)
                logger.debug(f"Datum CBOR: {utxo.output.datum.cbor.hex() if utxo.output.datum.cbor else 'None'}")
                continue # Bỏ qua UTXO lỗi
        elif utxo.output.datum_hash:
             logger.debug(f"Skipping UTxO {utxo.input} with datum hash (inline datum required).")
        # else: logger.debug(f"Skipping UTxO {utxo.input} with no datum.")

    if not miner_data_list:
        logger.warning(f"No valid MinerDatum UTxOs found at address {contract_address}.")

    return miner_data_list

# --- Hàm lấy dữ liệu cho Validators ---
async def get_all_validator_data(
    context: BlockFrostChainContext,
    script_hash: ScriptHash,
    network: Network
) -> List[Tuple[UTxO, Dict[str, Any]]]:
    """
    Lấy và decode tất cả dữ liệu ValidatorDatum từ các UTXO tại địa chỉ script.

    Args:
        context: Context blockchain (ví dụ: BlockFrostChainContext).
        script_hash: Script hash của smart contract chứa Datum.
        network: Mạng Cardano đang sử dụng.

    Returns:
        Một list các dictionary chứa thông tin UTXO và ValidatorDatum đã decode.
        Trả về list rỗng nếu có lỗi hoặc không tìm thấy.
    """
    validator_data_list: List[Tuple[UTxO, Dict[str, Any]]] = []
    contract_address = Address(payment_part=script_hash, network=network)
    logger.info(f"Fetching Validator UTxOs from address: {contract_address}")

    try:
        utxos: List[UTxO] = await context.utxos(str(contract_address))
        logger.info(f"Found {len(utxos)} potential UTxOs at validator contract address.")
    except Exception as e:
        logger.exception(f"Failed to fetch UTxOs for address {contract_address}: {e}")
        return []

    for utxo in utxos:
        if utxo.output.datum:
            try:
                decoded_datum: ValidatorDatum = ValidatorDatum.from_cbor(utxo.output.datum.cbor)

                datum_dict = {
                    "uid": getattr(decoded_datum, 'uid', b'').hex(),
                    "subnet_uid": getattr(decoded_datum, 'subnet_uid', -1),
                    "stake": getattr(decoded_datum, 'stake', 0),
                    "last_performance": getattr(decoded_datum, 'last_performance', 0.0), # Dùng property
                    "trust_score": getattr(decoded_datum, 'trust_score', 0.0), # Dùng property
                    "accumulated_rewards": getattr(decoded_datum, 'accumulated_rewards', 0),
                    "last_update_slot": getattr(decoded_datum, 'last_update_slot', 0),
                    "performance_history_hash": getattr(decoded_datum, 'performance_history_hash', b'').hex() or None,
                    "wallet_addr_hash": getattr(decoded_datum, 'wallet_addr_hash', b'').hex(),
                    "status": getattr(decoded_datum, 'status', -1),
                    "registration_slot": getattr(decoded_datum, 'registration_slot', 0),
                    "api_endpoint": getattr(decoded_datum, 'api_endpoint', b'').decode('utf-8', errors='replace') or None, # decode bytes
                }

                # --- Thêm tuple (UTxO, datum_dict) vào list ---
                validator_data_list.append((utxo, datum_dict))

            except Exception as e:
                logger.warning(f"Failed to decode or process ValidatorDatum for UTxO {utxo.input}: {e}", exc_info=True)
                logger.debug(f"Datum CBOR: {utxo.output.datum.cbor.hex() if utxo.output.datum.cbor else 'None'}")
                continue
        elif utxo.output.datum_hash:
             logger.debug(f"Skipping UTxO {utxo.input} with datum hash.")

    if not validator_data_list:
        logger.warning(f"No valid ValidatorDatum UTxOs found at address {contract_address}.")

    return validator_data_list


# --- Có thể thêm các hàm tương tự cho SubnetDatum nếu cần ---
# async def get_all_subnet_dynamic_data(...) -> List[Dict[str, Any]]: ...
# async def get_subnet_static_data(...) -> Optional[Dict[str, Any]]: ...

