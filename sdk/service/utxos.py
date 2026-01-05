# sdk/service/utxos.py
import logging
from typing import Type, Optional
from sdk.compat.pycardano import Address, BlockFrostChainContext, UTxO, PlutusData

# Lấy logger đã cấu hình
logger = logging.getLogger(__name__)


def get_utxo_from_str(
    contract_address: Address,
    datumclass: Type[PlutusData],  # Sử dụng Type[PlutusData] cho rõ ràng
    context: BlockFrostChainContext,
    search_uid: bytes,  # Đảm bảo search_uid là bytes
) -> Optional[UTxO]:  # Trả về Optional[UTxO] vì có thể không tìm thấy
    """
    Lấy một UTxO từ địa chỉ contract dựa trên một UID cụ thể trong datum.

    Args:
        contract_address: Địa chỉ của Plutus smart contract.
        datumclass: Lớp của datum cần decode (ví dụ: MinerDatum).
        context: Context blockchain để truy vấn UTxOs.
        search_uid: UID (dạng bytes) cần tìm trong trường 'uid' của datum.

    Returns:
        UTxO có UID khớp, hoặc None nếu không tìm thấy.
    """
    logger.debug(
        f"Searching for UTxO with UID {search_uid.hex()} at address {contract_address}"
    )
    try:
        utxos = context.utxos(str(contract_address))
        logger.debug(f"Found {len(utxos)} UTxOs at address.")
    except Exception as e:
        logger.error(f"Failed to fetch UTxOs for address {contract_address}: {e}")
        return None

    for utxo in utxos:
        if utxo.output.datum:
            try:
                # Sử dụng from_cbor để decode an toàn hơn
                decoded_datum = datumclass.from_cbor(utxo.output.datum.cbor)  # type: ignore
                # Kiểm tra xem datum có thuộc tính 'uid' không
                if hasattr(decoded_datum, "uid"):
                    # So sánh uid (đảm bảo cả hai là bytes)
                    datum_uid = getattr(decoded_datum, "uid")
                    if isinstance(datum_uid, bytes) and datum_uid == search_uid:
                        logger.info(
                            f"Found UTxO {utxo.input} with matching UID {search_uid.hex()}"
                        )
                        return utxo
                    elif (
                        isinstance(datum_uid, str)
                        and datum_uid.encode("utf-8") == search_uid
                    ):
                        # Xử lý trường hợp uid trong datum là str (dù không nên)
                        logger.warning(
                            f"Datum UID for UTxO {utxo.input} is str, matched after encoding."
                        )
                        return utxo
                else:
                    logger.warning(
                        f"Datum in UTxO {utxo.input} does not have 'uid' attribute."
                    )

            except Exception as e:
                logger.warning(
                    f"Failed to decode or process datum for UTxO {utxo.input}: {e}"
                )
                logger.debug(
                    f"Datum CBOR: {utxo.output.datum.cbor.hex() if utxo.output.datum.cbor else 'None'}"  # type: ignore
                )
                continue  # Bỏ qua UTXO này nếu decode lỗi hoặc không có uid
        # Bỏ qua các UTXO chỉ có datum hash hoặc không có datum
        elif utxo.output.datum_hash:
            logger.debug(f"Skipping UTxO {utxo.input} with datum hash.")
        else:
            logger.debug(f"Skipping UTxO {utxo.input} with no datum.")

    logger.warning(
        f"No UTxO found with UID: {search_uid.hex()} at address {contract_address}"
    )
    return None  # Trả về None nếu không tìm thấy


def get_utxo_with_lowest_performance(  # <<<--- Đổi tên hàm
    contract_address: Address,
    datumclass: Type[PlutusData],  # Sử dụng Type[PlutusData]
    context: BlockFrostChainContext,
) -> Optional[UTxO]:  # Trả về Optional[UTxO]
    """
    Tìm UTxO có giá trị 'scaled_last_performance' thấp nhất tại địa chỉ smart contract.

    Args:
        contract_address: Địa chỉ của Plutus smart contract.
        datumclass: Lớp của datum cần decode (phải có thuộc tính 'scaled_last_performance').
        context: Context blockchain để truy vấn UTxOs.

    Returns:
        UTxO có performance thấp nhất, hoặc None nếu không tìm thấy UTXO hợp lệ nào.
    """
    lowest_performance_utxo: Optional[UTxO] = None
    lowest_performance: float = float("inf")  # <<<--- Khởi tạo bằng vô cùng lớn

    logger.debug(
        f"Searching for UTxO with lowest performance at address {contract_address}"
    )
    try:
        utxos = context.utxos(str(contract_address))
        logger.debug(f"Found {len(utxos)} UTxOs at address.")
    except Exception as e:
        logger.error(f"Failed to fetch UTxOs for address {contract_address}: {e}")
        return None

    found_valid_datum = False
    for utxo in utxos:
        if utxo.output.datum:
            try:
                # Decode datum bằng from_cbor
                decoded_datum = datumclass.from_cbor(utxo.output.datum.cbor)  # type: ignore

                # Kiểm tra và lấy giá trị performance đã scale
                if hasattr(decoded_datum, "scaled_last_performance"):
                    performance = getattr(decoded_datum, "scaled_last_performance")
                    if not isinstance(performance, int):
                        logger.warning(
                            f"UTxO {utxo.input} datum has non-integer 'scaled_last_performance'. Skipping."
                        )
                        continue

                    found_valid_datum = (
                        True  # Đánh dấu đã tìm thấy ít nhất 1 datum hợp lệ
                    )
                    # So sánh performance hiện tại với mức thấp nhất đã tìm thấy
                    if performance < lowest_performance:
                        lowest_performance = performance
                        lowest_performance_utxo = utxo
                        logger.debug(
                            f"Found new lowest performance {lowest_performance} at UTxO {utxo.input}"
                        )
                else:
                    logger.warning(
                        f"Datum in UTxO {utxo.input} does not have 'scaled_last_performance' attribute."
                    )

            except Exception as e:
                logger.warning(
                    f"Failed to decode or process datum for UTxO {utxo.input}: {e}"
                )
                logger.debug(f"Datum CBOR: {utxo.output.datum.cbor.hex() if utxo.output.datum.cbor else 'None'}")  # type: ignore
                continue  # Bỏ qua UTXO này nếu lỗi
        # Bỏ qua UTXO chỉ có datum hash hoặc không có datum
        elif utxo.output.datum_hash:
            logger.debug(f"Skipping UTxO {utxo.input} with datum hash.")
        else:
            logger.debug(f"Skipping UTxO {utxo.input} with no datum.")

    if not found_valid_datum:
        logger.warning(
            f"No UTxOs with valid '{datumclass.__name__}' datum containing 'scaled_last_performance' found at {contract_address}."
        )
        return None  # Trả về None nếu không tìm thấy UTXO nào có datum hợp lệ
    elif lowest_performance_utxo is None:
        # Trường hợp này ít xảy ra nếu found_valid_datum là True, nhưng để phòng ngừa
        logger.error(
            f"Found valid datums but failed to select lowest performance UTxO at {contract_address}."
        )
        return None

    logger.info(
        f"Selected UTxO {lowest_performance_utxo.input} with lowest performance: {lowest_performance}"
    )
    return lowest_performance_utxo
