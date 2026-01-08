from sdk.compat.luxtensor_types import (
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
)
from sdk.config.settings import settings
from typing import Optional, List
import logging

logger_cu = logging.getLogger(__name__)  # Logger riêng cho hàm mới
DEFAULT_MIN_UTXO_FETCH_ADA = 5_000_000  # Ước tính tối thiểu ADA cần có trong UTXO đầu vào (2 ADA output + 3 ADA dự phòng/phí)


def find_suitable_ada_input(
    context: BlockFrostChainContext,
    address: str,
    min_ada: int = DEFAULT_MIN_UTXO_FETCH_ADA,
) -> Optional[UTxO]:
    """
    Tìm một UTxO chỉ chứa ADA tại địa chỉ cho trước với lượng ADA tối thiểu.

    Args:
        context: Context blockchain.
        address: Địa chỉ ví cần tìm UTXO.
        min_ada: Lượng ADA tối thiểu (lovelace) mà UTxO phải có.

    Returns:
        UTxO phù hợp đầu tiên tìm thấy, hoặc None.
    """
    logger_cu.info(
        f"Searching for suitable ADA-only UTxO at {address} with at least {min_ada} lovelace..."
    )
    try:
        utxos = context.utxos(address)
        logger_cu.debug(f"Found {len(utxos)} total UTxOs at address.")

        suitable_utxos = []
        for utxo in utxos:
            # Kiểm tra xem có multi_asset không và lượng ADA có đủ không
            if (
                not utxo.output.amount.multi_asset
                and utxo.output.amount.coin >= min_ada
            ):
                suitable_utxos.append(utxo)
                logger_cu.debug(
                    f"  - Found suitable ADA-only UTxO: {utxo.input} with {utxo.output.amount.coin} lovelace"
                )
                # Có thể trả về ngay cái đầu tiên tìm thấy để đơn giản
                # return utxo

        # Nếu muốn chọn UTXO nhỏ nhất đủ dùng thay vì cái đầu tiên:
        if suitable_utxos:
            suitable_utxos.sort(
                key=lambda u: u.output.amount.coin
            )  # Sắp xếp theo ADA tăng dần
            logger_cu.info(
                f"Found {len(suitable_utxos)} suitable ADA-only UTxOs. Selecting the smallest one: {suitable_utxos[0].input}"
            )
            return suitable_utxos[0]

    except Exception as e:
        logger_cu.exception(f"Error fetching or filtering UTxOs for {address}: {e}")

    logger_cu.warning(
        f"No suitable ADA-only UTxO found with at least {min_ada} lovelace at {address}."
    )
    return None


def create_utxo_explicit_input(
    payment_xsk: ExtendedSigningKey,
    stake_xsk: Optional[ExtendedSigningKey],  # Cho phép None
    amount: int,  # Lượng ADA muốn lock trong output mới
    script_hash: ScriptHash,
    datum: PlutusData,
    context: BlockFrostChainContext,
    network: Optional[Network] = None,
) -> TransactionId:
    """
    Tạo UTxO tại địa chỉ script bằng cách chọn tường minh một UTXO đầu vào chỉ chứa ADA.

    Args:
        payment_xsk: Khóa ký payment của ví funding.
        stake_xsk: Khóa ký stake của ví funding (tùy chọn).
        amount: Lượng lovelace sẽ được khóa trong UTxO mới tại địa chỉ script.
        script_hash: Hash của Plutus script.
        datum: Datum cần gắn vào UTxO mới.
        context: Context blockchain.
        network: Mạng Cardano.

    Returns:
        TransactionId của giao dịch đã gửi.

    Raises:
        ValueError: Nếu không tìm thấy UTXO đầu vào phù hợp.
        Exception: Nếu có lỗi xảy ra trong quá trình build hoặc submit.
    """
    network = network or settings.CARDANO_NETWORK # type: ignore

    # --- Xác định địa chỉ ví funding và địa chỉ contract ---
    pay_xvk = payment_xsk.to_verification_key()
    owner_address: Address
    if stake_xsk:
        stk_xvk = stake_xsk.to_verification_key()
        owner_address = Address(pay_xvk.hash(), stk_xvk.hash(), network=network) # type: ignore
    else:
        owner_address = Address(pay_xvk.hash(), network=network) # type: ignore

    contract_address = Address(payment_part=script_hash, network=network) # type: ignore
    logger_cu.info(
        f"Attempting explicit UTXO creation for contract: {contract_address}"
    )
    logger_cu.info(f"Funding Address: {owner_address}")

    # --- Tìm UTXO đầu vào phù hợp (chỉ chứa ADA) ---
    # Ước tính lượng ADA tối thiểu cần trong UTXO input
    # Cần đủ cho `amount` + phí giao dịch ước tính (ví dụ 0.5 ADA)
    min_input_ada = amount + 500_000
    selected_input_utxo = find_suitable_ada_input(
        context, str(owner_address), min_input_ada
    )

    if not selected_input_utxo:
        raise ValueError(
            f"Could not find a suitable ADA-only UTxO with at least {min_input_ada} lovelace at {owner_address}. "
            "Please ensure the funding wallet has adequate, simple UTxOs."
        )

    logger_cu.info(
        f"Selected input UTxO: {selected_input_utxo.input} ({selected_input_utxo.output.amount.coin} lovelace)"
    )

    # --- Xây dựng giao dịch ---
    try:
        builder = TransactionBuilder(context=context)

        # 1. Thêm Input tường minh đã chọn
        builder.add_input(selected_input_utxo)

        # 2. Thêm Output chính đến contract
        builder.add_output(
            TransactionOutput(
                address=contract_address,
                amount=amount,  # Lượng ADA lock vào contract # type: ignore
                datum=datum,
            )
        )

        # 3. Build, Sign, và Submit
        # Để TransactionBuilder tự xử lý việc tính phí và tạo change output
        logger_cu.info("Building and signing transaction...")
        signed_tx = builder.build_and_sign(
            signing_keys=[
                payment_xsk
            ],  # Chỉ cần khóa payment vì input là từ owner_address
            change_address=owner_address,  # Chỉ định địa chỉ nhận lại tiền thừa
        )

        logger_cu.info(
            f"Submitting transaction (Tx Fee: {signed_tx.transaction_body.fee} lovelace)..."
        )
        tx_id = context.submit_tx(signed_tx)  # Code gốc không có await
        logger_cu.info(f"Transaction submitted successfully: {tx_id}")
        return tx_id

    except Exception as e:
        logger_cu.exception(f"Error during explicit UTXO creation transaction: {e}")
        # Có thể in thêm thông tin debug về builder nếu cần
        # print("Builder details:", builder.outputs, builder.inputs, builder.fee)
        raise e  # Ném lại lỗi để script gọi biết


def create_utxo(
    payment_xsk: ExtendedSigningKey,
    stake_xsk: ExtendedSigningKey,
    amount: int,
    script_hash: ScriptHash,
    datum: PlutusData,
    context: BlockFrostChainContext,
    network: Network = None, # type: ignore
) -> TransactionId:
    """
    Creates a UTxO with the specified amount of lovelace and datum locked into a Plutus smart contract.

    This function builds and submits a transaction that locks a given amount of lovelace into a Plutus script
    address, attaching the provided datum. The transaction is signed with the provided payment signing key,
    and any change is returned to the owner's address.

    Args:
        payment_xsk (ExtendedSigningKey): The extended signing key for payment.
        stake_xsk (ExtendedSigningKey): The extended signing key for staking (optional).
        amount (int): The amount of lovelace to lock into the UTxO (e.g., 2_000_000 for 2 tADA).
        script_hash (ScriptHash): The hash of the Plutus script (smart contract).
        datum (PlutusData): The datum (data) to attach to the UTxO.
        context (BlockFrostChainContext): The blockchain context for interacting with the Cardano network.
        network (Network, optional): The Cardano network to use (e.g., Network.TESTNET or Network.MAINNET).
                                     Defaults to settings.CARDANO_NETWORK if not provided.

    Returns:
        TransactionId: The ID of the transaction submitted to the blockchain.
    """
    # Set the network, defaulting to settings.CARDANO_NETWORK if not provided
    network = network or settings.CARDANO_NETWORK

    # Derive the payment verification key from the signing key
    pay_xvk = payment_xsk.to_verification_key()

    # Create the owner's address, including staking part if stake_xsk is provided
    if stake_xsk:
        stk_xvk = stake_xsk.to_verification_key()
        print(pay_xvk.hash())
        print(stk_xvk.hash())

        owner_address = Address(
            payment_part=pay_xvk.hash(),
            staking_part=stk_xvk.hash(),
            network=network,
        )
        print(owner_address)
    else:
        owner_address = Address(
            payment_part=pay_xvk.hash(),
            network=network,
        )
    print(f"Owner Address: {owner_address}")

    # Create the contract address using the provided script hash
    contract_address = Address(
        payment_part=script_hash,
        network=network,
    )
    print(contract_address)

    # Initialize the transaction builder with the blockchain context
    builder = TransactionBuilder(context=context)

    # Add an input from the owner's address to fund the transaction
    builder.add_input_address(owner_address)

    # Add an output to the contract address with the specified amount and datum
    builder.add_output(
        TransactionOutput(
            address=contract_address,
            amount=amount, # type: ignore
            datum=datum,
        )
    )

    # Build and sign the transaction, sending change back to the owner
    signed_tx = builder.build_and_sign(
        signing_keys=[payment_xsk],
        change_address=owner_address,
    )

    # Submit the transaction to the blockchain and return the transaction ID
    tx_id = context.submit_tx(signed_tx)
    return tx_id
